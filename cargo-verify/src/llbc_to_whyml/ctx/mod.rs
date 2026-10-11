//! LLBC → WhyML 的翻译上下文。
//!
//! [`Ctx`] 保存翻译单个 crate 时需要的全部状态：输出的顶层声明、当前作用域内的
//! 泛型参数与局部变量、正在翻译的循环深度等。翻译逻辑按主题拆分到子模块：
//!
//! - [`ty`]、[`type_decl`]：类型与类型声明的翻译；
//! - [`place`]：位置表达式（`Place`）与操作数（`Operand`）的翻译；
//! - [`const_expr`]：常量表达式与字面量的翻译；
//! - [`func_ref`]：函数引用（普通函数、内建函数、trait 方法）的翻译；
//! - [`rvalue`]：右值（运算、聚合等）的翻译；
//! - [`spec`]：规格闭包（契约断言、量词等）的翻译；
//! - [`stmt_expr`]：语句翻译为可执行代码（`Expr`）；
//! - [`stmt_term`]：语句翻译为逻辑项（`Term`），供规格使用；
//! - [`decl`]：函数、全局量与声明组的翻译。

use crate::llbc_to_whyml::{
    builder::{Builder, ExprBuilder, TermBuilder},
    error::Result,
    ident::translate_ident,
    name_map::{self, NameMap, TypeParamNames},
    prelude::{IMPORT_REF, IMPORTS},
};

use charon_lib::{
    ast::{BindingStack, FieldId, LocalId, TranslatedCrate, TypeVarId},
    ids::IndexVec,
};

use why3_ptree::{
    loc::Position,
    ptree::{Decl, Expr, Ident, MlwFile, Term},
};

use std::collections::HashSet;

mod const_expr;
mod decl;
mod func_ref;
mod place;
mod rvalue;
mod spec;
mod stmt_expr;
mod stmt_term;
mod ty;
mod type_decl;

/// 翻译一个 crate，返回生成的 WhyML 文件以及其中用到的元组投影。
pub(super) fn translate_crate(
    crate_: &mut TranslatedCrate,
) -> Result<(MlwFile, HashSet<(usize, FieldId)>)> {
    let name_map = name_map::build(crate_);

    let mut whyml_decls = Vec::new();
    whyml_decls.push(IMPORTS.clone());
    whyml_decls.push(IMPORT_REF.clone());

    let mut ctx = Ctx {
        crate_,
        name_map: &name_map,
        whyml_decls: &mut whyml_decls,
        tuple_field_accesses: HashSet::new(),
        type_param_stack: BindingStack::empty(),
        locals_stack: Vec::new(),
        loop_depth: 0,
    };
    for decl_group in crate_.ordered_decls.as_ref().unwrap() {
        ctx.translate_decl_group(decl_group)?;
    }

    let tuple_field_accesses = ctx.tuple_field_accesses;
    Ok((MlwFile::Decls(whyml_decls.into()), tuple_field_accesses))
}

/// 单个 crate 的翻译上下文。
///
/// 类型参数与局部变量的作用域是嵌套的，因此分别用 [`BindingStack`] 和 `Vec` 保存；
/// 输出的声明先收集到 `whyml_decls`，翻译结束后整体写成 `.mlw` 文件。
struct Ctx<'a> {
    /// 正在翻译的 crate。
    crate_: &'a TranslatedCrate,
    /// 预先计算好的名字映射。
    name_map: &'a NameMap,
    /// 输出的顶层声明。
    whyml_decls: &'a mut Vec<Decl>,
    /// 本次翻译用到的元组投影，翻译结束后统一生成 `Tuple` 模块中的访问函数。
    tuple_field_accesses: HashSet<(usize, FieldId)>,
    /// 当前作用域中泛型参数对应的 WhyML 类型变量。
    type_param_stack: BindingStack<IndexVec<TypeVarId, Ident>>,
    /// 当前作用域中局部变量对应的 WhyML 标识符。
    locals_stack: Vec<IndexVec<LocalId, Ident>>,
    /// 当前所处的循环深度，决定 `break`/`continue` 对应的异常。
    loop_depth: usize,
}

impl<'a> Ctx<'a> {
    /// 记录用到的元组投影 `(arity, field_id)`，供调用方生成访问函数。
    pub(super) fn record_tuple_field_access(&mut self, arity: usize, field_id: FieldId) {
        self.tuple_field_accesses.insert((arity, field_id));
    }

    /// 追加一条顶层声明。
    pub(super) fn push_decl(&mut self, decl: Decl) {
        self.whyml_decls.push(decl);
    }

    /// 追加多条顶层声明。
    pub(super) fn extend_decls(&mut self, decls: impl IntoIterator<Item = Decl>) {
        self.whyml_decls.extend(decls);
    }

    /// 在当前作用域中绑定 `type_param_names` 对应的类型变量，然后执行 `f`。
    pub(super) fn with_generic_params<T>(
        &mut self,
        type_param_names: &TypeParamNames,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.type_param_stack
            .push(type_param_names.map_ref(|ident| translate_ident(ident.as_str().into())));
        let result = f(self);
        self.type_param_stack.pop();
        result
    }

    /// 在当前作用域中绑定一组局部变量名，然后执行 `f`。
    pub(super) fn with_locals<T>(
        &mut self,
        local_names: &IndexVec<LocalId, String>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.locals_stack
            .push(local_names.map_ref(|ident| translate_ident(ident.as_str().into())));
        let result = f(self);
        self.locals_stack.pop();
        result
    }

    /// 当前作用域中 `local_id` 对应的 WhyML 标识符。
    pub(super) fn get_local(&self, local_id: LocalId) -> &Ident {
        &self.locals_stack.last().unwrap()[local_id]
    }

    /// 进入一层循环（`loop_depth` 加一）执行 `f`；`break`/`continue` 的目标由循环深度决定。
    pub(super) fn enter_loop<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.loop_depth += 1;
        let result = f(self);
        self.loop_depth -= 1;
        result
    }

    /// 依次应用多个参数：`f a b` 构造为 `(f a) b`。
    pub(super) fn translate_apply<T: Builder>(
        &self,
        func: T::T,
        args: impl IntoIterator<Item = T::T>,
    ) -> T::T {
        args.into_iter()
            .fold(func, |expr, arg| T::apply(Position::default(), expr, arg))
    }

    /// `translate_apply` 的 `Expr` 版本。
    pub(super) fn translate_apply_expr(
        &self,
        func: Expr,
        args: impl IntoIterator<Item = Expr>,
    ) -> Expr {
        self.translate_apply::<ExprBuilder>(func, args)
    }

    /// `translate_apply` 的 `Term` 版本。
    pub(super) fn translate_apply_term(
        &self,
        func: Term,
        args: impl IntoIterator<Item = Term>,
    ) -> Term {
        self.translate_apply::<TermBuilder>(func, args)
    }
}

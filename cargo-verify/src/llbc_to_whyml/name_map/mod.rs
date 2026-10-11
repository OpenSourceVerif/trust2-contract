//! 名字映射：把 LLBC 中的名字转换成 WhyML 中的名字（字符串形式）。
//!
//! WhyML 的命名空间与 Rust 不同，且同名条目不允许冲突，因此翻译前先为整个
//! crate 建立名字映射（[`NameMap`]），翻译过程中直接查表。
//!
//! 子模块划分：
//!
//! - [`names`]：名字字符串本身（支撑库别名、临时变量名等）；
//! - [`keywords`]：大小写规范化与关键字规避；
//! - [`locals`]：局部变量名与冲突消解；
//! - [`build`]：遍历 crate 构造 [`NameMap`]。

mod build;
mod keywords;
mod locals;
mod names;

pub use build::build;
pub use names::*;

use charon_lib::{
    ast::{
        FieldId, FunDeclId, GlobalDeclId, LocalId, SpecBodyId, SpecClosureId, TypeDeclId,
        TypeVarId, VariantId,
    },
    ids::{IndexMap, IndexVec},
};

/// 一个 crate 中所有条目的名字映射。
///
/// 键是 LLBC 中的条目编号，值是对应的 WhyML 名字。
pub struct NameMap {
    pub type_decl_names: IndexMap<TypeDeclId, TypeDeclNames>,
    pub func_decl_names: IndexMap<FunDeclId, FuncDeclNames>,
    pub global_names: IndexMap<GlobalDeclId, GlobalNames>,
    pub spec_body_names: IndexMap<SpecBodyId, LocalNames>,
    pub spec_closure_names: IndexMap<SpecClosureId, LocalNames>,
}

/// 类型声明的名字：类型本身的名字，以及其成员（字段/构造子）的名字。
pub struct TypeDeclNames {
    pub item_ident: String,
    pub type_param_names: TypeParamNames,
    pub sub_idents: TypeDeclSubIdents,
}

/// 类型声明内部各成员的名字。
pub enum TypeDeclSubIdents {
    Record(IndexVec<FieldId, String>),
    Variant(IndexVec<VariantId, ConstructorIdents>),
    Abstract,
}

/// 枚举构造子的名字（结构体式构造子还包含记录类型的名字与各字段的名字）。
pub struct ConstructorIdents {
    pub constructor_ident: String,
    pub record_idents_opt: Option<(String, IndexVec<FieldId, String>)>,
}

/// 函数声明的名字：函数名、类型参数名、局部变量名与规格中的局部变量名。
pub struct FuncDeclNames {
    pub item_ident: String,
    pub type_param_names: TypeParamNames,
    pub local_names: LocalNames,
    pub spec_names: FuncSpecNames,
}

/// 函数规格（前置/后置条件）中局部变量的名字。
pub struct FuncSpecNames {
    pub precondition_local_names: Vec<LocalNames>,
    pub postcondition_local_names: Vec<LocalNames>,
}

/// 全局量的名字。
pub struct GlobalNames {
    pub item_ident: String,
    pub type_param_names: TypeParamNames,
}

/// 局部变量的名字。
///
/// 有函数体时每个局部变量都有名字；没有函数体（如外部函数）时只有参数可能
/// 有名字，其余用 `None` 表示。
pub enum LocalNames {
    Concrete(IndexVec<LocalId, String>),
    Abstract(Vec<Option<String>>),
}

/// 泛型参数（类型变量）的名字。
pub type TypeParamNames = IndexVec<TypeVarId, String>;

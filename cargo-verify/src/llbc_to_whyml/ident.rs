//! WhyML 标识符的构造。
//!
//! LLBC 中带 `::`、泛型参数等名字不能直接用作 WhyML 标识符，因此需要经过
//! [`name_map`] 处理得到合法字符串，再由本模块包装成语法树中的 [`Ident`]。
//! 此外，编译器内部生成的临时名字（临时变量、元组投影、循环异常）以及
//! 常用的中缀运算符标识符也集中在这里定义。

use crate::llbc_to_whyml::name_map;

use charon_lib::ast::FieldId;

use std::sync::LazyLock;

use why3_ptree::{
    ident::{self, OP_EQU},
    loc::Position,
    ptree::{Ident, Qualid},
    ptree_helpers,
};

/// 把字符串包装成 WhyML 标识符。
pub(super) fn translate_ident(ident: Box<str>) -> Ident {
    ptree_helpers::ident(None, Position::default(), ident)
}

/// 第 `i` 个临时变量的标识符。
pub(super) fn local_temp_ident(i: usize) -> Ident {
    translate_ident(name_map::local_temp_name(i).into())
}

/// 第 `i` 个基本块的标识符（基本块被翻译成 lambda）。
pub(super) fn local_block_ident(i: usize) -> Ident {
    translate_ident(name_map::local_block_name(i).into())
}

/// 元组字段访问函数的标识符。
pub(super) fn tuple_field_accessor_ident(arity: usize, field_id: FieldId) -> Ident {
    translate_ident(name_map::tuple_field_accessor_ident(arity, field_id).into())
}

/// 枚举构造子字段访问函数的标识符。
pub(super) fn variant_constructor_field_accessor_ident(
    constructor_ident: &str,
    field_id: FieldId,
) -> Ident {
    translate_ident(
        name_map::variant_constructor_field_accessor_ident(constructor_ident, field_id).into(),
    )
}

/// 结构体式枚举构造子访问函数的标识符。
pub(super) fn variant_constructor_accessor_ident(constructor_ident: &str) -> Ident {
    translate_ident(name_map::variant_constructor_accessor_ident(constructor_ident).into())
}

/// 第 `loop_depth` 层循环的 `break` 异常标识符。
pub(super) fn break_exn_ident(loop_depth: usize) -> Ident {
    translate_ident(name_map::break_exn_name(loop_depth).into())
}

/// 第 `loop_depth` 层循环的 `continue` 异常标识符。
pub(super) fn continue_exn_ident(loop_depth: usize) -> Ident {
    translate_ident(name_map::continue_exn_name(loop_depth).into())
}

/// 中缀运算符 `=`。
pub(super) static EQUAL: LazyLock<Ident> = LazyLock::new(|| translate_ident(OP_EQU.clone()));

/// 中缀运算符 `<`。
pub(super) static LESS: LazyLock<Ident> = LazyLock::new(|| translate_ident(ident::op_infix("<")));

/// 中缀运算符 `<=`。
pub(super) static LESS_EQUAL: LazyLock<Ident> =
    LazyLock::new(|| translate_ident(ident::op_infix("<=")));

/// 中缀运算符 `>`。
pub(super) static GREATER: LazyLock<Ident> =
    LazyLock::new(|| translate_ident(ident::op_infix(">")));

/// 中缀运算符 `>=`。
pub(super) static GREATER_EQUAL: LazyLock<Ident> =
    LazyLock::new(|| translate_ident(ident::op_infix(">=")));

/// 中缀运算符 `<>`。
pub(super) static NOT_EQUAL: LazyLock<Ident> =
    LazyLock::new(|| translate_ident(ident::op_infix("<>")));

/// `return` 所用的标签。
pub(super) static RETURN_LABEL: LazyLock<Qualid> =
    LazyLock::new(|| ptree_helpers::qualid(Box::new([ptree_helpers::RETURN_ID.into()])));

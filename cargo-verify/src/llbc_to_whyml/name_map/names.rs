//! WhyML 名字（字符串形式）的生成。
//!
//! 这里只负责把各种东西的名字写成字符串：支撑库的模块名与别名、
//! 编译器内部生成的临时名字等。把它们包装成语法树节点的工作见
//! [`crate::llbc_to_whyml::ident`]。

use charon_lib::ast::FieldId;

use std::sync::LazyLock;

/// 名字各段之间的分隔符。
pub(super) const SEPARATOR: &str = "''_";

/// 支撑库所在的模块路径（`use` 的第一段）。
pub const LIB_DIR: &str = "trust2_contract";

/// 支撑库模块的别名（末尾加 `'`，避免与模块名本身混淆）。
pub fn lib_alias(lib_name: &str) -> String {
    format!("{lib_name}'")
}

// 各支撑库：`(模块路径, 别名)`。
pub static LIB_BOOL: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["bool", "Bool"], lib_alias("Bool")));

pub static LIB_BUILTIN: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["builtin", "Builtin"], lib_alias("Builtin")));

pub static LIB_CHAR: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["char", "Char"], lib_alias("Char")));

pub static LIB_I8: LazyLock<(&[&str], String)> = LazyLock::new(|| (&["i8", "I8"], lib_alias("I8")));

pub static LIB_I16: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["i16", "I16"], lib_alias("I16")));

pub static LIB_I32: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["i32", "I32"], lib_alias("I32")));

pub static LIB_I64: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["i64", "I64"], lib_alias("I64")));

pub static LIB_I128: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["i128", "I128"], lib_alias("I128")));

pub static LIB_TUPLE: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["tuple", "Tuple"], lib_alias("Tuple")));

pub static LIB_U8: LazyLock<(&[&str], String)> = LazyLock::new(|| (&["u8", "U8"], lib_alias("U8")));

pub static LIB_U16: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["u16", "U16"], lib_alias("U16")));

pub static LIB_U32: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["u32", "U32"], lib_alias("U32")));

pub static LIB_U64: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["u64", "U64"], lib_alias("U64")));

pub static LIB_U128: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["u128", "U128"], lib_alias("U128")));

/// 空类型（由 Rust 的 `!` 类型翻译而来）的名字。
pub const EMPTY_TYPE_NAME: &str = "a'";

/// 第 `i` 个临时变量的名字。
pub fn local_temp_name(i: usize) -> String {
    format!("x{i}'")
}

/// 第 `i` 个基本块的名字。
pub fn local_block_name(i: usize) -> String {
    format!("b{i}'")
}

/// 元组字段访问函数的名字。
pub fn tuple_field_accessor_ident(arity: usize, field_id: FieldId) -> String {
    format!("get_tuple{arity}{SEPARATOR}{field_id}")
}

/// 结构体式枚举构造子访问函数的名字。
pub fn variant_constructor_accessor_ident(constructor_ident: &str) -> String {
    format!("get_{constructor_ident}''")
}

/// 枚举构造子字段访问函数的名字。
pub fn variant_constructor_field_accessor_ident(
    constructor_ident: &str,
    field_id: FieldId,
) -> String {
    format!("get_{constructor_ident}{SEPARATOR}{field_id}'")
}

/// 第 `loop_depth` 层循环的 `break` 异常名字。
pub fn break_exn_name(loop_depth: usize) -> String {
    format!("Break'{loop_depth}'")
}

/// 第 `loop_depth` 层循环的 `continue` 异常名字。
pub fn continue_exn_name(loop_depth: usize) -> String {
    format!("Continue'{loop_depth}'")
}

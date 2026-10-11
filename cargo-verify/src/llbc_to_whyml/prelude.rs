//! WhyML 输出中反复使用的静态项。
//!
//! 包括：对所有整数/布尔/字符/元组支撑库的 `use` 声明（[`IMPORTS`]、[`IMPORT_REF`]），
//! 空类型、字符串类型、单元类型，以及通配模式与常量表达式/项
//! （`true`、`false`、`()`、`absurd` 等）。
//! 这些值都不含位置信息，因此可以复用，不必每次重新构造。

use crate::llbc_to_whyml::{
    ident::translate_ident,
    name_map::{
        EMPTY_TYPE_NAME, LIB_BOOL, LIB_BUILTIN, LIB_CHAR, LIB_DIR, LIB_I8, LIB_I16, LIB_I32,
        LIB_I64, LIB_I128, LIB_TUPLE, LIB_U8, LIB_U16, LIB_U32, LIB_U64, LIB_U128,
    },
};

use why3_ptree::{
    loc::Position,
    ptree::{Decl, Expr, ExprDesc, Ident, PatDesc, Pattern, Pty, Term, TermDesc},
    ptree_helpers,
};

use std::{iter, sync::LazyLock};

/// 对所有支撑库模块的 `use` 声明。
pub(super) static IMPORTS: LazyLock<Decl> = LazyLock::new(|| {
    Decl::Useimport(
        Position::default(),
        false,
        [
            &*LIB_BOOL,
            &*LIB_BUILTIN,
            &*LIB_CHAR,
            &*LIB_I8,
            &*LIB_I16,
            &*LIB_I32,
            &*LIB_I64,
            &*LIB_I128,
            &*LIB_TUPLE,
            &*LIB_U8,
            &*LIB_U16,
            &*LIB_U32,
            &*LIB_U64,
            &*LIB_U128,
        ]
        .into_iter()
        .map(|(path, alias)| {
            (
                ptree_helpers::qualid(
                    iter::once(LIB_DIR)
                        .chain((*path).iter().copied())
                        .map(|ident| ident.into())
                        .collect(),
                ),
                Some(translate_ident(alias.as_str().into())),
            )
        })
        .collect(),
    )
});

/// 对 Why3 标准库中引用类型 `ref.Ref` 的 `use` 声明。
pub(super) static IMPORT_REF: LazyLock<Decl> = LazyLock::new(|| {
    Decl::Useimport(
        Position::default(),
        false,
        Box::new([(
            ptree_helpers::qualid(Box::new(["ref".into(), "Ref".into()])),
            None,
        )]),
    )
});

/// 元组支撑库的别名 `Tuple'`。
pub(super) static LIB_TUPLE_IDENT: LazyLock<Ident> =
    LazyLock::new(|| translate_ident(LIB_TUPLE.1.as_str().into()));

/// 字符串类型。
pub(super) static STRING: LazyLock<Pty> = LazyLock::new(|| {
    Pty::Tyapp(
        ptree_helpers::qualid(Box::new(["string".into()])),
        Box::new([]),
    )
});

/// 空类型（由 Rust 的 `!` 类型翻译而来）。
pub(super) static EMPTY: LazyLock<Pty> = LazyLock::new(|| {
    Pty::Tyapp(
        ptree_helpers::qualid(Box::new([EMPTY_TYPE_NAME.into()])),
        Box::new([]),
    )
});

/// 单元类型 `()`。
pub(super) static UNIT_TYPE: LazyLock<Pty> = LazyLock::new(|| Pty::Tuple(Box::new([])));

// static ARRAY: LazyLock<Qualid> =
//     LazyLock::new(|| Qualid(Box::new([LIB_IDENT.clone(), translate_ident("array".into())])));

/// 通配模式 `_`。
pub(super) static WILDCARD: LazyLock<Pattern> =
    LazyLock::new(|| ptree_helpers::pat(Position::default(), PatDesc::Wild));

/// `absurd`：不可达分支的表达式。
pub(super) static ABSURD_EXPR: LazyLock<Expr> =
    LazyLock::new(|| ptree_helpers::expr(Position::default(), ExprDesc::Absurd));

/// 单元值 `()`（表达式形式）。
pub(super) static UNIT_EXPR: LazyLock<Expr> =
    LazyLock::new(|| ptree_helpers::expr(Position::default(), ExprDesc::Tuple(Box::new([]))));

/// 单元值 `()`（项形式）。
pub(super) static UNIT_TERM: LazyLock<Term> =
    LazyLock::new(|| ptree_helpers::term(Position::default(), TermDesc::Tuple(Box::new([]))));

/// `true`（表达式形式）。
pub(super) static TRUE_EXPR: LazyLock<Expr> =
    LazyLock::new(|| ptree_helpers::expr(Position::default(), ExprDesc::True));

/// `true`（项形式）。
pub(super) static TRUE_TERM: LazyLock<Term> =
    LazyLock::new(|| ptree_helpers::term(Position::default(), TermDesc::True));

/// `false`（表达式形式）。
pub(super) static FALSE_EXPR: LazyLock<Expr> =
    LazyLock::new(|| ptree_helpers::expr(Position::default(), ExprDesc::False));

/// `false`（项形式）。
pub(super) static FALSE_TERM: LazyLock<Term> =
    LazyLock::new(|| ptree_helpers::term(Position::default(), TermDesc::False));

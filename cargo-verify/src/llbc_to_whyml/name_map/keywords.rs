//! 标识符的大小写规范化与 WhyML 关键字规避。
//!
//! WhyML 对标识符的写法有要求（类型名/构造子必须大写开头，值名必须小写开头），
//! 同时内置关键字与预定义标识符不能直接用作名字，这里负责把 LLBC 中的
//! 名字转换成合法的 WhyML 名字。

use utils::case;

use std::{borrow::Cow, collections::HashSet, sync::LazyLock};

/// 枚举构造子的名字前缀（WhyML 要求构造子首字母大写）。
pub(super) const CONSTRUCTOR_PREFIX: &str = "C";

/// 类型参数的名字前缀。
pub(super) const TYPE_PARAM_PREFIX: &str = "a";

// fn snake_to_u(ident: &str, prefix: &str) -> String {
//     if case::is_snake_case(ident) {
//         let ident = case::to_camel_case(ident);
//         if ident.as_bytes().first().is_some_and(u8::is_ascii_uppercase) {
//             ident
//         } else {
//             format!("{prefix}{ident}")
//         }
//     } else {
//         ensure_u(ident.into(), prefix)
//     }
// }

/// 驼峰式名字 → 小写下划线式名字（用于值）。
pub(super) fn camel_to_l(ident: &str) -> String {
    if case::is_camel_case(ident) {
        let ident = case::to_snake_case(ident);
        if let Some(c) = ident.as_bytes().first() {
            if is_l(*c) { ident } else { format!("_{ident}") }
        } else {
            "__".into()
        }
    } else {
        ensure_l(ident.into())
    }
}

/// 全大写名字 → 小写下划线式名字（用于全局量）。
pub(super) fn upper_to_l(ident: &str) -> String {
    if case::is_upper_case(ident) {
        let ident = case::to_snake_case(ident);
        if let Some(c) = ident.as_bytes().first() {
            if is_l(*c) { ident } else { format!("_{ident}") }
        } else {
            "__".into()
        }
    } else {
        ensure_l(ident.into())
    }
}

/// 驼峰式名字 → 类型参数式名字（必要时加前缀）。
pub(super) fn camel_to_q(ident: &str, prefix: &str) -> String {
    if case::is_camel_case(ident) {
        let ident = case::to_snake_case(ident);
        if let Some(c) = ident.as_bytes().first() {
            if c.is_ascii_lowercase() {
                ident
            } else {
                format!("{prefix}_{ident}")
            }
        } else {
            prefix.into()
        }
    } else {
        format!("{prefix}_{ident}")
    }
}

/// 保证名字以小写字母或下划线开头。
pub(super) fn ensure_l(ident: Cow<str>) -> String {
    if is_l(ident.as_bytes()[0]) {
        ident.into_owned()
    } else {
        format!("_{ident}")
    }
}

/// 保证名字以大写字母开头（必要时加前缀）。
pub(super) fn ensure_u(ident: Cow<str>, prefix: &str) -> String {
    if ident.as_bytes()[0].is_ascii_uppercase() {
        ident.into_owned()
    } else {
        format!("{prefix}{ident}")
    }
}

/// 判断字符能否作为小写标识符的开头。
pub(super) fn is_l(c: u8) -> bool {
    c.is_ascii_lowercase() || c == b'_'
}

/// 把 Rust 标识符中 WhyML 不接受的字符转义掉。
pub(super) fn to_whyml(ident: &str) -> String {
    let mut ident_ = String::with_capacity(ident.len());
    for c in ident.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            ident_.push(c);
        } else if c.is_ascii() {
            ident_.push_str("'_");
        } else {
            ident_.push_str(&format!("''{}", c as u32));
        }
    }
    ident_
}

/// WhyML 的关键字与预定义标识符，不能用作普通名字。
pub(super) static TAKEN_WORDS_SET: LazyLock<HashSet<&str>> = LazyLock::new(|| {
    const TAKEN_WORDS: &[&str] = &[
        // Keywords
        "abstract",
        "absurd",
        "alias",
        "any",
        "as",
        "assert",
        "assume",
        "at",
        "axiom",
        "begin",
        "break",
        "by",
        "check",
        "clone",
        "coinductive",
        "constant",
        "continue",
        "diverges",
        "do",
        "done",
        "downto",
        "else",
        "end",
        "ensures",
        "epsilon",
        "exception",
        "exists",
        "export",
        "false",
        "for",
        "forall",
        "fun",
        "function",
        "ghost",
        "goal",
        "if",
        "import",
        "in",
        "inductive",
        "invariant",
        "label",
        "lemma",
        "let",
        "match",
        "meta",
        "module",
        "mutable",
        "not",
        "old",
        "partial",
        "predicate",
        "private",
        "pure",
        "raise",
        "raises",
        "reads",
        "rec",
        "requires",
        "return",
        "returns",
        "scope",
        "so",
        "then",
        "theory",
        "to",
        "true",
        "try",
        "type",
        "use",
        "val",
        "variant",
        "while",
        "with",
        "writes",
        "float",
        "range",
        "ref",
        // Pre-defined identifiers
        "int",
        "real",
        "string",
        "bool",
        "True",
        "False",
        "tuple0",
        "Tuple0",
        "unit",
    ];

    let mut word_set = HashSet::new();
    for &word in TAKEN_WORDS {
        word_set.insert(word);
    }
    word_set
});

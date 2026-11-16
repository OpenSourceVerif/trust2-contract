//! 翻译过程中可能出现的错误。
//!
//! 这些错误都表示「LLBC 中出现了本翻译器尚不支持的东西」，
//! 因此大多数变体只携带出错的位置信息，用于给出可读的报错。

use charon_lib::ast::{ItemId, TypeDeclId};

use itertools::Itertools;

use std::{
    error,
    fmt::{self, Display, Formatter},
    result,
};

#[derive(Debug)]
pub enum Error {
    MixedDeclGroup(Vec<ItemId>),
    Union(TypeDeclId),
    EnumVariantWithFields(String),
    TypeInvariant(String),
    RawPointer,
    RawBytesConst,
    InlineAsm,
    CopyNonOverlappingInSpec,
    LoopInSpec,
    NestedSpec,
    VariadicFunc,
}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Error::MixedDeclGroup(item_ids) => {
                write!(
                    f,
                    "unsupported mixed declaration group: {}",
                    item_ids.iter().format(", ")
                )
            }
            Error::Union(type_decl_id) => {
                write!(f, "unsupported union: {type_decl_id}")
            }
            Error::EnumVariantWithFields(variant) => {
                write!(f, "unsupported enum variant with fields: {variant}")
            }
            Error::TypeInvariant(ty_name) => {
                write!(
                    f,
                    "type invariant verification is not yet supported in WhyML backend: {ty_name}"
                )
            }
            Error::RawPointer => write!(f, "unsupported raw pointer"),
            Error::RawBytesConst => write!(f, "unsupported constant in raw byte representation"),
            Error::InlineAsm => write!(f, "unsupported inline assembly"),
            Error::CopyNonOverlappingInSpec => {
                write!(f, "`CopyNonOverlapping` statement in specification")
            }
            Error::LoopInSpec => write!(f, "loop in specification"),
            Error::NestedSpec => write!(f, "nested specification"),
            Error::VariadicFunc => write!(f, "variadic function"),
        }
    }
}

pub type Result<T> = result::Result<T, Error>;

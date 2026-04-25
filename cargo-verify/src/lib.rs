//! Library entrypoints for `cargo-verify`.
//!
//! The CLI binary stays thin and delegates translation plus spec lowering to
//! this crate so the same logic can be exercised from tests.

pub mod rust_to_llbc;
pub mod spec_ast;
pub mod spec_lower;

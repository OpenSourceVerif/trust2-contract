//! Lower trust2-contract `spec` blocks from LLBC closure form to a Why3-style AST.

/// Stateful evaluator for LLBC statements and expressions inside one spec body.
mod builder;
/// Closure-specific metadata extraction and binder/pattern shaping helpers.
mod closure;
/// Error types reported while lowering one crate's specs.
mod errors;
/// Public lowering entrypoint plus top-level orchestration.
mod lower;
/// Shared name and operator translation helpers.
mod naming;

#[cfg(test)]
mod tests;

pub use self::{
    errors::{SpecLowerError, SpecLowerErrors},
    lower::lower_crate_specs,
    naming::name_to_string,
};

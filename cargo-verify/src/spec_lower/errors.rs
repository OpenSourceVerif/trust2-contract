//! Error types used while lowering LLBC specs into the internal spec AST.

use charon_lib::ast::Span;

use std::{
    error,
    fmt::{self, Display, Formatter},
};

/// Structured description of one lowering failure.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SpecLowerError {
    /// Fully qualified function name being lowered when the error occurred.
    pub(super) function_name: Box<str>,
    /// Logical sub-kind inside the spec, such as `precondition` or `postcondition/forall`.
    pub(super) spec_kind: Box<str>,
    /// Source span attached to the LLBC node that triggered the failure.
    pub(super) span: Span,
    /// Human-readable explanation of the unsupported pattern or missing data.
    pub(super) reason: Box<str>,
}

impl Display for SpecLowerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "function=`{}`, spec_kind=`{}`: {}",
            self.function_name, self.spec_kind, self.reason
        )
    }
}

impl error::Error for SpecLowerError {}

/// Aggregated spec-lowering failures for one crate.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SpecLowerErrors {
    /// All lowering failures collected while scanning a crate.
    pub(super) errors: Vec<SpecLowerError>,
}

impl Display for SpecLowerErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "spec lowering failed with {} error(s):",
            self.errors.len()
        )?;
        for error in &self.errors {
            writeln!(f, "  - {error}")?;
        }
        Ok(())
    }
}

impl error::Error for SpecLowerErrors {}

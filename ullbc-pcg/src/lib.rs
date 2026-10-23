//! Independent PCG construction over Charon ULLBC.
//!
//! The initial supported fragment is a straight-line CFG (including goto chains),
//! scalar/unit locals, and non-escaping direct shared/mutable references to them.
//! Reference liveness is computed from ULLBC itself. Calls, reborrows, reference
//! arguments/returns, aggregates containing references, branches and loops are
//! rejected. Four-phase rules and dependency-aware leaf packing follow the
//! pinned upstream subset documented in ALGORITHM.md.
//! No MIR, rustc borrow facts, or MIR-PCG results are consumed.
mod analysis;
pub mod export;
mod graph;
mod input;
mod liveness;
mod pcg;
pub use pcg::EvalStmtPhase;
mod results;
mod transfer;

use charon_lib::ullbc_ast as u;
pub use graph::*;
pub use results::*;

/// Constructs a fresh graph and computes every reachable point independently
/// from ULLBC. Returns no partial Analysis when the body is unsupported/invalid.
pub fn analyze(body: &u::ExprBody) -> Result<Analysis, AnalysisError> {
    analysis::run(body)
}

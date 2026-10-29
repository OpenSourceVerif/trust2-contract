//! Independent CFG PCG construction over Charon ULLBC and adapted rustc NLL facts.
//! No rustc dependency or MIR-PCG results are needed by this crate. The initial
//! supported semantics are scalar/unit locals and non-escaping direct references.
//! Branch joins, cyclic PCG abstraction, calls and reborrows are explicitly rejected.
mod analysis;
mod borrowck;
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

/// Propagates a fresh PCG over the reachable ULLBC CFG using matching compiler facts.
/// Returns no partial Analysis when the body or metadata is unsupported/invalid.
pub fn analyze(
    body: &u::ExprBody,
    facts: &charon_lib::ast::BodyBorrowckInfo,
) -> Result<Analysis, AnalysisError> {
    analysis::run(body, facts)
}

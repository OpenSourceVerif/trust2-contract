//! Compiler-independent PCG rules ported from the pinned upstream checkout.
//! See ALGORITHM.md for source correspondence and the explicit subset boundary.
mod capability;
mod phase;
pub(crate) mod triple;
pub use capability::Capability;
pub use phase::EvalStmtPhase;

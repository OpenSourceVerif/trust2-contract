// Adapted from prusti/pcg at 6fdb05e6377da5316e5388320d2ddf3d13082f6e.
// Source: src/pcg/domain/eval_stmt_phase.rs; compiler/display wrappers removed.
use serde::Serialize;

#[derive(PartialEq, Eq, Copy, Clone, Debug, Ord, PartialOrd, Hash, Serialize)]
pub enum EvalStmtPhase {
    #[serde(rename = "pre_operands")]
    PreOperands,
    #[serde(rename = "post_operands")]
    PostOperands,
    #[serde(rename = "pre_main")]
    PreMain,
    #[serde(rename = "post_main")]
    PostMain,
}

impl EvalStmtPhase {
    pub const fn first() -> Self {
        EvalStmtPhase::PreOperands
    }

    #[must_use]
    pub const fn last() -> Self {
        EvalStmtPhase::PostMain
    }
}

impl EvalStmtPhase {
    #[must_use]
    pub fn is_operands_stage(&self) -> bool {
        matches!(
            self,
            EvalStmtPhase::PreOperands | EvalStmtPhase::PostOperands
        )
    }

    #[must_use]
    pub fn phases() -> [EvalStmtPhase; 4] {
        [
            EvalStmtPhase::PreOperands,
            EvalStmtPhase::PostOperands,
            EvalStmtPhase::PreMain,
            EvalStmtPhase::PostMain,
        ]
    }

    pub fn next(self) -> Option<EvalStmtPhase> {
        match self {
            EvalStmtPhase::PreOperands => Some(EvalStmtPhase::PostOperands),
            EvalStmtPhase::PostOperands => Some(EvalStmtPhase::PreMain),
            EvalStmtPhase::PreMain => Some(EvalStmtPhase::PostMain),
            EvalStmtPhase::PostMain => None,
        }
    }
}

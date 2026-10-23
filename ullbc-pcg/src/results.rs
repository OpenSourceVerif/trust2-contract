use crate::{Capability, Edge, EvalStmtPhase, Graph, LifetimeProjection, Place};
use charon_lib::{ast::LocalId, ullbc_ast as u};
use serde::Serialize;
use std::fmt;

/// Statement index equal to the block's statement count denotes its terminator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Location {
    pub block: u::BlockId,
    pub statement_index: usize,
}
impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}:{}", self.block.index(), self.statement_index)
    }
}

/// Semantic source of a capability transition, retained in exported traces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum CapabilityReason {
    TriplePostcondition,
    PrepareWrite,
    DerefExpansion,
    CollapseDeref,
    RestoreUnblocked,
    DeadReference,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Action {
    Require {
        place: Place,
        capability: Capability,
    },
    SetCapability {
        place: Place,
        from: Option<Capability>,
        to: Option<Capability>,
        reason: CapabilityReason,
    },
    Allocate {
        local: LocalId,
    },
    Deallocate {
        local: LocalId,
    },
    AddEdge {
        edge: Edge,
    },
    RemoveEdge {
        edge: Edge,
    },
    LabelProjection {
        from: LifetimeProjection,
        to: LifetimeProjection,
    },
}
#[derive(Clone, Debug, Serialize)]
pub struct AppliedAction {
    pub action: Action,
    pub after: Graph,
}
#[derive(Clone, Debug, Serialize)]
pub struct PhaseResult {
    pub phase: EvalStmtPhase,
    pub before: Graph,
    pub actions: Vec<AppliedAction>,
    pub after: Graph,
}
#[derive(Clone, Debug, Serialize)]
pub struct PointResult {
    pub location: Location,
    pub initial: Graph,
    pub phases: Vec<PhaseResult>,
    pub live_references_after: Vec<LocalId>,
}
impl PointResult {
    pub fn phase(&self, phase: EvalStmtPhase) -> &PhaseResult {
        self.phases
            .iter()
            .find(|p| p.phase == phase)
            .expect("all four phases are recorded")
    }
    pub fn after(&self) -> &Graph {
        &self.phase(EvalStmtPhase::PostMain).after
    }
    pub fn actions(&self) -> impl Iterator<Item = &AppliedAction> {
        self.phases.iter().flat_map(|p| &p.actions)
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct LocalInfo {
    pub id: LocalId,
    pub name: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Analysis {
    pub locals: Vec<LocalInfo>,
    pub points: Vec<PointResult>,
}
impl Analysis {
    pub fn at(&self, location: Location) -> Option<&PointResult> {
        self.points.iter().find(|point| point.location == location)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ErrorKind {
    Unsupported,
    InvalidInput,
    CapabilityUnavailable,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AnalysisError {
    pub location: Option<Location>,
    pub kind: ErrorKind,
    pub phase: Option<EvalStmtPhase>,
    pub message: String,
}
impl AnalysisError {
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            location: None,
            phase: None,
            kind,
            message: message.into(),
        }
    }
    pub(crate) fn at(mut self, location: Location) -> Self {
        self.location = Some(location);
        self
    }
}
impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(location) = self.location {
            write!(f, "{location}: ")?;
        }
        if let Some(phase) = self.phase {
            write!(f, "{phase:?}: ")?;
        }
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for AnalysisError {}

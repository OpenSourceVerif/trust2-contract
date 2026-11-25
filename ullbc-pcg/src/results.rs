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
    PrepareExclusive,
    ControlFlowJoin,
    DerefExpansion,
    CollapseDeref,
    RestoreUnblocked,
    DeadReference,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Action {
    /// Owned-PCG repacking operations. Their field sets come from the type declaration.
    Expand {
        base: Place,
        fields: Vec<Place>,
        capability: Capability,
    },
    Collapse {
        base: Place,
        fields: Vec<Place>,
        capability: Capability,
    },
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
    /// An edge removal only executes on paths where that edge exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<crate::PathAlternatives>,
    pub action: Action,
    pub after: Graph,
}
#[derive(Clone, Debug, Serialize)]
pub struct PhaseResult {
    pub phase: EvalStmtPhase,
    pub before: Graph,
    pub borrowck_queries: Vec<BorrowckQuery>,
    pub actions: Vec<AppliedAction>,
    pub after: Graph,
}
#[derive(Clone, Debug, Serialize)]
pub struct PointResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callee: Option<charon_lib::ast::FunDeclId>,
    pub location: Location,
    pub initial: Graph,
    pub phases: Vec<PhaseResult>,
    pub live_references_after: Vec<LocalId>,
    pub loans_before: Vec<usize>,
    pub loans_after: Vec<usize>,
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LocalInfo {
    pub id: LocalId,
    pub name: Option<String>,
    pub fields: Vec<FieldInfo>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FieldInfo {
    pub parent: Place,
    pub index: charon_lib::ast::FieldId,
    pub name: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Analysis {
    pub arguments: Vec<LocalId>,
    pub return_place: Place,
    pub locals: Vec<LocalInfo>,
    pub loans: Vec<AnalysisLoan>,
    /// Shared reborrows of *(&T) omitted by rustc BorrowSet::ignore_borrow.
    pub unindexed_shared_reborrows: Vec<Location>,
    /// ULLBC reference locals mapped to the original rustc RegionVids.
    pub reference_regions: Vec<charon_lib::ast::BorrowckReferenceRegion>,
    pub outlives: Vec<charon_lib::ast::BorrowckOutlives>,
    pub points: Vec<PointResult>,
    pub blocks: Vec<BlockResult>,
}
#[derive(Clone, Debug, Serialize)]
pub struct AnalysisLoan {
    pub id: usize,
    pub reserve: Location,
    pub borrowed: Place,
    pub assigned: charon_lib::ast::LocalId,
    pub kind: crate::BorrowKind,
    pub region: usize,
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

#[derive(Clone, Debug, Serialize)]
pub struct Incoming {
    /// None identifies the function entry, including when bb0 has a backedge.
    pub predecessor: Option<u::BlockId>,
    /// Repacking and capability weakening specific to this predecessor.
    /// The predecessor's block exit is the graph before these actions.
    pub actions: Vec<AppliedAction>,
}
#[derive(Clone, Debug, Serialize)]
pub struct BlockResult {
    /// Original exceptional successor, excluded from normal-flow PCG transfer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unwind: Option<u::BlockId>,
    pub incoming: Vec<Incoming>,
    pub block: u::BlockId,
    pub successors: Vec<u::BlockId>,
    pub entry: Graph,
    pub exit: Graph,
}
#[derive(Clone, Debug, Serialize)]
pub enum BorrowckQuery {
    LoanInScope {
        loan: usize,
        location: Location,
        result: bool,
    },
    Outlives {
        from: LocalId,
        to: LocalId,
        result: bool,
    },
}

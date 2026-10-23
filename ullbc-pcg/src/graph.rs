//! PCG nodes/edges for the closed scalar/direct-reference fragment.
//! Mirrors upstream BorrowEdge, DerefEdge (a hyperedge), and BorrowFlowEdge.
pub use crate::pcg::Capability;
use crate::{EvalStmtPhase, Location};
use charon_lib::ast::{LocalId, RegionId};
use serde::Serialize;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Place {
    Local(LocalId),
    Deref(LocalId),
}
impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(id) => write!(f, "_{id}"),
            Self::Deref(id) => write!(f, "*_{id}"),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum BorrowKind {
    Shared,
    Mutable,
}
/// Snapshot before the named phase, matching upstream SnapshotLocation::Before.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Snapshot {
    pub location: Location,
    pub phase: EvalStmtPhase,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct LifetimeProjection {
    pub reference: LocalId,
    pub region: RegionId,
    /// Old reference value (move/overwrite); distinct from an expansion label.
    pub base_label: Option<Snapshot>,
    /// Temporary label while a mutable reference is expanded.
    pub label: Option<Snapshot>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Node {
    Place(Place),
    LifetimeProjection(LifetimeProjection),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum OperandKind {
    Copy,
    Move,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
pub enum Edge {
    Borrow {
        location: Location,
        borrowed: Place,
        assigned: LifetimeProjection,
        borrow_kind: BorrowKind,
    },
    /// {reference, projection} -> deref: both source nodes are blocked.
    Deref {
        reference: LocalId,
        projection: LifetimeProjection,
    },
    BorrowFlow {
        from: LifetimeProjection,
        to: LifetimeProjection,
        operand: OperandKind,
    },
}
impl Edge {
    pub fn sources(&self) -> Vec<Node> {
        match self {
            Self::Borrow { borrowed, .. } => vec![Node::Place(*borrowed)],
            Self::Deref {
                reference,
                projection,
            } => vec![
                Node::Place(Place::Local(*reference)),
                Node::LifetimeProjection(*projection),
            ],
            Self::BorrowFlow { from, .. } => vec![Node::LifetimeProjection(*from)],
        }
    }
    pub fn target(&self) -> Node {
        match self {
            Self::Borrow { assigned, .. } => Node::LifetimeProjection(*assigned),
            Self::Deref { reference, .. } => Node::Place(Place::Deref(*reference)),
            Self::BorrowFlow { to, .. } => Node::LifetimeProjection(*to),
        }
    }
    pub(crate) fn rename_projection(&mut self, from: LifetimeProjection, to: LifetimeProjection) {
        let rename = |p: &mut LifetimeProjection| {
            if *p == from {
                *p = to;
            }
        };
        match self {
            Self::Borrow { assigned, .. } => rename(assigned),
            Self::Deref { projection, .. } => rename(projection),
            Self::BorrowFlow { from, to, .. } => {
                rename(from);
                rename(to);
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlaceCapability {
    pub place: Place,
    pub capability: Capability,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OwnedLocal {
    pub local: LocalId,
    pub allocated: bool,
    pub initialized: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Graph {
    pub owned: Vec<OwnedLocal>,
    pub capabilities: Vec<PlaceCapability>,
    pub edges: Vec<Edge>,
}
impl Graph {
    pub fn capability(&self, place: Place) -> Option<Capability> {
        self.capabilities
            .iter()
            .find(|entry| entry.place == place)
            .map(|entry| entry.capability)
    }
    pub fn borrows(&self) -> impl Iterator<Item = &Edge> {
        self.edges
            .iter()
            .filter(|e| matches!(e, Edge::Borrow { .. }))
    }
    pub fn is_leaf(&self, node: Node) -> bool {
        !self.edges.iter().any(|e| e.sources().contains(&node))
    }
    pub(crate) fn set_capability(&mut self, place: Place, capability: Option<Capability>) {
        self.capabilities.retain(|c| c.place != place);
        if let Some(capability) = capability {
            self.capabilities
                .push(PlaceCapability { place, capability });
        }
        self.capabilities.sort_by_key(|c| c.place);
    }
}

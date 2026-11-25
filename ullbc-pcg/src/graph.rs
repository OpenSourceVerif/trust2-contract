//! Owned place trees, capabilities, lifetime projections and path-conditioned borrow edges.
pub use crate::pcg::Capability;
use crate::{EvalStmtPhase, Location};
use charon_lib::ast::{LocalId, RegionId};
use serde::Serialize;
use std::{collections::BTreeSet, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Place {
    Local(LocalId),
    Deref(LocalId),
    /// Index into this local's field table. Each entry identifies its parent
    /// and the declared field index, including for nested tuples and structs.
    Field {
        local: LocalId,
        projection: usize,
    },
}
impl Place {
    pub fn local(self) -> LocalId {
        match self {
            Self::Local(id) | Self::Deref(id) | Self::Field { local: id, .. } => id,
        }
    }
}
impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(id) => write!(f, "_{id}"),
            Self::Deref(id) => write!(f, "*_{id}"),
            Self::Field { local, projection } => write!(f, "_{local}[projection {projection}]"),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
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
    /// The value of a reference before its stack slot was retired.
    PreviousReference {
        reference: LocalId,
        snapshot: Snapshot,
    },
    ArgumentProjection {
        argument: LocalId,
        region: RegionId,
    },
    Place(Place),
    LifetimeProjection(LifetimeProjection),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum OperandKind {
    Copy,
    Move,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind")]
pub enum Edge {
    /// Dependency specified by a callee signature. The result may alias another
    /// value with that lifetime; the call is not a concrete borrow of its input.
    CallAbstraction {
        location: Location,
        callee: charon_lib::ast::FunDeclId,
        from: LifetimeProjection,
        to: LifetimeProjection,
    },
    /// Upstream BorrowFlow(InitialBorrows): the caller supplies the referent.
    InitialBorrowFlow {
        argument: LocalId,
        to: LifetimeProjection,
    },
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
            Self::InitialBorrowFlow { argument, to } => vec![Node::ArgumentProjection {
                argument: *argument,
                region: to.region,
            }],
            Self::Borrow { borrowed, .. } => vec![Node::Place(*borrowed)],
            Self::Deref {
                reference,
                projection,
            } => vec![
                projection
                    .base_label
                    .map_or(Node::Place(Place::Local(*reference)), |snapshot| {
                        Node::PreviousReference {
                            reference: *reference,
                            snapshot,
                        }
                    }),
                Node::LifetimeProjection(*projection),
            ],
            Self::BorrowFlow { from, .. } | Self::CallAbstraction { from, .. } => {
                vec![Node::LifetimeProjection(*from)]
            }
        }
    }
    pub fn target(&self) -> Node {
        match self {
            Self::InitialBorrowFlow { to, .. } => Node::LifetimeProjection(*to),
            Self::Borrow { assigned, .. } => Node::LifetimeProjection(*assigned),
            Self::Deref { reference, .. } => Node::Place(Place::Deref(*reference)),
            Self::BorrowFlow { to, .. } | Self::CallAbstraction { to, .. } => {
                Node::LifetimeProjection(*to)
            }
        }
    }
    pub(crate) fn rename_projection(&mut self, from: LifetimeProjection, to: LifetimeProjection) {
        let rename = |p: &mut LifetimeProjection| {
            if *p == from {
                *p = to;
            }
        };
        match self {
            Self::InitialBorrowFlow { to: target, .. } => rename(target),
            Self::Borrow { assigned, .. } => rename(assigned),
            Self::Deref { projection, .. } => rename(projection),
            Self::BorrowFlow { from, to, .. } | Self::CallAbstraction { from, to, .. } => {
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
/// A predecessor selection at a ULLBC join. An alternative is the conjunction
/// of its CFG edges; alternatives are disjoined.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct CfgEdge {
    pub from: charon_lib::ullbc_ast::BlockId,
    pub to: charon_lib::ullbc_ast::BlockId,
}
pub type PathAlternatives = BTreeSet<BTreeSet<CfgEdge>>;
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EdgeCondition {
    pub edge: Edge,
    pub alternatives: PathAlternatives,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Graph {
    /// Edges absent from this list hold on every incoming path.
    pub edge_conditions: Vec<EdgeCondition>,
    /// Definitely initialized caller-owned referents of scalar reference arguments.
    pub initialized_referents: BTreeSet<LocalId>,
    pub owned: Vec<OwnedLocal>,
    pub expanded: std::collections::BTreeSet<Place>,
    pub initialized_fields: std::collections::BTreeSet<Place>,
    pub capabilities: Vec<PlaceCapability>,
    pub edges: Vec<Edge>,
}
impl Graph {
    pub fn condition(&self, edge: &Edge) -> Option<&PathAlternatives> {
        self.edge_conditions
            .iter()
            .find(|c| &c.edge == edge)
            .map(|c| &c.alternatives)
    }
    pub(crate) fn rename_projection(&mut self, from: LifetimeProjection, to: LifetimeProjection) {
        // Two path-specific edges may become identical when a label is removed.
        // A single resulting edge holds on the union of their paths.
        let mut conditions: std::collections::BTreeMap<_, _> = self
            .edge_conditions
            .drain(..)
            .map(|c| (c.edge, c.alternatives))
            .collect();
        let mut merged = std::collections::BTreeMap::<Edge, Option<PathAlternatives>>::new();
        self.edges.retain_mut(|edge| {
            let condition = conditions.remove(edge);
            edge.rename_projection(from, to);
            match merged.entry(edge.clone()) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(condition);
                    true
                }
                std::collections::btree_map::Entry::Occupied(mut slot) => {
                    if let Some(previous) = slot.get_mut() {
                        if let Some(paths) = condition {
                            previous.extend(paths);
                        } else {
                            *slot.get_mut() = None;
                        }
                    }
                    false
                }
            }
        });
        self.edge_conditions = merged
            .into_iter()
            .filter_map(|(edge, c)| c.map(|alternatives| EdgeCondition { edge, alternatives }))
            .collect();
    }
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

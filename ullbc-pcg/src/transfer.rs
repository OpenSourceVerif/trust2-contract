//! Subset port of PCG's visitor/{mod,triple,stmt,assign,pack,obtain}.rs.
//! Rule order follows upstream; see ALGORITHM.md for exact correspondence.
use crate::{
    input::{Operand, Operation, Program, Rhs},
    pcg::triple::{PlaceCondition, TripleWalker},
    *,
};
use charon_lib::{
    ast::{LocalId, RefKind, Region, RegionId, TyKind},
    ullbc_ast as u,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
struct ReferenceInfo {
    region: RegionId,
    kind: BorrowKind,
}
pub(crate) struct State {
    graph: Graph,
    references: BTreeMap<LocalId, ReferenceInfo>,
    /// Operand's value is evaluated before the destination is overwritten.
    operand_projection: Option<LifetimeProjection>,
}
fn invalid(message: impl Into<String>) -> AnalysisError {
    AnalysisError::new(ErrorKind::InvalidInput, message)
}
fn denied(place: Place, cap: Capability) -> AnalysisError {
    AnalysisError::new(
        ErrorKind::CapabilityUnavailable,
        format!("cannot obtain {cap:?} for {place}"),
    )
}
impl State {
    pub fn new(body: &u::ExprBody, program: &Program) -> Result<Self, AnalysisError> {
        let mut references = BTreeMap::new();
        for (id, local) in body.locals.locals.iter_enumerated() {
            if let TyKind::Ref(region, _, kind) = local.ty.kind() {
                let Region::Body(region) = region else {
                    return Err(input::unsupported(
                        "reference with non-body region requires region adapter",
                    ));
                };
                if region.index() >= body.bound_body_regions {
                    return Err(invalid("reference region outside body binder"));
                }
                references.insert(
                    id,
                    ReferenceInfo {
                        region: *region,
                        kind: if *kind == RefKind::Mut {
                            BorrowKind::Mutable
                        } else {
                            BorrowKind::Shared
                        },
                    },
                );
            }
        }
        let owned: Vec<_> = body
            .locals
            .locals
            .iter_enumerated()
            .map(|(id, _)| OwnedLocal {
                local: id,
                allocated: !program.explicit_storage.contains(&id),
                initialized: id.index() > 0 && id.index() <= body.locals.arg_count,
            })
            .collect();
        let capabilities = owned
            .iter()
            .filter(|l| l.allocated)
            .map(|l| PlaceCapability {
                place: Place::Local(l.local),
                capability: if l.initialized {
                    Capability::Exclusive
                } else {
                    Capability::Write
                },
            })
            .collect();
        Ok(Self {
            graph: Graph {
                owned,
                capabilities,
                edges: Vec::new(),
            },
            references,
            operand_projection: None,
        })
    }
    pub fn graph(&self) -> Graph {
        self.graph.clone()
    }
    fn owned(&self, id: LocalId) -> &OwnedLocal {
        &self.graph.owned[id.index()]
    }
    fn current_projection(&self, id: LocalId) -> LifetimeProjection {
        LifetimeProjection {
            reference: id,
            region: self.references[&id].region,
            base_label: None,
            label: None,
        }
    }
    fn projections(&self) -> BTreeSet<LifetimeProjection> {
        self.graph
            .edges
            .iter()
            .flat_map(|e| e.sources().into_iter().chain([e.target()]))
            .filter_map(|n| {
                if let Node::LifetimeProjection(p) = n {
                    Some(p)
                } else {
                    None
                }
            })
            .collect()
    }
    /// All graph mutations pass through here, and are snapshotted immediately.
    fn emit(
        &mut self,
        action: Action,
        actions: &mut Vec<AppliedAction>,
    ) -> Result<(), AnalysisError> {
        match &action {
            Action::Require { place, capability } => {
                if !self
                    .graph
                    .capability(*place)
                    .is_some_and(|k| k.permits(*capability))
                {
                    return Err(denied(*place, *capability));
                }
            }
            Action::SetCapability {
                place,
                from,
                to,
                reason,
            } => {
                if self.graph.capability(*place) != *from {
                    return Err(invalid("capability transition has a stale precondition"));
                }
                // Owned initialization is separate from borrowed capabilities, as in
                // OwnedPcg::ensures. Merely weakening for overwrite does not move a value.
                if *reason == CapabilityReason::TriplePostcondition
                    && matches!(to, Some(Capability::Exclusive | Capability::Write))
                {
                    let id = match place {
                        Place::Local(id) => *id,
                        Place::Deref(id) => self.referent(*id)?,
                    };
                    self.graph.owned[id.index()].initialized = *to == Some(Capability::Exclusive);
                }
                self.graph.set_capability(*place, *to);
            }
            Action::Allocate { local } => {
                if self.owned(*local).allocated {
                    return Err(invalid("allocate requires unallocated storage"));
                }
                self.graph.owned[local.index()] = OwnedLocal {
                    local: *local,
                    allocated: true,
                    initialized: false,
                };
                self.graph
                    .set_capability(Place::Local(*local), Some(Capability::Write));
            }
            Action::Deallocate { local } => {
                self.graph.owned[local.index()] = OwnedLocal {
                    local: *local,
                    allocated: false,
                    initialized: false,
                };
                self.graph.set_capability(Place::Local(*local), None);
                self.graph.set_capability(Place::Deref(*local), None);
            }
            Action::AddEdge { edge } => {
                if self.graph.edges.contains(edge) {
                    return Err(invalid("duplicate PCG edge"));
                }
                self.graph.edges.push(edge.clone());
            }
            Action::RemoveEdge { edge } => {
                let index = self
                    .graph
                    .edges
                    .iter()
                    .position(|e| e == edge)
                    .ok_or_else(|| invalid("missing PCG edge"))?;
                self.graph.edges.remove(index);
            }
            Action::LabelProjection { from, to } => {
                for edge in &mut self.graph.edges {
                    edge.rename_projection(*from, *to);
                }
                if self.operand_projection == Some(*from) {
                    self.operand_projection = Some(*to);
                }
            }
        }
        actions.push(AppliedAction {
            action,
            after: self.graph(),
        });
        Ok(())
    }
    fn set(
        &mut self,
        place: Place,
        to: Option<Capability>,
        reason: CapabilityReason,
        actions: &mut Vec<AppliedAction>,
    ) -> Result<(), AnalysisError> {
        let from = self.graph.capability(place);
        if from != to {
            self.emit(
                Action::SetCapability {
                    place,
                    from,
                    to,
                    reason,
                },
                actions,
            )?;
        }
        Ok(())
    }
    fn referent(&self, reference: LocalId) -> Result<LocalId, AnalysisError> {
        let projection = self
            .graph
            .edges
            .iter()
            .find_map(|e| match e {
                Edge::Deref {
                    reference: id,
                    projection,
                } if *id == reference => Some(*projection),
                _ => None,
            })
            .unwrap_or_else(|| self.current_projection(reference));
        let mut work = vec![projection];
        let mut seen = BTreeSet::new();
        let mut owners = BTreeSet::new();
        while let Some(p) = work.pop() {
            if !seen.insert(p) {
                continue;
            }
            for edge in &self.graph.edges {
                match edge {
                    Edge::Borrow {
                        borrowed: Place::Local(id),
                        assigned,
                        ..
                    } if *assigned == p => {
                        owners.insert(*id);
                    }
                    Edge::BorrowFlow { from, to, .. } if *to == p => work.push(*from),
                    _ => {}
                }
            }
        }
        if owners.len() != 1 {
            return Err(denied(Place::Deref(reference), Capability::Read));
        }
        Ok(*owners.first().unwrap())
    }
    fn label_reference(
        &mut self,
        id: LocalId,
        snapshot: Snapshot,
        actions: &mut Vec<AppliedAction>,
    ) -> Result<(), AnalysisError> {
        let projections: Vec<_> = self
            .projections()
            .into_iter()
            .filter(|p| p.reference == id && p.base_label.is_none())
            .collect();
        for from in projections {
            self.emit(
                Action::LabelProjection {
                    from,
                    to: LifetimeProjection {
                        base_label: Some(snapshot),
                        ..from
                    },
                },
                actions,
            )?;
        }
        Ok(())
    }
    fn restore_owned(
        &mut self,
        id: LocalId,
        reason: CapabilityReason,
        actions: &mut Vec<AppliedAction>,
    ) -> Result<(), AnalysisError> {
        let p = Place::Local(id);
        if self.owned(id).allocated && self.graph.is_leaf(Node::Place(p)) {
            let cap = if self.owned(id).initialized {
                Capability::Exclusive
            } else {
                Capability::Write
            };
            // Upstream restore_place leaves R unchanged; obtain upgrades a leaf R
            // if stronger permissions are needed, or the next pre-operands does.
            if self.graph.capability(p) != Some(Capability::Read) {
                self.set(p, Some(cap), reason, actions)?;
            }
        }
        Ok(())
    }
    /// pack_old_and_dead_borrow_leaves: repeatedly remove eligible LEAF edges.
    /// A dead local alone is not enough if another reference depends on it.
    fn pack(
        &mut self,
        live: &BTreeSet<LocalId>,
        actions: &mut Vec<AppliedAction>,
    ) -> Result<(), AnalysisError> {
        let read_leaves: Vec<_> = self
            .graph
            .capabilities
            .iter()
            .filter_map(|c| match c.place {
                Place::Local(id)
                    if c.capability == Capability::Read
                        && self.graph.is_leaf(Node::Place(c.place))
                        && self.owned(id).initialized =>
                {
                    Some(c.place)
                }
                _ => None,
            })
            .collect();
        for p in read_leaves {
            self.set(
                p,
                Some(Capability::Exclusive),
                CapabilityReason::RestoreUnblocked,
                actions,
            )?;
        }
        loop {
            let edge = self
                .graph
                .edges
                .iter()
                .find(|edge| {
                    if !self.graph.is_leaf(edge.target()) {
                        return false;
                    }
                    match edge.target() {
                        // Upstream packs unblocked nonempty projections even if live.
                        Node::Place(Place::Deref(_)) => true,
                        Node::LifetimeProjection(p) => {
                            p.base_label.is_some() || !live.contains(&p.reference)
                        }
                        _ => false,
                    }
                })
                .cloned();
            let Some(edge) = edge else {
                break;
            };
            self.emit(Action::RemoveEdge { edge: edge.clone() }, actions)?;
            match edge {
                Edge::Deref {
                    reference,
                    projection,
                } => {
                    if projection.label.is_some() {
                        self.emit(
                            Action::LabelProjection {
                                from: projection,
                                to: LifetimeProjection {
                                    label: None,
                                    ..projection
                                },
                            },
                            actions,
                        )?;
                    }
                    self.set(
                        Place::Deref(reference),
                        None,
                        CapabilityReason::CollapseDeref,
                        actions,
                    )?;
                    self.restore_owned(reference, CapabilityReason::CollapseDeref, actions)?;
                }
                Edge::Borrow {
                    borrowed: Place::Local(owner),
                    assigned,
                    ..
                } => {
                    self.restore_owned(owner, CapabilityReason::RestoreUnblocked, actions)?;
                    if assigned.base_label.is_none()
                        && !live.contains(&assigned.reference)
                        && self
                            .graph
                            .capability(Place::Local(assigned.reference))
                            .is_some()
                    {
                        self.set(
                            Place::Local(assigned.reference),
                            Some(Capability::Write),
                            CapabilityReason::DeadReference,
                            actions,
                        )?;
                    }
                }
                Edge::BorrowFlow { .. } => {}
                _ => {
                    return Err(invalid(
                        "non-local borrowed source outside supported fragment",
                    ));
                }
            }
        }
        Ok(())
    }
    fn obtain(
        &mut self,
        place: Place,
        required: Capability,
        snapshot: Snapshot,
        actions: &mut Vec<AppliedAction>,
    ) -> Result<(), AnalysisError> {
        if let Place::Deref(id) = place {
            let info = self.references[&id];
            if info.kind == BorrowKind::Shared && required != Capability::Read {
                return Err(denied(place, required));
            }
            let owner = self.referent(id)?;
            if !self.owned(owner).initialized && required != Capability::Write {
                return Err(denied(place, required));
            }
            if !self
                .graph
                .edges
                .iter()
                .any(|e| matches!(e, Edge::Deref { reference, .. } if *reference == id))
            {
                let base = Place::Local(id);
                self.emit(
                    Action::Require {
                        place: base,
                        capability: if required == Capability::Read {
                            Capability::Read
                        } else {
                            Capability::Exclusive
                        },
                    },
                    actions,
                )?;
                let from = self.current_projection(id);
                let projection = if info.kind == BorrowKind::Mutable {
                    LifetimeProjection {
                        label: Some(snapshot),
                        ..from
                    }
                } else {
                    from
                };
                // PCG adds a deref edge, updates caps, then labels the blocked RP.
                self.emit(
                    Action::AddEdge {
                        edge: Edge::Deref {
                            reference: id,
                            projection,
                        },
                    },
                    actions,
                )?;
                let reading = required == Capability::Read || info.kind == BorrowKind::Shared;
                self.set(
                    base,
                    Some(if reading {
                        Capability::Read
                    } else {
                        Capability::ShallowExclusive
                    }),
                    CapabilityReason::DerefExpansion,
                    actions,
                )?;
                self.set(
                    place,
                    Some(if reading {
                        Capability::Read
                    } else {
                        Capability::Exclusive
                    }),
                    CapabilityReason::DerefExpansion,
                    actions,
                )?;
                if projection != from {
                    self.emit(
                        Action::LabelProjection {
                            from,
                            to: projection,
                        },
                        actions,
                    )?;
                }
            }
        } else if let Place::Local(id) = place {
            if !self.owned(id).allocated {
                return Err(denied(place, required));
            }
            if required != Capability::Read
                && self.graph.capability(place) == Some(Capability::Read)
                && self.graph.is_leaf(Node::Place(place))
                && self.owned(id).initialized
            {
                self.set(
                    place,
                    Some(Capability::Exclusive),
                    CapabilityReason::RestoreUnblocked,
                    actions,
                )?;
            }
            if required == Capability::Write && self.references.contains_key(&id) {
                self.label_reference(id, snapshot, actions)?;
            }
        }
        self.emit(
            Action::Require {
                place,
                capability: required,
            },
            actions,
        )
    }
    fn require(
        &mut self,
        c: PlaceCondition,
        snapshot: Snapshot,
        actions: &mut Vec<AppliedAction>,
    ) -> Result<(), AnalysisError> {
        match c {
            PlaceCondition::Capability(p, k) => self.obtain(p, k, snapshot, actions),
            PlaceCondition::Return => self.obtain(
                Place::Local(LocalId::from(0)),
                Capability::Exclusive,
                snapshot,
                actions,
            ),
            PlaceCondition::AllocateOrDeallocate(id) => {
                if self.owned(id).allocated {
                    self.obtain(Place::Local(id), Capability::Write, snapshot, actions)?;
                    self.set(
                        Place::Local(id),
                        Some(Capability::Write),
                        CapabilityReason::PrepareWrite,
                        actions,
                    )?;
                }
                Ok(())
            }
            PlaceCondition::Unalloc(id) => {
                // ULLBC StorageLive explicitly frees/reallocates already-live storage.
                // Keep this adapter difference separate from MIR-PCG's Unalloc rule.
                if self.owned(id).allocated {
                    self.obtain(Place::Local(id), Capability::Write, snapshot, actions)?;
                    self.emit(Action::Deallocate { local: id }, actions)?;
                }
                Ok(())
            }
            PlaceCondition::RemoveCapability(_) => {
                Err(invalid("RemoveCapability is a postcondition"))
            }
        }
    }
    fn ensure(
        &mut self,
        c: PlaceCondition,
        actions: &mut Vec<AppliedAction>,
    ) -> Result<(), AnalysisError> {
        match c {
            PlaceCondition::Capability(p, k) => {
                self.set(p, Some(k), CapabilityReason::TriplePostcondition, actions)
            }
            PlaceCondition::RemoveCapability(p) => {
                self.set(p, None, CapabilityReason::TriplePostcondition, actions)
            }
            PlaceCondition::AllocateOrDeallocate(id) => {
                self.emit(Action::Allocate { local: id }, actions)
            }
            PlaceCondition::Unalloc(id) => self.emit(Action::Deallocate { local: id }, actions),
            PlaceCondition::Return => Err(invalid("Return is a precondition")),
        }
    }
    pub fn phase(
        &mut self,
        op: Operation,
        tw: &TripleWalker,
        location: Location,
        phase: EvalStmtPhase,
        live: &BTreeSet<LocalId>,
    ) -> Result<PhaseResult, AnalysisError> {
        let before = self.graph();
        let snapshot = Snapshot { location, phase };
        let mut actions = Vec::new();
        let triples = if phase.is_operands_stage() {
            &tw.operand_triples
        } else {
            &tw.main_triples
        };
        if phase == EvalStmtPhase::PreOperands {
            self.operand_projection = None;
            self.pack(live, &mut actions)?;
        }
        match phase {
            EvalStmtPhase::PreOperands | EvalStmtPhase::PreMain => {
                for triple in triples {
                    self.require(triple.pre, snapshot, &mut actions)?;
                }
            }
            EvalStmtPhase::PostOperands | EvalStmtPhase::PostMain => {
                for triple in triples {
                    if let Some(post) = triple.post {
                        self.ensure(post, &mut actions)?;
                    }
                }
            }
        }
        match phase {
            EvalStmtPhase::PreOperands => {
                if let Operation::Assign(
                    _,
                    Rhs::Use(Operand::Copy(Place::Local(id)) | Operand::Move(Place::Local(id))),
                ) = op
                    && self.references.contains_key(&id)
                {
                    self.operand_projection = Some(self.current_projection(id));
                }
            }
            EvalStmtPhase::PostOperands => {
                if let Operation::Assign(_, Rhs::Use(Operand::Move(Place::Local(id)))) = op
                    && self.references.contains_key(&id)
                {
                    self.label_reference(id, snapshot, &mut actions)?;
                }
            }
            EvalStmtPhase::PreMain => {
                if let Operation::Assign(target, _) = op {
                    // stmt_pre_main: materialize E/e -> W before overwriting.
                    self.set(
                        target,
                        Some(Capability::Write),
                        CapabilityReason::PrepareWrite,
                        &mut actions,
                    )?;
                }
            }
            EvalStmtPhase::PostMain => {
                if let Operation::Assign(Place::Local(target), rhs) = op {
                    match rhs {
                        Rhs::Borrow(kind, owner) => {
                            self.emit(
                                Action::AddEdge {
                                    edge: Edge::Borrow {
                                        location,
                                        borrowed: Place::Local(owner),
                                        assigned: self.current_projection(target),
                                        borrow_kind: kind,
                                    },
                                },
                                &mut actions,
                            )?;
                        }
                        Rhs::Use(operand) if self.references.contains_key(&target) => {
                            let from = self
                                .operand_projection
                                .ok_or_else(|| invalid("missing reference operand projection"))?;
                            let operand = if matches!(operand, Operand::Move(_)) {
                                OperandKind::Move
                            } else {
                                OperandKind::Copy
                            };
                            // Unique lifetime occurrence in &scalar -> &scalar. No
                            // general outlives query is inferred from region IDs.
                            self.emit(
                                Action::AddEdge {
                                    edge: Edge::BorrowFlow {
                                        from,
                                        to: self.current_projection(target),
                                        operand,
                                    },
                                },
                                &mut actions,
                            )?;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(PhaseResult {
            phase,
            before,
            actions,
            after: self.graph(),
        })
    }
}

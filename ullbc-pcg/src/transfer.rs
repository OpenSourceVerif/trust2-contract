//! Subset port of PCG's visitor/{mod,triple,stmt,assign,pack,obtain}.rs.
//! Rule order follows upstream; see ALGORITHM.md for exact correspondence.
mod join;
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Owner {
    Local(Place),
    Caller(LocalId),
    CallResult,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct ReferenceInfo {
    region: RegionId,
    kind: BorrowKind,
}
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct State<'a> {
    locals: &'a [LocalInfo],
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
impl<'a> State<'a> {
    pub fn finish_block(&mut self) {
        self.operand_projection = None;
    }
    pub fn new(body: &u::ExprBody, program: &'a Program) -> Result<Self, AnalysisError> {
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
        let initialized_fields = owned
            .iter()
            .filter(|l| l.initialized)
            .flat_map(|l| {
                (0..program.locals[l.local.index()].fields.len()).map(move |projection| {
                    Place::Field {
                        local: l.local,
                        projection,
                    }
                })
            })
            .collect();
        let mut edges = Vec::new();
        let mut initialized_referents = BTreeSet::new();
        for (&arg, info) in &references {
            if arg.index() > 0 && arg.index() <= body.locals.arg_count {
                initialized_referents.insert(arg);
                edges.push(Edge::InitialBorrowFlow {
                    argument: arg,
                    to: LifetimeProjection {
                        reference: arg,
                        region: info.region,
                        base_label: None,
                        label: None,
                    },
                });
            }
        }
        Ok(Self {
            locals: &program.locals,
            graph: Graph {
                edge_conditions: Vec::new(),
                initialized_fields,
                expanded: BTreeSet::new(),
                owned,
                capabilities,
                edges,
                initialized_referents,
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
        let condition = match &action {
            Action::RemoveEdge { edge } => self.graph.condition(edge).cloned(),
            _ => None,
        };
        match &action {
            Action::Expand {
                base,
                fields,
                capability,
            } => {
                self.graph.expanded.insert(*base);
                self.graph.set_capability(
                    *base,
                    (*capability == Capability::Read).then_some(Capability::Read),
                );
                for field in fields {
                    self.graph.set_capability(*field, Some(*capability));
                }
            }
            Action::Collapse {
                base,
                fields,
                capability,
            } => {
                self.graph.expanded.remove(base);
                for field in fields {
                    self.graph.set_capability(*field, None);
                }
                self.graph.set_capability(*base, Some(*capability));
            }
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
                    let initialized = *to == Some(Capability::Exclusive);
                    if let Place::Deref(id) = *place {
                        let owners = self.referents(id)?;
                        // A write through a conditional reference initializes just
                        // one owner. A move can invalidate any of the alternatives.
                        if owners.len() == 1 || !initialized {
                            for owner in owners {
                                match owner {
                                    Owner::CallResult if !initialized => {
                                        return Err(input::unsupported(
                                            "move through a reference with an abstract call origin",
                                        ));
                                    }
                                    Owner::CallResult => {}
                                    Owner::Local(p) => self.set_initialized(p, initialized),
                                    Owner::Caller(id) if initialized => {
                                        self.graph.initialized_referents.insert(id);
                                    }
                                    Owner::Caller(id) => {
                                        self.graph.initialized_referents.remove(&id);
                                    }
                                }
                            }
                        }
                    } else {
                        self.set_initialized(*place, initialized);
                    }
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
                self.set_initialized(Place::Local(*local), false);
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
                self.graph.edge_conditions.retain(|c| &c.edge != edge);
            }
            Action::LabelProjection { from, to } => {
                self.graph.rename_projection(*from, *to);
                if self.operand_projection == Some(*from) {
                    self.operand_projection = Some(*to);
                }
            }
        }
        actions.push(AppliedAction {
            condition,
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
        if matches!(to, Some(Capability::Exclusive | Capability::Write))
            && matches!(place, Place::Field { .. })
        {
            let mut ancestor = place;
            while let Some(f) = self.locals[place.local().index()].field(ancestor) {
                ancestor = f.parent;
                if self.graph.capability(ancestor) == Some(Capability::Read) {
                    if !self.graph.is_leaf(Node::Place(ancestor)) {
                        return Err(denied(place, to.unwrap()));
                    }
                    self.emit(
                        Action::SetCapability {
                            place: ancestor,
                            from: Some(Capability::Read),
                            to: None,
                            reason: CapabilityReason::PrepareExclusive,
                        },
                        actions,
                    )?;
                }
            }
        }
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
    fn referents(&self, reference: LocalId) -> Result<BTreeSet<Owner>, AnalysisError> {
        let mut work = vec![Node::Place(Place::Deref(reference))];
        let mut visited = BTreeSet::new();
        let mut owners = BTreeSet::new();
        while let Some(node) = work.pop() {
            if !visited.insert(node) {
                continue;
            }
            match node {
                Node::Place(Place::Deref(id)) => {
                    work.push(Node::LifetimeProjection(self.current_projection(id)));
                    work.extend(self.graph.edges.iter().filter_map(|e| match e {
                        Edge::Deref {
                            reference,
                            projection,
                        } if *reference == id => Some(Node::LifetimeProjection(*projection)),
                        _ => None,
                    }));
                }
                Node::Place(p) => {
                    owners.insert(Owner::Local(p));
                }
                Node::PreviousReference { .. } => {}
                Node::ArgumentProjection { argument, .. } => {
                    owners.insert(Owner::Caller(argument));
                }
                Node::LifetimeProjection(p) => {
                    for edge in &self.graph.edges {
                        match edge {
                            Edge::CallAbstraction { to, .. } if *to == p => {
                                owners.insert(Owner::CallResult);
                            }
                            Edge::InitialBorrowFlow { argument, to } if *to == p => {
                                owners.insert(Owner::Caller(*argument));
                            }
                            Edge::Borrow {
                                borrowed, assigned, ..
                            } if *assigned == p => work.push(Node::Place(*borrowed)),
                            Edge::BorrowFlow { from, to, .. } if *to == p => {
                                work.push(Node::LifetimeProjection(*from))
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        if owners.is_empty() {
            return Err(denied(Place::Deref(reference), Capability::Read));
        }
        Ok(owners)
    }

    fn owner_initialized(&self, owner: Owner) -> bool {
        match owner {
            Owner::Local(p) => self.initialized(p),
            Owner::Caller(id) => self.graph.initialized_referents.contains(&id),
            Owner::CallResult => true,
        }
    }
    fn initialized(&self, p: Place) -> bool {
        match p {
            Place::Local(id) => self.owned(id).initialized,
            Place::Field { .. } => self.graph.initialized_fields.contains(&p),
            Place::Deref(_) => unreachable!("initialization is recorded at the owned place"),
        }
    }
    fn set_initialized(&mut self, place: Place, value: bool) {
        let local = place.local();
        let layout = &self.locals[local.index()];
        let mut work = vec![place];
        while let Some(p) = work.pop() {
            match p {
                Place::Local(id) => self.graph.owned[id.index()].initialized = value,
                Place::Field { .. } => {
                    if value {
                        self.graph.initialized_fields.insert(p);
                    } else {
                        self.graph.initialized_fields.remove(&p);
                    }
                }
                Place::Deref(_) => unreachable!(),
            }
            work.extend(layout.children(p));
        }
        let mut p = place;
        while let Some(field) = layout.field(p) {
            p = field.parent;
            let full = layout
                .children(p)
                .all(|f| self.graph.initialized_fields.contains(&f));
            match p {
                Place::Local(id) => self.graph.owned[id.index()].initialized = full,
                _ if full => {
                    self.graph.initialized_fields.insert(p);
                }
                _ => {
                    self.graph.initialized_fields.remove(&p);
                }
            }
        }
    }

    fn expand_to(
        &mut self,
        place: Place,
        required: Option<Capability>,
        actions: &mut Vec<AppliedAction>,
    ) -> Result<(), AnalysisError> {
        let layout = &self.locals[place.local().index()];
        let Some(field) = layout.field(place) else {
            return Ok(());
        };
        let base = field.parent;
        if !self.graph.expanded.contains(&base) {
            self.expand_to(base, required, actions)?;
            let capability = self
                .graph
                .capability(base)
                .ok_or_else(|| denied(base, Capability::Exclusive))?;
            if !self.graph.is_leaf(Node::Place(base)) {
                return Err(denied(base, Capability::Exclusive));
            }
            let capability =
                if required == Some(Capability::Read) && capability.permits(Capability::Read) {
                    Capability::Read
                } else {
                    capability
                };
            let fields = layout.children(base).collect();
            self.emit(
                Action::Expand {
                    base,
                    fields,
                    capability,
                },
                actions,
            )?;
        }
        Ok(())
    }

    fn collapse(
        &mut self,
        base: Place,
        required: Capability,
        actions: &mut Vec<AppliedAction>,
    ) -> Result<(), AnalysisError> {
        if !self.graph.expanded.contains(&base) {
            return Ok(());
        }
        let fields: Vec<_> = self.locals[base.local().index()].children(base).collect();
        for &field in &fields {
            self.collapse(field, required, actions)?;
            if !self.graph.is_leaf(Node::Place(field)) {
                return Err(denied(field, required));
            }
            if self.graph.capability(field) == Some(Capability::Read) && self.initialized(field) {
                self.set(
                    field,
                    Some(Capability::Exclusive),
                    CapabilityReason::RestoreUnblocked,
                    actions,
                )?;
            }
        }
        let capability = if fields
            .iter()
            .all(|f| self.graph.capability(*f) == Some(Capability::Exclusive))
        {
            Capability::Exclusive
        } else if fields.iter().all(|f| {
            self.graph
                .capability(*f)
                .is_some_and(|c| c.permits(Capability::Write))
        }) {
            Capability::Write
        } else {
            return Err(denied(base, required));
        };
        if !capability.permits(required) {
            return Err(denied(base, required));
        }
        self.emit(
            Action::Collapse {
                base,
                fields,
                capability,
            },
            actions,
        )
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
    fn restore_place(
        &mut self,
        p: Place,
        reason: CapabilityReason,
        actions: &mut Vec<AppliedAction>,
    ) -> Result<(), AnalysisError> {
        if self.owned(p.local()).allocated && self.graph.is_leaf(Node::Place(p)) {
            let initialized = if let Place::Deref(id) = p {
                self.referents(id)?
                    .iter()
                    .all(|p| self.owner_initialized(*p))
            } else {
                self.initialized(p)
            };
            let cap = if matches!(p,Place::Deref(id) if self.references[&id].kind==BorrowKind::Shared)
            {
                Capability::Read
            } else if initialized {
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
        borrowck: &borrowck::BorrowContext<'_>,
        location: Location,
        queries: &mut Vec<BorrowckQuery>,
    ) -> Result<(), AnalysisError> {
        let mut active = BTreeSet::new();
        for edge in &self.graph.edges {
            if let Edge::Borrow {
                location: creation, ..
            } = edge
                && let Some(loan) = borrowck.loan(*creation)
            {
                let loan = loan.id;
                let result = borrowck.in_scope(loan, location);
                queries.push(BorrowckQuery::LoanInScope {
                    loan,
                    location,
                    result,
                });
                if result {
                    active.insert(*creation);
                }
            }
        }
        let read_leaves: Vec<_> = self
            .graph
            .capabilities
            .iter()
            .filter_map(|c| match c.place {
                Place::Local(_) | Place::Field { .. }
                    if c.capability == Capability::Read
                        && self.graph.is_leaf(Node::Place(c.place))
                        && !self.graph.expanded.contains(&c.place)
                        && self.initialized(c.place) =>
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
                    // Conservative compiler guard in addition to ULLBC liveness
                    // and dependency leaves; never release an in-scope loan.
                    if matches!(edge, Edge::Borrow {location, ..} if active.contains(location)) {
                        return false;
                    }
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
                    self.restore_place(
                        Place::Local(reference),
                        CapabilityReason::CollapseDeref,
                        actions,
                    )?;
                }
                Edge::Borrow {
                    borrowed: owner,
                    assigned,
                    ..
                } => {
                    self.restore_place(owner, CapabilityReason::RestoreUnblocked, actions)?;
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
                Edge::BorrowFlow { .. }
                | Edge::InitialBorrowFlow { .. }
                | Edge::CallAbstraction { .. } => {}
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
            let owners = self.referents(id)?;
            if !owners.iter().all(|p| self.owner_initialized(*p)) && required != Capability::Write {
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
        } else {
            let id = place.local();
            self.expand_to(place, Some(required), actions)?;
            if required != Capability::Read
                || self.graph.capability(place) != Some(Capability::Read)
            {
                self.collapse(place, required, actions)?;
            }
            if !self.owned(id).allocated {
                return Err(denied(place, required));
            }
            if required != Capability::Read
                && self.graph.capability(place) == Some(Capability::Read)
                && self.graph.is_leaf(Node::Place(place))
                && self.initialized(place)
            {
                self.set(
                    place,
                    Some(Capability::Exclusive),
                    CapabilityReason::RestoreUnblocked,
                    actions,
                )?;
            }
            if required == Capability::Write
                && matches!(place, Place::Local(_))
                && self.references.contains_key(&id)
            {
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
            PlaceCondition::Return => {
                let ret = LocalId::ZERO;
                self.obtain(Place::Local(ret), Capability::Exclusive, snapshot, actions)?;
                if self.references.contains_key(&ret)
                    && self
                        .referents(ret)?
                        .iter()
                        .any(|owner| matches!(owner, Owner::Local(_)))
                {
                    return Err(invalid("returned reference has no caller-owned source"));
                }
                Ok(())
            }
            PlaceCondition::AllocateOrDeallocate(id) => {
                if self.owned(id).allocated {
                    // A reborrow of the pointee can outlive the reference's
                    // stack slot. Preserve its old value as the Deref source.
                    if self.references.contains_key(&id)
                        && !self.graph.is_leaf(Node::Place(Place::Deref(id)))
                    {
                        self.label_reference(id, snapshot, actions)?;
                    }
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
                if !self.graph.is_leaf(Node::Place(Place::Deref(id))) {
                    return Err(input::unsupported(
                        "reuse of a reference stack slot with an outstanding old dereference",
                    ));
                }
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
        op: &Operation,
        tw: &TripleWalker,
        location: Location,
        phase: EvalStmtPhase,
        live: &BTreeSet<LocalId>,
        borrowck: &borrowck::BorrowContext<'_>,
    ) -> Result<PhaseResult, AnalysisError> {
        let before = self.graph();
        let mut borrowck_queries = Vec::new();
        let snapshot = Snapshot { location, phase };
        let mut actions = Vec::new();
        let triples = if phase.is_operands_stage() {
            &tw.operand_triples
        } else {
            &tw.main_triples
        };
        if phase == EvalStmtPhase::PreOperands {
            self.operand_projection = None;
            self.pack(
                live,
                &mut actions,
                borrowck,
                location,
                &mut borrowck_queries,
            )?;
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
                let source = match op {
                    Operation::Assign(
                        _,
                        Rhs::Use(Operand::Copy(Place::Local(id)) | Operand::Move(Place::Local(id))),
                    ) => Some(*id),
                    Operation::Assign(_, Rhs::Call { result_source, .. }) => *result_source,
                    _ => None,
                };
                if let Some(id) = source
                    && self.references.contains_key(&id)
                {
                    self.operand_projection = Some(self.current_projection(id));
                }
            }
            EvalStmtPhase::PostOperands => {
                for operand in op.operands() {
                    if let Operand::Move(Place::Local(id)) = operand
                        && self.references.contains_key(id)
                    {
                        self.label_reference(*id, snapshot, &mut actions)?;
                    }
                }
            }
            EvalStmtPhase::PreMain => {
                if let Operation::Assign(target, _) = *op {
                    if let Place::Local(id) = target
                        && self.references.contains_key(&id)
                        && !self.graph.is_leaf(Node::Place(Place::Deref(id)))
                    {
                        return Err(input::unsupported(
                            "overwriting a reference with an outstanding reborrow requires a labelled dereference place",
                        ));
                    }
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
                if let Operation::Assign(Place::Local(target), ref rhs) = *op {
                    match rhs {
                        Rhs::Borrow(kind, owner) => {
                            self.emit(
                                Action::AddEdge {
                                    edge: Edge::Borrow {
                                        location,
                                        borrowed: *owner,
                                        assigned: self.current_projection(target),
                                        borrow_kind: *kind,
                                    },
                                },
                                &mut actions,
                            )?;
                        }
                        Rhs::Use(_)
                        | Rhs::Call {
                            result_source: Some(_),
                            ..
                        } if self.references.contains_key(&target) => {
                            let from = self
                                .operand_projection
                                .ok_or_else(|| invalid("missing reference operand projection"))?;
                            let result = borrowck.outlives(from.reference, target);
                            borrowck_queries.push(BorrowckQuery::Outlives {
                                from: from.reference,
                                to: target,
                                result,
                            });
                            if !result {
                                return Err(invalid(
                                    "rustc outlives does not justify reference flow",
                                ));
                            }
                            self.emit(
                                Action::AddEdge {
                                    edge: match rhs {
                                        Rhs::Call { callee, .. } => Edge::CallAbstraction {
                                            location,
                                            callee: *callee,
                                            from,
                                            to: self.current_projection(target),
                                        },
                                        Rhs::Use(operand) => Edge::BorrowFlow {
                                            from,
                                            to: self.current_projection(target),
                                            operand: if matches!(operand, Operand::Move(_)) {
                                                OperandKind::Move
                                            } else {
                                                OperandKind::Copy
                                            },
                                        },
                                        _ => unreachable!("reference assignment"),
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
            borrowck_queries,
            phase,
            before,
            actions,
            after: self.graph(),
        })
    }
}

//! Owned projection trees must agree before their capabilities can be compared.
//! A borrow graph is a may-state. Remember the incoming CFG edges that make each
//! non-universal edge valid, so the next consumer can recover the borrow's origin.
use super::*;

impl<'a> State<'a> {
    pub fn merge(
        block: u::BlockId,
        mut incoming: Vec<(Option<u::BlockId>, Self)>,
    ) -> Result<(Self, Vec<Incoming>), AnalysisError> {
        let mut traces: Vec<_> = incoming
            .iter()
            .map(|(predecessor, _)| Incoming {
                predecessor: *predecessor,
                actions: Vec::new(),
            })
            .collect();
        if incoming.len() == 1 {
            return Ok((incoming.pop().unwrap().1, traces));
        }
        let shapes: BTreeSet<_> = incoming
            .iter()
            .flat_map(|(_, s)| &s.graph.expanded)
            .copied()
            .collect();
        for base in shapes {
            // A local allocated in just one arm does not have a must-capability at the join.
            let always_allocated = incoming
                .iter()
                .all(|(_, s)| s.owned(base.local()).allocated);
            for ((_, s), trace) in incoming.iter_mut().zip(&mut traces) {
                if !always_allocated {
                    if s.graph.expanded.contains(&base) {
                        s.collapse(base, Capability::Write, &mut trace.actions)?;
                    }
                } else if !s.graph.expanded.contains(&base) {
                    let child = s.locals[base.local().index()]
                        .children(base)
                        .next()
                        .ok_or_else(|| invalid("expanded place has no fields"))?;
                    s.expand_to(child, None, &mut trace.actions)?;
                }
            }
        }
        let mut result = incoming[0].1.clone();
        for (_, s) in &incoming[1..] {
            for (owned, other) in result.graph.owned.iter_mut().zip(&s.graph.owned) {
                owned.allocated &= other.allocated;
                owned.initialized &= other.initialized;
            }
            result
                .graph
                .initialized_referents
                .retain(|id| s.graph.initialized_referents.contains(id));
            result
                .graph
                .initialized_fields
                .retain(|p| s.graph.initialized_fields.contains(p));
            result.graph.capabilities.retain_mut(|c| {
                let Some(other) = s.graph.capability(c.place) else {
                    return false;
                };
                if c.capability.permits(other) {
                    c.capability = other;
                    true
                } else {
                    other.permits(c.capability)
                }
            });
        }
        for ((_, state), trace) in incoming.iter_mut().zip(&mut traces) {
            for c in state.graph.capabilities.clone() {
                let to = result.graph.capability(c.place);
                if to != Some(c.capability) {
                    state.emit(
                        Action::SetCapability {
                            place: c.place,
                            from: Some(c.capability),
                            to,
                            reason: CapabilityReason::ControlFlowJoin,
                        },
                        &mut trace.actions,
                    )?;
                }
            }
        }
        let edges: BTreeSet<_> = incoming
            .iter()
            .flat_map(|(_, s)| s.graph.edges.iter().cloned())
            .collect();
        result.graph.edge_conditions.clear();
        for edge in &edges {
            let first = &incoming[0].1.graph;
            if incoming.iter().all(|(_, s)| {
                s.graph.edges.contains(edge) && s.graph.condition(edge) == first.condition(edge)
            }) {
                if let Some(alternatives) = first.condition(edge) {
                    result.graph.edge_conditions.push(EdgeCondition {
                        edge: edge.clone(),
                        alternatives: alternatives.clone(),
                    });
                }
                continue;
            }
            let mut alternatives = PathAlternatives::new();
            for (pred, s) in &incoming {
                if !s.graph.edges.contains(edge) {
                    continue;
                }
                let from = pred.ok_or_else(|| {
                    input::unsupported("loop-carried borrow from the function entry")
                })?;
                let incoming_edge = CfgEdge { from, to: block };
                let paths = s
                    .graph
                    .condition(edge)
                    .cloned()
                    .unwrap_or_else(|| BTreeSet::from([BTreeSet::new()]));
                for mut path in paths {
                    path.insert(incoming_edge);
                    alternatives.insert(path);
                }
            }
            result.graph.edge_conditions.push(EdgeCondition {
                edge: edge.clone(),
                alternatives,
            });
        }
        result.graph.edges = edges.into_iter().collect();
        Ok((result, traces))
    }
}

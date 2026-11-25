//! CFG worklist, block entry/exit states, and four-phase transfer.
use crate::{
    borrowck::BorrowContext, input, liveness, pcg::triple::TripleWalker, transfer::State, *,
};
use charon_lib::{ast::BodyBorrowckInfo, ullbc_ast as u};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(crate) fn run(
    body: &u::ExprBody,
    facts: &BodyBorrowckInfo,
    krate: Option<&charon_lib::ast::TranslatedCrate>,
) -> Result<Analysis, AnalysisError> {
    let program = input::lower(body, krate)?;
    let borrowck = BorrowContext::new(body, facts, &program)?;
    let live = liveness::compute(&program);
    let initial = State::new(body, &program)?;
    let mut entries = BTreeMap::new();
    let mut incoming_traces = BTreeMap::new();
    let backedges = backedges(&program);
    let mut exits: BTreeMap<u::BlockId, State> = BTreeMap::new();
    let mut traces = BTreeMap::new();
    let mut pending = VecDeque::from([u::START_BLOCK_ID]);
    let mut queued = BTreeSet::from([u::START_BLOCK_ID]);
    while let Some(block_id) = pending.pop_front() {
        queued.remove(&block_id);
        let block = &program.blocks[&block_id];
        let mut inputs = Vec::new();
        if block_id == u::START_BLOCK_ID {
            inputs.push((None, initial.clone()));
        }
        for pred in &block.predecessors {
            if let Some(exit) = exits.get(pred) {
                if backedges.contains(&(*pred, block_id)) {
                    let graph = exit.graph();
                    // Summarizing borrows re-created across iterations requires
                    // the upstream loop abstraction. Never reuse a static loan
                    // location as the identity of several dynamic loans.
                    let renewed = graph.edges.iter().any(|e| {
                        matches!(e, Edge::Borrow { location, .. } | Edge::CallAbstraction {location, ..}
                        if reachable(&program, block_id, location.block))
                    });
                    if renewed || !graph.edge_conditions.is_empty() {
                        return Err(input::unsupported(
                            "loop-carried borrow requires a loop abstraction",
                        )
                        .at(Location {
                            block: block_id,
                            statement_index: 0,
                        }));
                    }
                }
                inputs.push((Some(*pred), exit.clone()));
            }
        }
        let (mut state, incoming) = State::merge(block_id, inputs).map_err(|e| {
            e.at(Location {
                block: block_id,
                statement_index: 0,
            })
        })?;
        incoming_traces.insert(block_id, incoming);
        if entries.get(&block_id) == Some(&state) {
            continue;
        }
        entries.insert(block_id, state.clone());
        let mut points = Vec::new();
        for (statement_index, op) in block.operations.iter().enumerate() {
            let location = Location {
                block: block_id,
                statement_index,
            };
            let initial = state.graph();
            let tw = TripleWalker::for_operation(op);
            let mut phases = Vec::new();
            for phase in EvalStmtPhase::phases() {
                phases.push(
                    state
                        .phase(op, &tw, location, phase, &live.before[&location], &borrowck)
                        .map_err(|mut e| {
                            e.phase = Some(phase);
                            e.at(location)
                        })?,
                );
            }
            let point = borrowck.point(location);
            points.push(PointResult {
                callee: match op {
                    input::Operation::Assign(_, input::Rhs::Call { callee, .. }) => Some(*callee),
                    _ => None,
                },
                location,
                initial,
                phases,
                live_references_after: live.after[&location].iter().copied().collect(),
                loans_before: point.before.clone(),
                loans_after: point.after.clone(),
            });
        }
        state.finish_block();
        traces.insert(block_id, points);
        if exits.get(&block_id) == Some(&state) {
            continue;
        }
        exits.insert(block_id, state.clone());
        for &successor in &block.successors {
            if queued.insert(successor) {
                pending.push_back(successor);
            }
        }
    }
    let blocks = program
        .blocks
        .iter()
        .map(|(&id, block)| BlockResult {
            unwind: match body.body[id].terminator.kind {
                u::TerminatorKind::Assert { on_unwind, .. }
                | u::TerminatorKind::Call { on_unwind, .. } => Some(on_unwind),
                _ => None,
            },
            block: id,
            successors: block.successors.clone(),
            incoming: incoming_traces.remove(&id).unwrap(),
            entry: entries[&id].graph(),
            exit: exits[&id].graph(),
        })
        .collect();
    Ok(Analysis {
        arguments: (1..=body.locals.arg_count)
            .map(charon_lib::ast::LocalId::from)
            .collect(),
        return_place: Place::Local(charon_lib::ast::LocalId::ZERO),
        loans: borrowck.loans(&program),
        unindexed_shared_reborrows: borrowck.unindexed_shared_reborrows,
        reference_regions: facts.reference_regions.clone(),
        outlives: facts.outlives.clone(),
        locals: program.locals.clone(),
        points: traces.into_values().flatten().collect(),
        blocks,
    })
}

fn reachable(program: &input::Program, start: u::BlockId, target: u::BlockId) -> bool {
    let mut pending = vec![start];
    let mut seen = BTreeSet::new();
    while let Some(block) = pending.pop() {
        if block == target {
            return true;
        }
        if seen.insert(block) {
            pending.extend(&program.blocks[&block].successors);
        }
    }
    false
}

/// DFS return edges close cycles, including irreducible CFGs.
fn backedges(program: &input::Program) -> BTreeSet<(u::BlockId, u::BlockId)> {
    let mut found = BTreeSet::new();
    let mut seen = BTreeSet::from([u::START_BLOCK_ID]);
    let mut active = seen.clone();
    let mut stack = vec![(u::START_BLOCK_ID, 0)];
    while let Some((id, next)) = stack.last_mut() {
        if let Some(&successor) = program.blocks[id].successors.get(*next) {
            *next += 1;
            if active.contains(&successor) {
                found.insert((*id, successor));
            } else if seen.insert(successor) {
                active.insert(successor);
                stack.push((successor, 0));
            }
        } else {
            active.remove(id);
            stack.pop();
        }
    }
    found
}

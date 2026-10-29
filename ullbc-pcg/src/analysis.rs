//! CFG worklist, block entry/exit states, and four-phase transfer.
use crate::{
    borrowck::BorrowContext, input, liveness, pcg::triple::TripleWalker, transfer::State, *,
};
use charon_lib::{ast::BodyBorrowckInfo, ullbc_ast as u};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(crate) fn run(body: &u::ExprBody, facts: &BodyBorrowckInfo) -> Result<Analysis, AnalysisError> {
    let program = input::lower(body)?;
    let borrowck = BorrowContext::new(body, facts, &program)?;
    let live = liveness::compute(&program);
    let mut entries = BTreeMap::from([(u::START_BLOCK_ID, State::new(body, &program)?)]);
    let mut exits = BTreeMap::new();
    let mut traces = BTreeMap::new();
    let mut pending = VecDeque::from([u::START_BLOCK_ID]);
    let mut queued = BTreeSet::from([u::START_BLOCK_ID]);
    while let Some(block_id) = pending.pop_front() {
        queued.remove(&block_id);
        let block = &program.blocks[&block_id];
        let mut state = entries[&block_id].clone();
        let mut points = Vec::new();
        for (statement_index, &op) in block.operations.iter().enumerate() {
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
            let changed = match entries.get_mut(&successor) {
                None => {
                    entries.insert(successor, state.clone());
                    true
                }
                Some(entry) => entry.join(&state)?,
            };
            if changed && queued.insert(successor) {
                pending.push_back(successor);
            }
        }
    }
    let blocks = program
        .blocks
        .iter()
        .map(|(&id, block)| BlockResult {
            block: id,
            successors: block.successors.clone(),
            entry: entries[&id].graph(),
            exit: exits[&id].graph(),
        })
        .collect();
    Ok(Analysis {
        locals: body
            .locals
            .locals
            .iter_enumerated()
            .map(|(id, l)| LocalInfo {
                id,
                name: l.name.clone(),
            })
            .collect(),
        points: traces.into_values().flatten().collect(),
        blocks,
    })
}

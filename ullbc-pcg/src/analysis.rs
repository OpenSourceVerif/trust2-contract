//! Same phase iteration and state threading as PCG engine::visit_all_phases.
use crate::{input, liveness, pcg::triple::TripleWalker, transfer::State, *};
use charon_lib::ullbc_ast as u;

pub(crate) fn run(body: &u::ExprBody) -> Result<Analysis, AnalysisError> {
    let program = input::lower(body)?;
    let live = liveness::compute(&program);
    let mut state = State::new(body, &program)?;
    let mut points = Vec::new();
    for (index, &(location, op)) in program.operations.iter().enumerate() {
        let initial = state.graph();
        let tw = TripleWalker::for_operation(op);
        let mut phases = Vec::new();
        for phase in EvalStmtPhase::phases() {
            phases.push(
                state
                    .phase(op, &tw, location, phase, &live.before[index])
                    .map_err(|mut e| {
                        e.phase = Some(phase);
                        e.at(location)
                    })?,
            );
        }
        points.push(PointResult {
            location,
            initial,
            phases,
            live_references_after: live.after[index].iter().copied().collect(),
        });
    }
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
        points,
    })
}

//! Backward local-reference liveness, solved on the ULLBC CFG.
use crate::{
    Location, Place,
    input::{Operand, Operation, Program, Rhs},
};
use charon_lib::ast::LocalId;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
pub(crate) struct Liveness {
    pub before: BTreeMap<Location, BTreeSet<LocalId>>,
    pub after: BTreeMap<Location, BTreeSet<LocalId>>,
}

pub(crate) fn compute(program: &Program) -> Liveness {
    let mut entries: BTreeMap<_, BTreeSet<LocalId>> = program
        .blocks
        .keys()
        .map(|id| (*id, BTreeSet::new()))
        .collect();
    let mut pending: VecDeque<_> = program.blocks.keys().copied().collect();
    let mut queued: BTreeSet<_> = pending.iter().copied().collect();
    let mut before = BTreeMap::new();
    let mut after = BTreeMap::new();
    while let Some(id) = pending.pop_front() {
        queued.remove(&id);
        let block = &program.blocks[&id];
        let mut live: BTreeSet<_> = block
            .successors
            .iter()
            .flat_map(|s| entries[s].iter().copied())
            .collect();
        for (statement_index, op) in block.operations.iter().enumerate().rev() {
            let location = Location {
                block: id,
                statement_index,
            };
            after.insert(location, live.clone());
            match op {
                Operation::Return if program.references.contains(&LocalId::ZERO) => {
                    live.insert(LocalId::ZERO);
                }
                Operation::Allocate(id) | Operation::Deallocate(id) => {
                    live.remove(id);
                }
                Operation::Assign(target, rhs) => {
                    match target {
                        Place::Local(id) => {
                            live.remove(id);
                        }
                        Place::Deref(id) => {
                            live.insert(*id);
                        }
                        Place::Field { .. } => {}
                    }
                    if let Rhs::Borrow(_, Place::Deref(id)) = rhs {
                        live.insert(*id);
                    }
                }
                _ => {}
            }
            for operand in op.operands() {
                if let Operand::Copy(p) | Operand::Move(p) = operand
                    && program.references.contains(&p.local())
                {
                    live.insert(p.local());
                }
            }
            before.insert(location, live.clone());
        }
        if entries[&id] != live {
            entries.insert(id, live);
            for predecessor in &block.predecessors {
                if queued.insert(*predecessor) {
                    pending.push_back(*predecessor);
                }
            }
        }
    }
    Liveness { before, after }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Block;
    use charon_lib::ullbc_ast::BlockId;

    #[test]
    fn backward_worklist_reaches_a_fixed_point_on_a_cycle() {
        let a = BlockId::from(0);
        let b = BlockId::from(1);
        let r = LocalId::from(2);
        let program = Program {
            locals: vec![],
            blocks: BTreeMap::from([
                (
                    a,
                    Block {
                        operations: vec![Operation::Goto],
                        successors: vec![b],
                        predecessors: vec![b],
                    },
                ),
                (
                    b,
                    Block {
                        operations: vec![
                            Operation::Assign(
                                Place::Local(LocalId::from(0)),
                                Rhs::Use(Operand::Copy(Place::Deref(r))),
                            ),
                            Operation::Goto,
                        ],
                        successors: vec![a],
                        predecessors: vec![a],
                    },
                ),
            ]),
            references: BTreeSet::from([r]),
            explicit_storage: BTreeSet::new(),
        };
        let live = compute(&program);
        assert_eq!(
            live.before[&Location {
                block: a,
                statement_index: 0
            }],
            BTreeSet::from([r])
        );
        assert_eq!(
            live.after[&Location {
                block: b,
                statement_index: 1
            }],
            BTreeSet::from([r])
        );
    }
}

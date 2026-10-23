use crate::{
    Place,
    input::{Operand, Operation, Program, Rhs},
};
use charon_lib::ast::LocalId;
use std::collections::BTreeSet;
pub(crate) struct Liveness {
    pub before: Vec<BTreeSet<LocalId>>,
    pub after: Vec<BTreeSet<LocalId>>,
}

/// Exact local use/def liveness for the supported straight-line fragment. It is
/// not a general replacement for NLL: lower() rejects every reference escape.
pub(crate) fn compute(program: &Program) -> Liveness {
    let mut live = BTreeSet::new();
    let mut before = vec![BTreeSet::new(); program.operations.len()];
    let mut after = vec![BTreeSet::new(); program.operations.len()];
    for (index, (_, op)) in program.operations.iter().enumerate().rev() {
        after[index] = live.clone();
        match op {
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
                }
                if let Rhs::Use(Operand::Copy(p) | Operand::Move(p)) = rhs {
                    let (Place::Local(id) | Place::Deref(id)) = p;
                    if program.references.contains(id) {
                        live.insert(*id);
                    }
                }
            }
            _ => {}
        }
        before[index] = live.clone();
    }
    Liveness { before, after }
}

// © 2023, ETH Zurich
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
// Adapted from pcg/src/pcg/triple.rs at 6fdb05e6377da5316e5388320d2ddf3d13082f6e.
// Only operations validated by input::lower reach these triples.
use crate::{
    BorrowKind, Capability, Place,
    input::{Operand, Operation, Rhs},
};
use charon_lib::ast::LocalId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlaceCondition {
    Capability(Place, Capability),
    RemoveCapability(Place),
    AllocateOrDeallocate(LocalId),
    Unalloc(LocalId),
    Return,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Triple {
    pub pre: PlaceCondition,
    pub post: Option<PlaceCondition>,
}
#[derive(Default)]
pub(crate) struct TripleWalker {
    pub operand_triples: Vec<Triple>,
    pub main_triples: Vec<Triple>,
}
impl TripleWalker {
    pub fn for_operation(op: &Operation) -> Self {
        use Capability as K;
        use PlaceCondition as C;
        let mut result = Self::default();
        for operand in op.operands() {
            result.operand_triples.extend(match *operand {
                Operand::Constant => None,
                Operand::Copy(p) => Some(Triple {
                    pre: C::Capability(p, K::Read),
                    post: None,
                }),
                Operand::Move(p) => Some(Triple {
                    pre: C::Capability(p, K::Exclusive),
                    post: Some(C::Capability(p, K::Write)),
                }),
            });
        }
        if let Operation::Assign(_, Rhs::Borrow(kind, p)) = op {
            result.operand_triples.push(match kind {
                BorrowKind::Shared => Triple {
                    pre: C::Capability(*p, K::Read),
                    post: Some(C::Capability(*p, K::Read)),
                },
                BorrowKind::Mutable => Triple {
                    pre: C::Capability(*p, K::Exclusive),
                    post: Some(C::RemoveCapability(*p)),
                },
            });
        }
        let triple = match *op {
            Operation::Assign(p, _) => Some(Triple {
                pre: C::Capability(p, K::Write),
                post: Some(C::Capability(p, K::Exclusive)),
            }),
            Operation::Allocate(id) => Some(Triple {
                pre: C::Unalloc(id),
                post: Some(C::AllocateOrDeallocate(id)),
            }),
            Operation::Deallocate(id) => Some(Triple {
                pre: C::AllocateOrDeallocate(id),
                post: Some(C::Unalloc(id)),
            }),
            Operation::Return => Some(Triple {
                pre: C::Return,
                post: Some(C::Capability(Place::Local(LocalId::ZERO), K::Write)),
            }),
            Operation::Nop | Operation::Inspect(_) | Operation::Goto => None,
        };
        result.main_triples.extend(triple);
        result
    }
}

//! 收集规格表达式依赖的局部变量，不依赖翻译上下文。

use charon_lib::ast::{LocalId, Operand, Rvalue};
use std::collections::HashSet;

use crate::llbc_to_whyml::error::{Error, Result};

/// 操作数至多依赖一个基础局部变量；全局量和常量没有局部依赖。
/// LLBC 中的索引和子切片投影已由 Charon 消除，字段投影只需追溯基础位置。
pub(super) fn operand_local(operand: &Operand) -> Option<LocalId> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => place.local_id(),
        Operand::Const(_) => None,
    }
}

/// 批量读取操作数依赖，由调用方决定如何收集和去重。
pub(super) fn operands_locals<'a>(
    operands: impl IntoIterator<Item = &'a Operand>,
) -> impl Iterator<Item = LocalId> {
    operands.into_iter().filter_map(operand_local)
}

/// 一批右值共用一个依赖集合，避免为每个操作数和捕获值分配临时集合。
pub(super) fn rvalues_locals<'a>(
    rvalues: impl IntoIterator<Item = &'a Rvalue>,
) -> Result<HashSet<LocalId>> {
    let mut locals = HashSet::new();
    for rvalue in rvalues {
        match rvalue {
            Rvalue::Use(operand, _) | Rvalue::UnaryOp(_, operand) => {
                locals.extend(operand_local(operand));
            }
            Rvalue::Ref { place, .. } => locals.extend(place.local_id()),
            Rvalue::RawPtr { .. } => return Err(Error::RawPointer),
            Rvalue::BinaryOp(_, lhs, rhs) => locals.extend(operands_locals([lhs, rhs])),
            Rvalue::NullaryOp(..) => (),
            Rvalue::Aggregate(_, operands) => locals.extend(operands_locals(operands)),
            Rvalue::Discriminant(..) | Rvalue::Len(..) => todo!(),
            Rvalue::Repeat(..) => unreachable!(),
        }
    }
    Ok(locals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use charon_lib::ast::{
        BinOp, BorrowKind, FieldId, FieldProjKind, GenericArgs, GlobalDeclId, GlobalDeclRef,
        NullOp, Place, ProjectionElem, RefKind, Ty, UnOp, WithRetag,
    };

    fn local(id: usize) -> Place {
        Place::new(LocalId::from(id), Ty::mk_unit())
    }

    #[test]
    fn operands_collect_projected_locals_and_ignore_constants_and_globals() {
        let projected = local(1)
            .project(ProjectionElem::Deref, Ty::mk_unit())
            .project(
                ProjectionElem::Field(FieldProjKind::Tuple(1), FieldId::ZERO),
                Ty::mk_unit(),
            );
        let global = Place::new_global(
            GlobalDeclRef {
                id: GlobalDeclId::ZERO,
                generics: Box::new(GenericArgs::empty()),
            },
            Ty::mk_unit(),
        );
        let operands = [
            Operand::Copy(projected),
            Operand::Move(local(2)),
            Operand::mk_const_unit(),
            Operand::Copy(global),
        ];
        assert_eq!(
            operands_locals(&operands).collect::<Vec<_>>(),
            vec![LocalId::from(1), LocalId::from(2)],
        );
    }

    #[test]
    fn batch_unions_dependencies_across_captures_and_aggregate_fields() {
        let mut aggregate = Rvalue::unit_value();
        let Rvalue::Aggregate(_, operands) = &mut aggregate else {
            unreachable!();
        };
        operands.extend([
            Operand::Copy(local(2)),
            Operand::Move(local(3)),
            Operand::Copy(local(3)),
            Operand::mk_const_unit(),
        ]);
        let captures = [
            Rvalue::Use(Operand::Copy(local(1)), WithRetag::No),
            Rvalue::BinaryOp(BinOp::Eq, Operand::Copy(local(1)), Operand::Move(local(2))),
            aggregate,
            Rvalue::UnaryOp(UnOp::Not, Operand::Copy(local(4))),
            Rvalue::Ref {
                place: local(5),
                kind: BorrowKind::Shared,
                ptr_metadata: Operand::mk_const_unit(),
            },
            Rvalue::NullaryOp(NullOp::SizeOf, Ty::mk_unit()),
        ];
        assert_eq!(
            rvalues_locals(&captures).unwrap(),
            (1..=5).map(LocalId::from).collect(),
        );
    }

    #[test]
    fn empty_and_constant_batches_have_no_dependencies() {
        assert!(rvalues_locals([]).unwrap().is_empty());
        assert!(
            rvalues_locals(&[
                Rvalue::unit_value(),
                Rvalue::Use(Operand::mk_const_unit(), WithRetag::No),
            ])
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn raw_pointer_in_batch_preserves_error() {
        let captures = [
            Rvalue::Use(Operand::Copy(local(1)), WithRetag::No),
            Rvalue::RawPtr {
                place: local(2),
                kind: RefKind::Shared,
                ptr_metadata: Operand::mk_const_unit(),
            },
        ];
        assert!(matches!(rvalues_locals(&captures), Err(Error::RawPointer)));
    }
}

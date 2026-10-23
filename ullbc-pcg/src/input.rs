//! Validate the closed supported fragment before using local reference liveness.
use crate::{AnalysisError as Error, BorrowKind, ErrorKind, Location, Place};
use charon_lib::{
    ast::{self as a, *},
    ullbc_ast as u,
};
use std::collections::BTreeSet;

pub(crate) fn unsupported(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::Unsupported, message)
}
fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::InvalidInput, message)
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Operand {
    Constant,
    Copy(Place),
    Move(Place),
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Rhs {
    Use(Operand),
    Borrow(BorrowKind, LocalId),
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Operation {
    Allocate(LocalId),
    Deallocate(LocalId),
    Assign(Place, Rhs),
    Nop,
    Goto,
    Return,
}
#[derive(Clone, Debug)]
pub(crate) struct Program {
    pub operations: Vec<(Location, Operation)>,
    pub references: BTreeSet<LocalId>,
    pub explicit_storage: BTreeSet<LocalId>,
}

fn scalar(ty: &Ty) -> bool {
    ty.is_unit()
        || matches!(
            ty.kind(),
            TyKind::Literal(
                LiteralTy::Int(_) | LiteralTy::UInt(_) | LiteralTy::Bool | LiteralTy::Char
            )
        )
}
fn compatible(left: &Ty, right: &Ty) -> bool {
    match (left.kind(), right.kind()) {
        // Body lifetime identities may differ after a reference copy/move.
        (TyKind::Ref(_, l, lm), TyKind::Ref(_, r, rm)) => lm == rm && l == r,
        _ => left == right,
    }
}
fn local_ty(locals: &Locals, id: LocalId) -> Result<&Ty, Error> {
    locals
        .locals
        .get(id)
        .map(|l| &l.ty)
        .ok_or_else(|| invalid(format!("unknown local {id}")))
}
fn place(p: &a::Place, locals: &Locals) -> Result<Place, Error> {
    match &p.kind {
        PlaceKind::Local(id) if compatible(&p.ty, local_ty(locals, *id)?) => Ok(Place::Local(*id)),
        PlaceKind::Local(_) => Err(invalid("local place type differs from declaration")),
        PlaceKind::Projection(base, ProjectionElem::Deref) => {
            let Place::Local(id) = place(base, locals)? else {
                return Err(unsupported("nested dereference"));
            };
            match local_ty(locals, id)?.kind() {
                TyKind::Ref(_, inner, _) if inner == &p.ty && scalar(inner) => Ok(Place::Deref(id)),
                _ => Err(unsupported(
                    "dereference must be of a direct reference to a scalar/unit",
                )),
            }
        }
        _ => Err(unsupported("field/index/global/raw-pointer place")),
    }
}
fn constant(c: &ConstantExpr) -> bool {
    match (&c.kind, c.ty.kind()) {
        (
            ConstantExprKind::Literal(Literal::Scalar(ScalarValue::Signed(kind, _))),
            TyKind::Literal(LiteralTy::Int(ty)),
        ) => kind == ty,
        (
            ConstantExprKind::Literal(Literal::Scalar(ScalarValue::Unsigned(kind, _))),
            TyKind::Literal(LiteralTy::UInt(ty)),
        ) => kind == ty,
        (ConstantExprKind::Literal(Literal::Bool(_)), TyKind::Literal(LiteralTy::Bool))
        | (ConstantExprKind::Literal(Literal::Char(_)), TyKind::Literal(LiteralTy::Char)) => true,
        (ConstantExprKind::Adt(None, fields), _) => fields.is_empty() && c.ty.is_unit(),
        _ => false,
    }
}
fn operand(op: &a::Operand, expected: &Ty, locals: &Locals) -> Result<Operand, Error> {
    match op {
        a::Operand::Const(c) if compatible(&c.ty, expected) && constant(c) => Ok(Operand::Constant),
        a::Operand::Copy(p) | a::Operand::Move(p) if compatible(&p.ty, expected) => {
            let lowered = place(p, locals)?;
            if matches!(op, a::Operand::Copy(_)) {
                if matches!(p.ty.kind(), TyKind::Ref(_, _, RefKind::Mut)) {
                    return Err(unsupported("copy of mutable reference"));
                }
                Ok(Operand::Copy(lowered))
            } else {
                Ok(Operand::Move(lowered))
            }
        }
        _ => Err(unsupported("operand constant/type/operation")),
    }
}
fn statement(stmt: &u::StatementKind, locals: &Locals) -> Result<Operation, Error> {
    Ok(match stmt {
        u::StatementKind::StorageLive(id) => {
            local_ty(locals, *id)?;
            Operation::Allocate(*id)
        }
        u::StatementKind::StorageDead(id) => {
            local_ty(locals, *id)?;
            Operation::Deallocate(*id)
        }
        u::StatementKind::Nop => Operation::Nop,
        u::StatementKind::Assign(dst, value) => {
            let target = place(dst, locals)?;
            let rhs = match value {
                // Charon marks ordinary assignments WithRetag::Yes. In this closed
                // direct-reference fragment retagging does not change capability
                // ownership; this analysis does not model pointer provenance tags.
                Rvalue::Use(op, _) => Rhs::Use(operand(op, &dst.ty, locals)?),
                Rvalue::Ref {
                    place: borrowed,
                    kind,
                    ptr_metadata,
                } => {
                    let Place::Local(x) = place(borrowed, locals)? else {
                        return Err(unsupported("reborrow"));
                    };
                    if !scalar(local_ty(locals, x)?) {
                        return Err(unsupported("borrow of non-scalar"));
                    }
                    let kind = match kind {
                        a::BorrowKind::Mut => BorrowKind::Mutable,
                        a::BorrowKind::Shared => BorrowKind::Shared,
                        _ => return Err(unsupported("two-phase/fake/unique-immutable borrow")),
                    };
                    let expected_kind = if kind == BorrowKind::Mutable {
                        RefKind::Mut
                    } else {
                        RefKind::Shared
                    };
                    if !matches!(dst.ty.kind(), TyKind::Ref(_, ty, k) if *k == expected_kind && ty == &borrowed.ty)
                    {
                        return Err(invalid("borrow destination type"));
                    }
                    if !matches!(target, Place::Local(_)) {
                        return Err(unsupported("projected reference destination"));
                    }
                    if !matches!(ptr_metadata, a::Operand::Const(c) if c.ty.is_unit() && constant(c))
                    {
                        return Err(unsupported("non-unit pointer metadata"));
                    }
                    Rhs::Borrow(kind, x)
                }
                Rvalue::Aggregate(AggregateKind::Adt(ty, None, None), fields)
                    if fields.is_empty()
                        && dst.ty.is_unit()
                        && Ty::new(TyKind::Adt(ty.clone())).is_unit() =>
                {
                    Rhs::Use(Operand::Constant)
                }
                _ => {
                    return Err(unsupported(
                        "rvalue (only use, direct borrow, and unit are supported)",
                    ));
                }
            };
            Operation::Assign(target, rhs)
        }
        _ => return Err(unsupported("statement kind")),
    })
}

pub(crate) fn lower(body: &u::ExprBody) -> Result<Program, Error> {
    if body.locals.locals.is_empty() || body.locals.arg_count >= body.locals.locals.len() {
        return Err(invalid("missing return local or invalid argument count"));
    }
    let mut references = BTreeSet::new();
    for (id, local) in body.locals.locals.iter_enumerated() {
        if local.index != id {
            return Err(invalid("local index differs from its position"));
        }
        if let TyKind::Ref(_, ty, _) = local.ty.kind() {
            if id.index() <= body.locals.arg_count {
                return Err(unsupported("reference arguments or return"));
            }
            if !scalar(ty) {
                return Err(unsupported("nested/aggregate reference"));
            }
            references.insert(id);
        } else if !scalar(&local.ty) {
            return Err(unsupported(format!("type of local {id}: {:?}", local.ty)));
        }
    }
    let mut operations = vec![];
    let mut explicit_storage = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut block = u::START_BLOCK_ID;
    loop {
        let location = Location {
            block,
            statement_index: 0,
        };
        if !visited.insert(block) {
            return Err(unsupported("loop: fixed-point analysis is not implemented").at(location));
        }
        let data = body
            .body
            .get(block)
            .ok_or_else(|| invalid("missing block").at(location))?;
        for (statement_index, stmt) in data.statements.iter().enumerate() {
            let location = Location {
                block,
                statement_index,
            };
            let op = statement(&stmt.kind, &body.locals).map_err(|e| e.at(location))?;
            if let Operation::Allocate(id) = op {
                explicit_storage.insert(id);
            }
            operations.push((location, op));
        }
        let location = Location {
            block,
            statement_index: data.statements.len(),
        };
        match data.terminator.kind {
            u::TerminatorKind::Goto { target } => { operations.push((location, Operation::Goto)); block = target; }
            u::TerminatorKind::Return => { operations.push((location, Operation::Return)); break; }
            _ => return Err(unsupported("terminator: only goto/return are supported; calls, branches and unwinding need separate rules").at(location)),
        }
    }
    Ok(Program {
        operations,
        references,
        explicit_storage,
    })
}

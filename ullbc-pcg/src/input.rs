//! Validate the closed supported fragment before using local reference liveness.
use crate::places::Places;
use crate::{AnalysisError as Error, BorrowKind, ErrorKind, Location, Place};
use charon_lib::{
    ast::{self as a, *},
    ullbc_ast as u,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(crate) fn unsupported(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::Unsupported, message)
}
pub(crate) fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorKind::InvalidInput, message)
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Operand {
    Constant,
    Copy(Place),
    Move(Place),
}
#[derive(Clone, Debug)]
pub(crate) enum Rhs {
    Use(Operand),
    Borrow(BorrowKind, Place),
    Operands(Vec<Operand>),
    Call {
        callee: FunDeclId,
        arguments: Vec<Operand>,
        result_source: Option<LocalId>,
    },
}
#[derive(Clone, Debug)]
pub(crate) enum Operation {
    Allocate(LocalId),
    Deallocate(LocalId),
    Assign(Place, Rhs),
    Nop,
    Goto,
    Inspect(Operand),
    Return,
}
impl Operation {
    pub fn operands(&self) -> &[Operand] {
        match self {
            Self::Assign(_, Rhs::Use(op)) | Self::Inspect(op) => std::slice::from_ref(op),
            Self::Assign(_, Rhs::Operands(ops) | Rhs::Call { arguments: ops, .. }) => ops,
            _ => &[],
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Program {
    pub blocks: BTreeMap<u::BlockId, Block>,
    pub locals: Vec<crate::LocalInfo>,
    pub references: BTreeSet<LocalId>,
    pub explicit_storage: BTreeSet<LocalId>,
}

#[derive(Clone, Debug)]
pub(crate) struct Block {
    pub operations: Vec<Operation>,
    pub successors: Vec<u::BlockId>,
    pub predecessors: Vec<u::BlockId>,
}

pub(crate) fn scalar(ty: &Ty) -> bool {
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
fn operand(op: &a::Operand, expected: &Ty, places: &Places) -> Result<Operand, Error> {
    match op {
        a::Operand::Const(c) if compatible(&c.ty, expected) && constant(c) => Ok(Operand::Constant),
        a::Operand::Copy(p) | a::Operand::Move(p) if compatible(&p.ty, expected) => {
            let lowered = places.lower(p)?;
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
fn statement(
    stmt: &u::StatementKind,
    locals: &Locals,
    places: &Places,
) -> Result<Operation, Error> {
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
        u::StatementKind::Assert { assert, .. } => {
            Operation::Inspect(operand(&assert.cond, assert.cond.ty(), places)?)
        }
        u::StatementKind::Assign(dst, value) => {
            let target = places.lower(dst)?;
            let rhs = match value {
                // Charon marks ordinary assignments WithRetag::Yes. In this closed
                // direct-reference fragment retagging does not change capability
                // ownership; this analysis does not model pointer provenance tags.
                Rvalue::Use(op, _) => Rhs::Use(operand(op, &dst.ty, places)?),
                Rvalue::Ref {
                    place: borrowed,
                    kind,
                    ptr_metadata,
                } => {
                    let x = places.lower(borrowed)?;
                    if !scalar(&borrowed.ty) {
                        return Err(unsupported("borrow of non-scalar"));
                    }
                    let kind = match kind {
                        a::BorrowKind::Mut => BorrowKind::Mutable,
                        a::BorrowKind::Shared => BorrowKind::Shared,
                        a::BorrowKind::TwoPhaseMut => {
                            return Err(unsupported(
                                "two-phase mutable borrow requires a rustc activation point",
                            ));
                        }
                        _ => return Err(unsupported("unsupported borrow kind")),
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
                Rvalue::BinaryOp(bin, a, b)
                    if scalar(a.ty())
                        && scalar(b.ty())
                        && match bin {
                            BinOp::AddChecked | BinOp::SubChecked | BinOp::MulChecked => {
                                dst.ty.as_tuple().is_some_and(|ts| {
                                    ts.len() == 2
                                        && &ts[0] == a.ty()
                                        && ts[1] == Ty::new(TyKind::Literal(LiteralTy::Bool))
                                })
                            }
                            BinOp::Offset => false,
                            _ => scalar(&dst.ty),
                        } =>
                {
                    Rhs::Operands(vec![
                        operand(a, a.ty(), places)?,
                        operand(b, b.ty(), places)?,
                    ])
                }
                Rvalue::UnaryOp(_, a) if scalar(a.ty()) && scalar(&dst.ty) => {
                    Rhs::Operands(vec![operand(a, a.ty(), places)?])
                }
                Rvalue::Aggregate(AggregateKind::Adt(t, None, None), fields)
                    if dst.ty.as_adt() == Some(t) && !matches!(dst.ty.kind(), TyKind::Ref(..)) =>
                {
                    let children: Vec<_> = places.locals[target.local().index()]
                        .children(target)
                        .collect();
                    if children.len() != fields.len() {
                        return Err(invalid("aggregate arity does not match declaration"));
                    }
                    Rhs::Operands(
                        fields
                            .iter()
                            .zip(children)
                            .map(|(operand_value, child)| {
                                operand(operand_value, places.ty(child), places)
                            })
                            .collect::<Result<_, _>>()?,
                    )
                }
                _ => return Err(unsupported("unsupported ULLBC rvalue")),
            };
            Operation::Assign(target, rhs)
        }
        _ => return Err(unsupported("statement kind")),
    })
}

/// For a direct call the signature determines the lifetime dependencies of the
/// result. A single scalar reference parameter with the same bound lifetime as
/// the returned reference gives one abstraction edge. It does not assert aliasing.
fn call_operation(
    call: &Call,
    krate: Option<&TranslatedCrate>,
    places: &Places,
) -> Result<Operation, Error> {
    let FnOperand::Regular(ptr) = &call.func else {
        return Err(unsupported("indirect call"));
    };
    let FnPtrKind::Fun(FunId::Regular(callee)) = *ptr.kind else {
        return Err(unsupported("trait or builtin call"));
    };
    let fun = krate
        .and_then(|k| k.fun_decls.get(callee))
        .ok_or_else(|| unsupported("callee signature is required"))?;
    let sig = &fun.signature;
    if sig.is_unsafe
        || sig.is_variadic
        || !fun.generics.types.is_empty()
        || !fun.generics.const_generics.is_empty()
        || !fun.generics.trait_clauses.is_empty()
        || !fun.generics.regions_outlive.is_empty()
    {
        return Err(unsupported("call requires a general function abstraction"));
    }
    if sig.inputs.len() != call.args.len() || !compatible(&sig.output, &call.dest.ty) {
        return Err(invalid("call does not match callee signature"));
    }
    let target = places.lower(&call.dest)?;
    let arguments = call
        .args
        .iter()
        .zip(&sig.inputs)
        .map(|(a, t)| operand(a, t, places))
        .collect::<Result<Vec<_>, _>>()?;
    let mut result_source = None;
    if let TyKind::Ref(return_region, _, _) = sig.output.kind() {
        let mut inputs = sig
            .inputs
            .iter()
            .enumerate()
            .filter_map(|(i, ty)| match ty.kind() {
                TyKind::Ref(r, _, _) => Some((i, r)),
                _ => None,
            });
        let Some((i, input_region)) = inputs.next() else {
            return Err(unsupported("reference return without an input lifetime"));
        };
        if inputs.next().is_some()
            || input_region != return_region
            || !matches!(return_region, Region::Var(_))
        {
            return Err(unsupported(
                "call needs multiple or independent lifetime projections",
            ));
        }
        let (Operand::Move(Place::Local(id)) | Operand::Copy(Place::Local(id))) = arguments[i]
        else {
            return Err(unsupported(
                "reference call operand must identify a local projection",
            ));
        };
        result_source = Some(id);
    }
    Ok(Operation::Assign(
        target,
        Rhs::Call {
            callee,
            arguments,
            result_source,
        },
    ))
}

pub(crate) fn lower(body: &u::ExprBody, krate: Option<&TranslatedCrate>) -> Result<Program, Error> {
    if body.locals.locals.is_empty() || body.locals.arg_count >= body.locals.locals.len() {
        return Err(invalid("missing return local or invalid argument count"));
    }
    let places = Places::new(&body.locals, krate)?;
    let mut references = BTreeSet::new();
    for (id, local) in body.locals.locals.iter_enumerated() {
        if local.index != id {
            return Err(invalid("local index differs from its position"));
        }
        if let TyKind::Ref(_, ty, _) = local.ty.kind() {
            if !scalar(ty) {
                return Err(unsupported("nested/aggregate reference"));
            }
            references.insert(id);
        }
    }
    let mut blocks = BTreeMap::new();
    let mut explicit_storage = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut pending = VecDeque::from([u::START_BLOCK_ID]);
    while let Some(block) = pending.pop_front() {
        let location = Location {
            block,
            statement_index: 0,
        };
        if !visited.insert(block) {
            continue;
        }
        let data = body
            .body
            .get(block)
            .ok_or_else(|| invalid("missing block").at(location))?;
        let mut operations = Vec::new();
        for (statement_index, stmt) in data.statements.iter().enumerate() {
            let location = Location {
                block,
                statement_index,
            };
            let op = statement(&stmt.kind, &body.locals, &places).map_err(|e| e.at(location))?;
            if let Operation::Allocate(id) = op {
                explicit_storage.insert(id);
            }
            operations.push(op);
        }
        let location = Location {
            block,
            statement_index: data.statements.len(),
        };
        let (op, mut successors) = match &data.terminator.kind {
            u::TerminatorKind::Goto { target }
            | u::TerminatorKind::ContractAssert { target, .. } => (Operation::Goto, vec![*target]),
            u::TerminatorKind::Return => (Operation::Return, vec![]),
            u::TerminatorKind::Abort(_) | u::TerminatorKind::UnwindResume => {
                (Operation::Nop, vec![])
            }
            u::TerminatorKind::Switch { discr, .. } => (
                Operation::Inspect(
                    operand(discr, discr.ty(), &places).map_err(|e| e.at(location))?,
                ),
                data.targets().into_vec(),
            ),
            u::TerminatorKind::Call { call, target, .. } => (
                call_operation(call, krate, &places).map_err(|e| e.at(location))?,
                vec![*target],
            ),
            u::TerminatorKind::Assert { assert, target, .. } => (
                Operation::Inspect(
                    operand(&assert.cond, assert.cond.ty(), &places).map_err(|e| e.at(location))?,
                ),
                vec![*target],
            ),
            _ => return Err(unsupported("unsupported ULLBC terminator").at(location)),
        };
        operations.push(op);
        successors.sort_unstable();
        successors.dedup();
        pending.extend(successors.iter().copied());
        blocks.insert(
            block,
            Block {
                operations,
                successors,
                predecessors: Vec::new(),
            },
        );
    }
    let edges: Vec<_> = blocks
        .iter()
        .flat_map(|(id, block)| block.successors.iter().map(move |target| (*id, *target)))
        .collect();
    for (source, target) in edges {
        blocks.get_mut(&target).unwrap().predecessors.push(source);
    }
    Ok(Program {
        locals: places.locals,
        blocks,
        references,
        explicit_storage,
    })
}

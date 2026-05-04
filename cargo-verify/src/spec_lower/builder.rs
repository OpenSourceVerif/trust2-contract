//! Stateful LLBC evaluator used to lower spec closures into `spec_ast` terms.

use crate::spec_ast::{
    BinOp as PBinOp, Ident as PIdent, LiteralConst as PLiteralConst, Qualid as PQualid,
    Quant as PQuant, Term as PTerm, TermDesc as PTermDesc,
};
use charon_lib::{ast::*, ids::IndexVec};

use super::{
    closure::{
        ClosureRole, ClosureValue, LoweredClosure, derive_closure_binders,
        resolve_closure_call_fun_id,
    },
    errors::SpecLowerError,
    naming::{
        is_closure_type, local_ident_term, map_binop, name_to_qualid, name_to_string,
        sanitize_local_name,
    },
};

/// Intermediate value tracked for each local while evaluating LLBC statements.
#[derive(Clone)]
enum Value {
    /// Already-lowered term.
    Term(PTerm),
    /// Closure type and captured, already-lowered terms.
    Closure(ClosureValue),
}

impl Value {
    /// Convert the intermediate value back into a plain term.
    ///
    /// Closure values are reified as tuples of capture terms when they flow
    /// into a context that only accepts plain terms.
    fn as_term(&self, span: Span) -> PTerm {
        match self {
            Value::Term(term) => term.clone(),
            Value::Closure(closure) => PTerm::new(span, PTermDesc::Tuple(closure.captures.clone())),
        }
    }
}

/// Stateful lowering context for one LLBC expression body.
#[derive(Clone)]
pub(super) struct TermBuilder<'a> {
    /// Crate-level declaration tables used to resolve referenced items.
    krate: &'a TranslatedCrate,
    /// Local declarations for the body currently being evaluated.
    locals: &'a Locals,
    // `function_name` and `spec_kind` are used for error reporting.
    /// Fully qualified function name shown in lowering errors.
    function_name: Box<str>,
    /// Current logical spec fragment shown in lowering errors.
    spec_kind: Box<str>,

    /// Maps `LocalId`s to their current value if known.
    values: IndexVec<LocalId, Option<Value>>,
}

impl<'a> TermBuilder<'a> {
    /// Create a builder initialized for lowering one top-level function spec block.
    ///
    /// Initializes argument and return locals as named term identifiers so later
    /// expression evaluation can reference them directly in the lowered AST.
    pub(super) fn new_for_function(
        krate: &'a TranslatedCrate,
        locals: &'a Locals,
        function_name: Box<str>,
        spec_kind: Box<str>,
    ) -> Self {
        let mut values = locals.locals.map_ref(|_| None);
        for local in &locals.locals {
            if !locals.is_return_or_arg(local.index) {
                continue;
            }
            let name = if local.index.is_zero() {
                "result".to_owned()
            } else {
                sanitize_local_name(local.name.as_deref(), local.index)
            };
            values[local.index] = Some(Value::Term(local_ident_term(local.span, &name)));
        }
        Self {
            krate,
            locals,
            values,
            function_name,
            spec_kind,
        }
    }

    /// Build a structured lowering error enriched with function/spec context.
    pub(super) fn error<T>(&self, span: Span, reason: String) -> Result<T, SpecLowerError> {
        Err(SpecLowerError {
            function_name: self.function_name.clone(),
            spec_kind: self.spec_kind.clone(),
            span,
            reason: reason.into_boxed_str(),
        })
    }

    /// Evaluate a sequence of LLBC statements in order and update local state.
    pub(super) fn eval_statements(
        &mut self,
        statements: &[llbc_ast::Statement],
    ) -> Result<(), SpecLowerError> {
        for statement in statements {
            self.eval_statement(statement)?;
        }
        Ok(())
    }

    /// Evaluate an operand and require it to be a closure aggregate.
    pub(super) fn eval_operand_as_closure(
        &mut self,
        operand: &Operand,
        span: Span,
    ) -> Result<ClosureValue, SpecLowerError> {
        match self.eval_operand(operand, span)? {
            Value::Closure(closure) => Ok(closure),
            _ => self.error(
                span,
                "expected closure operand when lowering specification call".to_owned(),
            ),
        }
    }

    /// Lower a closure value by evaluating its call body in a nested builder.
    pub(super) fn lower_closure_value(
        &self,
        closure: &ClosureValue,
        role: ClosureRole,
        span: Span,
    ) -> Result<LoweredClosure, SpecLowerError> {
        let call_fun_id = resolve_closure_call_fun_id(
            self.krate,
            closure.type_id,
            &self.function_name,
            &self.spec_kind,
            span,
        )?;
        let Some(call_fun) = self.krate.fun_decls.get(call_fun_id) else {
            return self.error(
                span,
                format!("missing closure call function: {}", call_fun_id.index()),
            );
        };
        let Some(call_body) = call_fun.body.as_structured() else {
            return self.error(
                span,
                "closure call function has no structured body".to_owned(),
            );
        };

        let binders = derive_closure_binders(call_body, &call_fun.signature, role, span);
        let binder_terms = binders
            .iter()
            .map(|binder| {
                let name = binder
                    .id
                    .as_ref()
                    .map(|id| id.name.clone())
                    .unwrap_or_else(|| "_".to_owned());
                local_ident_term(span, &name)
            })
            .collect::<Vec<_>>();

        let mut builder = Self {
            krate: self.krate,
            locals: &call_body.locals,
            values: call_body.locals.locals.map_ref(|_| None),
            function_name: name_to_string(&call_fun.item_meta.name).into_boxed_str(),
            spec_kind: format!("{}/{}", self.spec_kind, role.label()).into_boxed_str(),
        };

        if call_body.locals.locals.get(LocalId::new(1)).is_some() {
            builder.set_local_value(LocalId::new(1), Value::Closure(closure.clone()), span)?;
        }
        if call_body.locals.locals.get(LocalId::new(2)).is_some() {
            builder.set_local_value(
                LocalId::new(2),
                Value::Term(PTerm::new(span, PTermDesc::Tuple(binder_terms))),
                span,
            )?;
        }

        builder.eval_statements(&call_body.body.statements)?;
        let term = builder.get_return_term(call_body.span)?;
        Ok(LoweredClosure { binders, term })
    }

    /// Read the current value of a local.
    ///
    /// If the local has no explicit value yet but exists in the local table, this
    /// falls back to a term identifier derived from the local name.
    fn get_local_value(&self, local: LocalId, span: Span) -> Result<Value, SpecLowerError> {
        match self.values.get(local).and_then(Clone::clone) {
            Some(value) => Ok(value),
            None => {
                if let Some(local_decl) = self.locals.locals.get(local) {
                    let name = sanitize_local_name(local_decl.name.as_deref(), local);
                    Ok(Value::Term(local_ident_term(span, &name)))
                } else {
                    self.error(
                        span,
                        format!(
                            "unknown local referenced in spec lowering: _{}",
                            local.index()
                        ),
                    )
                }
            }
        }
    }

    /// Assign a lowered value to a local slot.
    fn set_local_value(
        &mut self,
        local: LocalId,
        value: Value,
        span: Span,
    ) -> Result<(), SpecLowerError> {
        let Some(slot) = self.values.get_mut(local) else {
            return self.error(
                span,
                format!(
                    "assignment to unknown local in spec lowering: _{}",
                    local.index()
                ),
            );
        };
        *slot = Some(value);
        Ok(())
    }

    /// Evaluate one LLBC statement and apply its effect to builder state.
    ///
    /// Only the subset used in lowered spec closures is supported.
    fn eval_statement(&mut self, statement: &llbc_ast::Statement) -> Result<(), SpecLowerError> {
        use llbc_ast::StatementKind;
        match &statement.kind {
            StatementKind::Assign(place, rvalue) => {
                let value = self.eval_rvalue(rvalue, statement.span)?;
                self.assign_place(place, value, statement.span)
            }
            StatementKind::Call(call) => {
                let value = self.eval_call(call, statement.span)?;
                self.assign_place(&call.dest, value, statement.span)
            }
            StatementKind::Switch(llbc_ast::Switch::If(cond, then_block, else_block)) => {
                let cond = self.eval_operand_as_term(cond, statement.span)?;
                let mut then_builder = self.clone();
                then_builder.eval_statements(&then_block.statements)?;
                let then_term = then_builder.get_return_term(statement.span)?;
                let mut else_builder = self.clone();
                else_builder.eval_statements(&else_block.statements)?;
                let else_term = else_builder.get_return_term(statement.span)?;
                self.set_local_value(
                    LocalId::ZERO,
                    Value::Term(PTerm::new(
                        statement.span,
                        PTermDesc::If(Box::new(cond), Box::new(then_term), Box::new(else_term)),
                    )),
                    statement.span,
                )
            }
            StatementKind::StorageDead(local) => {
                if let Some(slot) = self.values.get_mut(*local) {
                    *slot = None;
                }
                Ok(())
            }
            StatementKind::StorageLive(_)
            | StatementKind::PlaceMention(_)
            | StatementKind::SetDiscriminant(_, _)
            | StatementKind::CopyNonOverlapping(_)
            | StatementKind::Drop(_, _, _)
            | StatementKind::Assert { .. }
            | StatementKind::Return
            | StatementKind::Break(_)
            | StatementKind::Continue(_)
            | StatementKind::Nop
            | StatementKind::Abort(_) => Ok(()),
            StatementKind::Switch(_) | StatementKind::Loop(_) => self.error(
                statement.span,
                "unsupported control flow in spec lowering (only simple `if` is supported)"
                    .to_owned(),
            ),
            StatementKind::Error(msg) => self.error(
                statement.span,
                format!("error statement encountered while lowering spec: {msg}"),
            ),
        }
    }

    /// Fetch the lowered return local (`_0`) as a term.
    fn get_return_term(&self, span: Span) -> Result<PTerm, SpecLowerError> {
        Ok(self.get_local_value(LocalId::ZERO, span)?.as_term(span))
    }

    /// Assign a value to a place.
    ///
    /// Current lowering only supports assignment to local places.
    fn assign_place(
        &mut self,
        place: &Place,
        value: Value,
        span: Span,
    ) -> Result<(), SpecLowerError> {
        match &place.kind {
            PlaceKind::Local(local) => self.set_local_value(*local, value, span),
            _ => self.error(
                span,
                "unsupported assignment target in spec lowering (only locals are supported)"
                    .to_owned(),
            ),
        }
    }

    /// Evaluate a function call and return its lowered value.
    ///
    /// Dispatches to builtin or regular call lowering depending on the callee id.
    fn eval_call(&mut self, call: &Call, span: Span) -> Result<Value, SpecLowerError> {
        let FnOperand::Regular(fn_ptr) = &call.func else {
            return self.error(
                span,
                "dynamic function call is unsupported in spec lowering".to_owned(),
            );
        };

        match fn_ptr.kind.as_ref() {
            FnPtrKind::Fun(FunId::Builtin(builtin)) => {
                self.eval_builtin_call(*builtin, &call.args, span)
            }
            FnPtrKind::Fun(FunId::Regular(fun_id)) => {
                self.eval_regular_call(*fun_id, &call.args, span)
            }
            FnPtrKind::Trait(_, _, fun_id) => self.eval_regular_call(*fun_id, &call.args, span),
        }
    }

    /// Lower a builtin specification/runtime helper call into a term value.
    ///
    /// Handles spec-specific builtins (`SpecForall`, `SpecExists`, etc.) and falls
    /// back to symbolic application for other builtins.
    fn eval_builtin_call(
        &mut self,
        builtin: BuiltinFunId,
        args: &[Operand],
        span: Span,
    ) -> Result<Value, SpecLowerError> {
        match builtin {
            BuiltinFunId::SpecImplies => {
                if args.len() != 2 {
                    return self.error(
                        span,
                        format!("SpecImplies expects 2 arguments, found {}", args.len()),
                    );
                }
                let lhs = self.eval_operand_as_term(&args[0], span)?;
                let rhs = self.eval_operand_as_term(&args[1], span)?;
                Ok(Value::Term(PTerm::new(
                    span,
                    PTermDesc::BinOp(Box::new(lhs), PBinOp::Implies, Box::new(rhs)),
                )))
            }
            BuiltinFunId::SpecOld => {
                if args.len() != 1 {
                    return self.error(
                        span,
                        format!("SpecOld expects 1 argument, found {}", args.len()),
                    );
                }
                let arg = self.eval_operand_as_term(&args[0], span)?;
                Ok(Value::Term(PTerm::new(
                    span,
                    PTermDesc::At(Box::new(arg), PIdent::new("old", span)),
                )))
            }
            BuiltinFunId::SpecForall => {
                if args.len() != 1 {
                    return self.error(
                        span,
                        format!(
                            "SpecForall expects 1 closure argument, found {}",
                            args.len()
                        ),
                    );
                }
                let closure = self.eval_operand_as_closure(&args[0], span)?;
                let lowered = self.lower_closure_value(&closure, ClosureRole::Forall, span)?;
                Ok(Value::Term(PTerm::new(
                    span,
                    PTermDesc::Quant(
                        PQuant::Forall,
                        lowered.binders,
                        Vec::new(),
                        Box::new(lowered.term),
                    ),
                )))
            }
            BuiltinFunId::SpecExists => {
                if args.len() != 1 {
                    return self.error(
                        span,
                        format!(
                            "SpecExists expects 1 closure argument, found {}",
                            args.len()
                        ),
                    );
                }
                let closure = self.eval_operand_as_closure(&args[0], span)?;
                let lowered = self.lower_closure_value(&closure, ClosureRole::Exists, span)?;
                Ok(Value::Term(PTerm::new(
                    span,
                    PTermDesc::Quant(
                        PQuant::Exists,
                        lowered.binders,
                        Vec::new(),
                        Box::new(lowered.term),
                    ),
                )))
            }
            BuiltinFunId::SpecAssert => {
                if args.len() != 1 {
                    return self.error(
                        span,
                        format!(
                            "SpecAssert expects 1 closure argument, found {}",
                            args.len()
                        ),
                    );
                }
                let closure = self.eval_operand_as_closure(&args[0], span)?;
                let lowered = self.lower_closure_value(&closure, ClosureRole::Assert, span)?;
                if !lowered.binders.is_empty() {
                    return self.error(
                        span,
                        "SpecAssert closure must not bind parameters".to_owned(),
                    );
                }
                Ok(Value::Term(lowered.term))
            }
            BuiltinFunId::SpecAssume => {
                if args.len() != 1 {
                    return self.error(
                        span,
                        format!(
                            "SpecAssume expects 1 closure argument, found {}",
                            args.len()
                        ),
                    );
                }
                let closure = self.eval_operand_as_closure(&args[0], span)?;
                let lowered = self.lower_closure_value(&closure, ClosureRole::Assume, span)?;
                if !lowered.binders.is_empty() {
                    return self.error(
                        span,
                        "SpecAssume closure must not bind parameters".to_owned(),
                    );
                }
                Ok(Value::Term(lowered.term))
            }
            BuiltinFunId::SpecEntry
            | BuiltinFunId::SpecPrecondition
            | BuiltinFunId::SpecPostcondition => self.error(
                span,
                format!("unexpected builtin call in lowered LLBC specs: {builtin:?}"),
            ),
            _ => {
                let args = args
                    .iter()
                    .map(|arg| self.eval_operand_as_term(arg, span))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Value::Term(PTerm::new(
                    span,
                    PTermDesc::IdApp(PQualid::ident(format!("{builtin:?}"), span), args),
                )))
            }
        }
    }

    /// Lower a non-builtin function call into an identifier application term.
    ///
    /// User-defined local functions are rejected in the current V1 lowering.
    fn eval_regular_call(
        &mut self,
        fun_id: FunDeclId,
        args: &[Operand],
        span: Span,
    ) -> Result<Value, SpecLowerError> {
        let Some(fun_decl) = self.krate.fun_decls.get(fun_id) else {
            return self.error(
                span,
                format!("unknown function id in spec call: {}", fun_id.index()),
            );
        };
        if fun_decl.item_meta.is_local {
            return self.error(
                span,
                format!(
                    "user-defined function calls are unsupported in V1 spec lowering: `{}`",
                    name_to_string(&fun_decl.item_meta.name)
                ),
            );
        }
        let args = args
            .iter()
            .map(|arg| self.eval_operand_as_term(arg, span))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Value::Term(PTerm::new(
            span,
            PTermDesc::IdApp(name_to_qualid(&fun_decl.item_meta.name, span), args),
        )))
    }

    /// Evaluate an rvalue and convert it to a lowered value.
    fn eval_rvalue(&mut self, rvalue: &Rvalue, span: Span) -> Result<Value, SpecLowerError> {
        match rvalue {
            Rvalue::Use(op) => self.eval_operand(op, span),
            Rvalue::Ref { place, .. } | Rvalue::RawPtr { place, .. } => {
                self.eval_place(place, span)
            }
            Rvalue::BinaryOp(binop, lhs, rhs) => {
                let lhs = self.eval_operand_as_term(lhs, span)?;
                let rhs = self.eval_operand_as_term(rhs, span)?;
                match binop {
                    BinOp::AddChecked | BinOp::SubChecked | BinOp::MulChecked => {
                        let op = match binop {
                            BinOp::AddChecked => PBinOp::Add,
                            BinOp::SubChecked => PBinOp::Sub,
                            BinOp::MulChecked => PBinOp::Mul,
                            _ => unreachable!(),
                        };
                        Ok(Value::Term(PTerm::new(
                            span,
                            PTermDesc::Tuple(vec![
                                PTerm::new(
                                    span,
                                    PTermDesc::BinOp(Box::new(lhs), op, Box::new(rhs)),
                                ),
                                PTerm::new(span, PTermDesc::False),
                            ]),
                        )))
                    }
                    _ => {
                        let Some(binop) = map_binop(*binop) else {
                            return self.error(
                                span,
                                format!("unsupported binary operator in spec lowering: {binop:?}"),
                            );
                        };
                        Ok(Value::Term(PTerm::new(
                            span,
                            PTermDesc::BinOp(Box::new(lhs), binop, Box::new(rhs)),
                        )))
                    }
                }
            }
            Rvalue::UnaryOp(unop, arg) => match unop {
                UnOp::Not => {
                    let arg = self.eval_operand_as_term(arg, span)?;
                    Ok(Value::Term(PTerm::new(span, PTermDesc::Not(Box::new(arg)))))
                }
                UnOp::Neg(_) => {
                    let arg = self.eval_operand_as_term(arg, span)?;
                    Ok(Value::Term(PTerm::new(
                        span,
                        PTermDesc::IdApp(PQualid::ident("neg", span), vec![arg]),
                    )))
                }
                UnOp::Cast(cast) => {
                    let arg = self.eval_operand_as_term(arg, span)?;
                    let cast_ty = match cast {
                        CastKind::Scalar(_, ty) => TyKind::Literal(*ty).into_ty(),
                        CastKind::RawPtr(_, ty)
                        | CastKind::FnPtr(_, ty)
                        | CastKind::Unsize(_, ty, _)
                        | CastKind::Transmute(_, ty)
                        | CastKind::Concretize(_, ty) => ty.clone(),
                    };
                    Ok(Value::Term(PTerm::new(
                        span,
                        PTermDesc::Cast(Box::new(arg), cast_ty),
                    )))
                }
            },
            Rvalue::Aggregate(kind, args) => {
                let args = args
                    .iter()
                    .map(|arg| self.eval_operand_as_term(arg, span))
                    .collect::<Result<Vec<_>, _>>()?;
                match kind {
                    AggregateKind::Adt(type_ref, _, _)
                        if matches!(type_ref.id, TypeId::Adt(_))
                            && is_closure_type(self.krate, type_ref.id) =>
                    {
                        let TypeId::Adt(type_id) = type_ref.id else {
                            unreachable!();
                        };
                        Ok(Value::Closure(ClosureValue {
                            type_id,
                            captures: args,
                        }))
                    }
                    AggregateKind::Adt(_, _, _) | AggregateKind::Array(_, _) => {
                        Ok(Value::Term(PTerm::new(span, PTermDesc::Tuple(args))))
                    }
                    AggregateKind::RawPtr(_, _) => Ok(Value::Term(PTerm::new(
                        span,
                        PTermDesc::IdApp(PQualid::ident("raw_ptr", span), args),
                    ))),
                }
            }
            Rvalue::Len(place, _, _) => {
                let arg = self.eval_place(place, span)?.as_term(span);
                Ok(Value::Term(PTerm::new(
                    span,
                    PTermDesc::IdApp(PQualid::ident("len", span), vec![arg]),
                )))
            }
            Rvalue::Repeat(value, _, len) => {
                let value = self.eval_operand_as_term(value, span)?;
                let len = self.constant_to_literal(len, span)?;
                Ok(Value::Term(PTerm::new(
                    span,
                    PTermDesc::IdApp(
                        PQualid::ident("repeat", span),
                        vec![value, PTerm::new(span, PTermDesc::Const(len))],
                    ),
                )))
            }
            Rvalue::ShallowInitBox(value, _) => self.eval_operand(value, span),
            Rvalue::NullaryOp(_, _) | Rvalue::Discriminant(_) => {
                self.error(span, "unsupported rvalue in spec lowering".to_owned())
            }
        }
    }

    /// Evaluate an operand as either a plain term or a closure value.
    fn eval_operand(&mut self, operand: &Operand, span: Span) -> Result<Value, SpecLowerError> {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => self.eval_place(place, span),
            Operand::Const(constant) => Ok(Value::Term(PTerm::new(
                span,
                PTermDesc::Const(self.constant_to_literal(constant, span)?),
            ))),
        }
    }

    /// Evaluate an operand and require it to be representable as a term.
    fn eval_operand_as_term(
        &mut self,
        operand: &Operand,
        span: Span,
    ) -> Result<PTerm, SpecLowerError> {
        Ok(self.eval_operand(operand, span)?.as_term(span))
    }

    /// Evaluate a place expression into a lowered value.
    fn eval_place(&mut self, place: &Place, span: Span) -> Result<Value, SpecLowerError> {
        match &place.kind {
            PlaceKind::Local(local) => self.get_local_value(*local, span),
            PlaceKind::Projection(base, elem) => {
                let base = self.eval_place(base, span)?;
                self.apply_projection(base, elem, span)
            }
            PlaceKind::Global(global) => {
                let Some(decl) = self.krate.global_decls.get(global.id) else {
                    return self.error(
                        span,
                        format!("unknown global in spec lowering: {}", global.id.index()),
                    );
                };
                Ok(Value::Term(PTerm::new(
                    span,
                    PTermDesc::Ident(name_to_qualid(&decl.item_meta.name, span)),
                )))
            }
        }
    }

    /// Apply one projection element to a previously-evaluated base value.
    fn apply_projection(
        &mut self,
        base: Value,
        elem: &ProjectionElem,
        span: Span,
    ) -> Result<Value, SpecLowerError> {
        match elem {
            ProjectionElem::Deref => Ok(base),
            ProjectionElem::Field(_, field_id) => {
                let index = field_id.index();
                match base {
                    Value::Closure(closure) => closure
                        .captures
                        .get(index)
                        .cloned()
                        .map(Value::Term)
                        .ok_or_else(|| SpecLowerError {
                            function_name: self.function_name.clone(),
                            spec_kind: self.spec_kind.clone(),
                            span,
                            reason: format!(
                                "closure capture index out of bounds in projection: {}",
                                field_id.index()
                            )
                            .into_boxed_str(),
                        }),
                    Value::Term(term) => {
                        let PTermDesc::Tuple(fields) = &term.desc else {
                            return self.error(
                                span,
                                "field projection on non-tuple term during spec lowering"
                                    .to_owned(),
                            );
                        };
                        let Some(field) = fields.get(index) else {
                            return self.error(
                                span,
                                format!(
                                    "tuple field projection out of bounds: {}",
                                    field_id.index()
                                ),
                            );
                        };
                        Ok(Value::Term(field.clone()))
                    }
                }
            }
            ProjectionElem::PtrMetadata => Ok(Value::Term(PTerm::new(
                span,
                PTermDesc::Const(PLiteralConst::Unit),
            ))),
            // the LLBC pass is in `charon/charon/src/transform/simplify_output/index_to_function_calls.rs`
            ProjectionElem::Index { .. } | ProjectionElem::Subslice { .. } => panic!("indexing and subslicing should already eliminated on LLBC side")
        }
    }

    /// Convert a constant expression to a literal constant used by `PTermDesc::Const`.
    fn constant_to_literal(
        &self,
        constant: &ConstantExpr,
        span: Span,
    ) -> Result<PLiteralConst, SpecLowerError> {
        match &constant.kind {
            ConstantExprKind::Literal(Literal::Bool(v)) => Ok(PLiteralConst::Bool(*v)),
            ConstantExprKind::Literal(Literal::Char(v)) => Ok(PLiteralConst::Char(*v)),
            ConstantExprKind::Literal(Literal::Str(v)) => Ok(PLiteralConst::Str(v.clone().into())),
            ConstantExprKind::Literal(Literal::Scalar(v)) => Ok(PLiteralConst::Int(*v)),
            ConstantExprKind::Adt(None, fields) if fields.is_empty() && constant.ty.is_unit() => {
                Ok(PLiteralConst::Unit)
            }
            _ => self.error(
                span,
                format!(
                    "unsupported non-literal constant in spec lowering: {:?}",
                    constant.kind.variant_name()
                ),
            ),
        }
    }
}

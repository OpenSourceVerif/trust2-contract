//! Lower trust2-contract `spec` blocks from LLBC closure form to a Why3-style ptree AST.

use crate::{
    ast::*,
    errors::Level,
    ids::IndexVec,
    transform::{TransformCtx, ctx::LlbcPass},
};

/// Lowering pass from `FunSpecs` blocks to [`PSpec`].
///
/// Adapter for `LlbcPass`.
pub struct Transform;

impl LlbcPass for Transform {
    fn transform_ctx(&self, ctx: &mut TransformCtx) {
        // use `ctx.for_each_body` with `transform_ctx` to avoid `transform_function` moving large [`FunDecl`] > 1000 Bytes.
        // we only need &mut on body and ctx.

        ctx.for_each_body(|ctx, body| {
            let fun_id = match ctx.errors.borrow().def_id {
                Some(ItemId::Fun(fun_id)) => fun_id,
                _ => unreachable!("expected function def_id while lowering specs"),
            };
            let decl = ctx
                .translated
                .fun_decls
                .get(fun_id)
                .expect("missing function declaration while lowering specs");
            self.log_before_body(ctx, &decl.item_meta.name, body);
            let function_name = name_to_string(&decl.item_meta.name);

            let Some(body) = body.as_structured_mut() else {
                return;
            };

            if body.specs.preconditions.is_empty() && body.specs.postconditions.is_empty() {
                body.lowered_specs = None;
                return;
            }

            match lower_specs(&ctx.translated, &function_name, body) {
                Ok(spec) => {
                    body.lowered_specs = Some(spec);
                }
                Err(errors) => {
                    for error in &errors {
                        let msg = format!(
                            "spec lowering failed (function=`{}`, spec_kind=`{}`): {}",
                            error.function_name, error.spec_kind, error.reason
                        );
                        let _ = ctx.errors.borrow().display_error(
                            &ctx.translated,
                            error.span,
                            Level::ERROR,
                            msg,
                        );
                    }
                    panic!(
                        "spec lowering failed for function `{}` ({} error(s))",
                        function_name,
                        errors.len()
                    );
                }
            }
        });
    }
}

#[derive(Debug, Clone)]
struct SpecLowerError {
    function_name: Box<str>,
    spec_kind: Box<str>,
    span: Span,
    reason: Box<str>,
}

#[derive(Debug, Clone, Copy)]
enum ClosureRole {
    Pre,
    Post,
    Forall,
    Exists,
    Assert,
    Assume,
}

impl ClosureRole {
    fn label(self) -> &'static str {
        match self {
            ClosureRole::Pre => "precondition",
            ClosureRole::Post => "postcondition",
            ClosureRole::Forall => "forall",
            ClosureRole::Exists => "exists",
            ClosureRole::Assert => "assert",
            ClosureRole::Assume => "assume",
        }
    }
}

fn lower_specs(
    krate: &TranslatedCrate,
    function_name: &str,
    body: &llbc_ast::ExprBody,
) -> Result<PSpec, Vec<SpecLowerError>> {
    let mut pre = Vec::new();
    let mut post = Vec::new();
    let mut errors = Vec::new();

    for spec_block in &body.specs.preconditions {
        let builder = TermBuilder::new_for_function(
            krate,
            &body.locals,
            function_name.to_owned().into_boxed_str(),
            "precondition".into(),
        );
        match lower_top_level_spec_block(builder, spec_block, ClosureRole::Pre) {
            Ok(lowered) => {
                if !lowered.binders.is_empty() {
                    errors.push(SpecLowerError {
                        function_name: function_name.to_owned().into_boxed_str(),
                        spec_kind: "precondition".into(),
                        span: spec_block.call.span,
                        reason: "precondition closure must not bind parameters".into(),
                    });
                } else {
                    pre.push(lowered.term);
                }
            }
            Err(err) => errors.push(err),
        }
    }

    for spec_block in &body.specs.postconditions {
        let builder = TermBuilder::new_for_function(
            krate,
            &body.locals,
            function_name.to_owned().into_boxed_str(),
            "postcondition".into(),
        );
        match lower_top_level_spec_block(builder, spec_block, ClosureRole::Post) {
            Ok(lowered) => {
                post.push(PPost {
                    span: spec_block.call.span,
                    clauses: vec![(
                        binders_to_pattern(spec_block.call.span, &lowered.binders),
                        lowered.term,
                    )],
                });
            }
            Err(err) => errors.push(err),
        }
    }

    if errors.is_empty() {
        Ok(PSpec { pre, post })
    } else {
        Err(errors)
    }
}

fn lower_top_level_spec_block(
    mut builder: TermBuilder<'_>,
    spec_block: &llbc_ast::FunSpecBlock,
    role: ClosureRole,
) -> Result<LoweredClosure, SpecLowerError> {
    builder.eval_statements(&spec_block.statements)?;
    let Some(closure_operand) = spec_block.call.args.first() else {
        return builder.error(
            spec_block.call.span,
            "empty spec call argument list".to_owned(),
        );
    };
    let closure = builder.eval_operand_as_closure(closure_operand, spec_block.call.span)?;
    builder.lower_closure_value(&closure, role, spec_block.call.span)
}

#[derive(Clone)]
enum Value {
    /// Already-lowered term.
    Term(PTerm),
    /// Closure type and captured, already-lowered terms.
    Closure(ClosureValue),
}

impl Value {
    fn as_term(&self, span: Span) -> PTerm {
        match self {
            Value::Term(term) => term.clone(),
            Value::Closure(closure) => PTerm::new(span, PTermDesc::Tuple(closure.captures.clone())),
        }
    }
}

#[derive(Clone)]
struct ClosureValue {
    type_id: TypeDeclId,
    captures: Vec<PTerm>,
}

struct LoweredClosure {
    binders: Vec<PBinder>,
    term: PTerm,
}

#[derive(Clone)]
struct TermBuilder<'a> {
    krate: &'a TranslatedCrate,
    locals: &'a Locals,
    // `function_name` and `spec_kind` are used for error reporting.
    function_name: Box<str>,
    spec_kind: Box<str>,

    /// Maps LocalIds to their current [`Value`] if known.
    values: IndexVec<LocalId, Option<Value>>,
}

impl<'a> TermBuilder<'a> {
    /// Create a builder initialized for lowering one top-level function spec block.
    ///
    /// Initializes argument and return locals as named term identifiers so later
    /// expression evaluation can reference them directly in the lowered AST.
    fn new_for_function(
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
    fn error<T>(&self, span: Span, reason: String) -> Result<T, SpecLowerError> {
        Err(SpecLowerError {
            function_name: self.function_name.clone(),
            spec_kind: self.spec_kind.clone(),
            span,
            reason: reason.into_boxed_str(),
        })
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

    /// Assign a lowered [`Value`] to a local slot.
    ///
    /// Returns an error if the destination local id is unknown in the current body.
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

    /// Evaluate a sequence of LLBC statements in order and update local state.
    fn eval_statements(
        &mut self,
        statements: &[llbc_ast::Statement],
    ) -> Result<(), SpecLowerError> {
        for statement in statements {
            self.eval_statement(statement)?;
        }
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

    /// Evaluate an rvalue and convert it to a lowered [`Value`].
    ///
    /// Complex constructs unsupported by the spec subset return an explicit error.
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

    /// Evaluate an operand and require it to be a closure aggregate.
    fn eval_operand_as_closure(
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

    /// Evaluate a place expression into a lowered value.
    ///
    /// Supports locals, projections, and named globals.
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
    ///
    /// Field projection is supported for tuple-like terms and closure captures.
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
            ProjectionElem::Index { .. } | ProjectionElem::Subslice { .. } => self.error(
                span,
                "index/subslice projections are unsupported in spec lowering".to_owned(),
            ),
        }
    }

    /// Convert a constant expression to a literal constant used by [`PTermDesc::Const`].
    ///
    /// Non-literal or unsupported constants produce a lowering error.
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

    /// Lower a closure value by evaluating its call body in a nested [`TermBuilder`].
    ///
    /// The closure binders are derived from the call signature and role, then the
    /// call body statements are interpreted to produce the resulting term.
    fn lower_closure_value(
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

        let mut builder = TermBuilder {
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
}

fn derive_closure_binders(
    body: &llbc_ast::ExprBody,
    signature: &FunSig,
    role: ClosureRole,
    span: Span,
) -> Vec<PBinder> {
    let Some(args_ty) = signature.inputs.get(1) else {
        return Vec::new();
    };
    let Some(args_tys) = args_ty.as_tuple() else {
        return Vec::new();
    };

    let binder_count = args_tys.iter().count();
    args_tys
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            let preferred_name = if matches!(role, ClosureRole::Post) && binder_count == 1 {
                "result".to_owned()
            } else {
                body.locals
                    .locals
                    .get(LocalId::new(index + 3))
                    .map(|local| sanitize_local_name(local.name.as_deref(), local.index))
                    .unwrap_or_else(|| format!("arg{index}"))
            };
            PBinder {
                span,
                id: Some(PIdent::new(preferred_name, span)),
                ghost: false,
                ty: Some(ty.clone()),
            }
        })
        .collect()
}

fn resolve_closure_call_fun_id(
    krate: &TranslatedCrate,
    type_id: TypeDeclId,
    function_name: &str,
    spec_kind: &str,
    span: Span,
) -> Result<FunDeclId, SpecLowerError> {
    let Some(type_decl) = krate.type_decls.get(type_id) else {
        return Err(SpecLowerError {
            function_name: function_name.to_owned().into_boxed_str(),
            spec_kind: spec_kind.to_owned().into_boxed_str(),
            span,
            reason: format!("unknown closure type id: {}", type_id.index()).into_boxed_str(),
        });
    };
    let ItemSource::Closure { info } = &type_decl.src else {
        return Err(SpecLowerError {
            function_name: function_name.to_owned().into_boxed_str(),
            spec_kind: spec_kind.to_owned().into_boxed_str(),
            span,
            reason: "spec closure value did not resolve to a closure type".into(),
        });
    };

    let candidates = [
        ("call", info.fn_impl.as_ref().map(|r| r.skip_binder.id)),
        (
            "call_mut",
            info.fn_mut_impl.as_ref().map(|r| r.skip_binder.id),
        ),
        ("call_once", Some(info.fn_once_impl.skip_binder.id)),
    ];
    for (method_name, trait_impl_id) in candidates {
        let Some(trait_impl_id) = trait_impl_id else {
            continue;
        };
        let Some(trait_impl) = krate.trait_impls.get(trait_impl_id) else {
            continue;
        };
        if let Some((_, method_ref)) = trait_impl
            .methods
            .iter()
            .find(|(name, _)| name.0.as_str() == method_name)
        {
            return Ok(method_ref.skip_binder.id);
        }
    }
    Err(SpecLowerError {
        function_name: function_name.to_owned().into_boxed_str(),
        spec_kind: spec_kind.to_owned().into_boxed_str(),
        span,
        reason: format!(
            "failed to resolve callable closure body for type `{}`",
            name_to_string(&type_decl.item_meta.name)
        )
        .into_boxed_str(),
    })
}

fn binders_to_pattern(span: Span, binders: &[PBinder]) -> PPattern {
    match binders {
        [] => PPattern {
            span,
            desc: PPatternDesc::Wild,
        },
        [binder] => {
            if let Some(id) = &binder.id {
                PPattern {
                    span,
                    desc: PPatternDesc::Var(id.clone()),
                }
            } else {
                PPattern {
                    span,
                    desc: PPatternDesc::Wild,
                }
            }
        }
        _ => PPattern {
            span,
            desc: PPatternDesc::Tuple(
                binders
                    .iter()
                    .map(|binder| PPattern {
                        span,
                        desc: binder
                            .id
                            .as_ref()
                            .map(|id| PPatternDesc::Var(id.clone()))
                            .unwrap_or(PPatternDesc::Wild),
                    })
                    .collect(),
            ),
        },
    }
}

fn is_closure_type(krate: &TranslatedCrate, type_id: TypeId) -> bool {
    let TypeId::Adt(type_id) = type_id else {
        return false;
    };
    krate
        .type_decls
        .get(type_id)
        .map(|decl| matches!(decl.src, ItemSource::Closure { .. }))
        .unwrap_or(false)
}

fn local_ident_term(span: Span, name: &str) -> PTerm {
    PTerm::new(
        span,
        PTermDesc::Ident(PQualid::Ident(PIdent::new(name, span))),
    )
}

fn sanitize_local_name(name: Option<&str>, local: LocalId) -> String {
    let base = name
        .map(str::to_owned)
        .unwrap_or_else(|| format!("_{}", local.index()));
    if let Some((prefix, suffix)) = base.rsplit_once('_')
        && !prefix.is_empty()
        && suffix.chars().all(|c| c.is_ascii_digit())
    {
        return prefix.to_owned();
    }
    base
}

fn name_to_string(name: &Name) -> String {
    name_path_segments(name).join("::")
}

fn name_to_qualid(name: &Name, span: Span) -> PQualid {
    let mut segments = name_path_segments(name).into_iter();
    let Some(first) = segments.next() else {
        return PQualid::ident("<anon>", span);
    };
    let mut qid = PQualid::Ident(PIdent::new(first, span));
    for segment in segments {
        qid = PQualid::Dot(Box::new(qid), PIdent::new(segment, span));
    }
    qid
}

fn name_path_segments(name: &Name) -> Vec<String> {
    name.name
        .iter()
        .map(|elem| match elem {
            PathElem::Ident(seg, _) => seg.clone(),
            PathElem::Impl(_) => "impl".to_owned(),
            PathElem::Instantiated(_) => "inst".to_owned(),
        })
        .collect()
}

fn map_binop(binop: BinOp) -> Option<PBinOp> {
    match binop {
        BinOp::Eq => Some(PBinOp::Eq),
        BinOp::Ne => Some(PBinOp::Ne),
        BinOp::Lt => Some(PBinOp::Lt),
        BinOp::Le => Some(PBinOp::Le),
        BinOp::Gt => Some(PBinOp::Gt),
        BinOp::Ge => Some(PBinOp::Ge),
        BinOp::Add(_) => Some(PBinOp::Add),
        BinOp::Sub(_) => Some(PBinOp::Sub),
        BinOp::Mul(_) => Some(PBinOp::Mul),
        BinOp::Div(_) => Some(PBinOp::Div),
        BinOp::Rem(_) => Some(PBinOp::Rem),
        BinOp::BitAnd => Some(PBinOp::And),
        BinOp::BitOr => Some(PBinOp::Or),
        _ => None,
    }
}

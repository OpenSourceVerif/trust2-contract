use crate::{
    ast::{
        Body, BodyVisitable, BuiltinFunId, Call, ConstantExprKind, ContractAssertKind, DeBruijnId,
        DeBruijnVar, FieldId, FnOperand, FnPtrKind, FunDecl, FunDeclId, FunId, Literal, Local,
        LocalId, Operand, Place, PlaceKind, ProjectionElem, QuantKind, Region, Rvalue, ScalarValue,
        Span, SpecClosure, SpecClosureId, TraitDeclId, TyKind, TyVisitable, TypeDeclId, TypeId,
        WithRetag, ullbc_ast_utils::BodyBuilder, visitor::AstVisitable,
    },
    ids::IndexMap,
    transform::{TransformCtx, ctx::UllbcPass},
    ullbc_ast::{BlockId, ExprBody, Statement, StatementKind, TerminatorKind},
};

use std::{
    collections::{HashMap, HashSet},
    mem,
    sync::Mutex,
};

/// A host-level specification closure whose captures were erased before `mir_borrowck`.
#[derive(Debug, Clone)]
pub struct PendingSpecClosure {
    /// Closure state `TypeDeclId` registered during MIR-to-ULLBC translation.
    pub closure_type_decl_id: TypeDeclId,
    /// Capture expressions translated in the host function's `LocalId` namespace.
    pub captures: Vec<Rvalue>,
}

/// Pending host-level specifications associated with a translated `FunDecl`.
#[derive(Debug, Clone, Default)]
pub struct PendingFunSpecs {
    /// Preconditions in MIR entry execution order.
    pub preconditions: Vec<PendingSpecClosure>,
    /// Postconditions in MIR entry execution order.
    pub postconditions: Vec<PendingSpecClosure>,
    /// Inline assertions/assumptions keyed by the non-zero anchor ID in `SpecEntry(anchor_id)`.
    pub inline_asserts: HashMap<u32, (ContractAssertKind, PendingSpecClosure)>,
}

impl PendingFunSpecs {
    /// Mutably iterates over all captured `Rvalue` expressions across preconditions,
    /// postconditions, and inline assertions.
    pub fn iter_captures_mut(&mut self) -> impl Iterator<Item = &mut Rvalue> {
        self.preconditions
            .iter_mut()
            .chain(self.postconditions.iter_mut())
            .chain(self.inline_asserts.values_mut().map(|(_, spec)| spec))
            .flat_map(|spec| spec.captures.iter_mut())
    }

    /// Shifts all captured host `LocalId`s at or above `from_index` by `delta`.
    ///
    /// Used when an enclosing runtime closure inserts `tupled_args` at `LocalId(2)`.
    pub fn shift_locals_from(&mut self, from_index: usize, delta: usize) {
        for rvalue in self.iter_captures_mut() {
            rvalue.dyn_visit_in_body_mut(|local_id: &mut LocalId| {
                if local_id.index() >= from_index {
                    *local_id += delta;
                }
            });
        }
    }
}

/// ULLBC pass that converts `trust2-contract` specification closures into `SpecClosure` nodes.
pub struct Transform {
    transformed: Mutex<HashSet<FunDeclId>>,
    fn_trait_id: Option<TraitDeclId>,
    fn_mut_trait_id: Option<TraitDeclId>,
    fn_once_trait_id: Option<TraitDeclId>,
}

impl Transform {
    /// Creates the specification collection pass, resolving `Fn`, `FnMut`, and `FnOnce` trait IDs.
    pub fn new(ctx: &TransformCtx) -> Self {
        let mut fn_trait_id = None;
        let mut fn_mut_trait_id = None;
        let mut fn_once_trait_id = None;
        for (trait_id, trait_) in ctx.translated.trait_decls.iter_enumerated() {
            let name = &trait_.item_meta.name;
            if name.equals_ref_name(&["core", "ops", "function", "Fn"]) {
                fn_trait_id = Some(trait_id);
            } else if name.equals_ref_name(&["core", "ops", "function", "FnMut"]) {
                fn_mut_trait_id = Some(trait_id);
            } else if name.equals_ref_name(&["core", "ops", "function", "FnOnce"]) {
                fn_once_trait_id = Some(trait_id);
            }
        }
        Self {
            transformed: Mutex::new(HashSet::new()),
            fn_trait_id,
            fn_mut_trait_id,
            fn_once_trait_id,
        }
    }

    /// Extracts the closure `FunDecl` for `closure_type_decl_id`, unpacks its argument tuple and
    /// captured state fields, and returns a normalized `SpecClosure`.
    fn build_spec_closure(
        &self,
        ctx: &mut TransformCtx,
        host_region_count: usize,
        closure_type_decl_id: TypeDeclId,
        captures: Vec<Rvalue>,
    ) -> SpecClosure {
        let closure_type_decl = ctx
            .translated
            .type_decls
            .remove(closure_type_decl_id)
            .unwrap();
        let closure_fields = closure_type_decl.kind.as_struct().unwrap();

        let (mut fn_method_decl, is_ref_self) = {
            let trait_impl_ids: Vec<_> = ctx
                .translated
                .trait_impls
                .iter_enumerated()
                .filter_map(|(trait_impl_id, trait_impl)| {
                    let TyKind::Adt(type_ref) =
                        trait_impl.impl_trait.self_ty(&ctx.translated)?.kind()
                    else {
                        return None;
                    };
                    let TypeId::Adt(type_decl_id) = type_ref.id else {
                        return None;
                    };
                    (type_decl_id == closure_type_decl_id).then_some(trait_impl_id)
                })
                .collect();

            let mut fn_decl = None;
            let mut fn_mut_decl = None;
            let mut fn_once_decl = None;
            for trait_impl_id in trait_impl_ids {
                let trait_impl = ctx.translated.trait_impls.remove(trait_impl_id).unwrap();
                let mut methods: Vec<_> = trait_impl
                    .methods
                    .into_iter()
                    .map(|trait_method| {
                        ctx.translated
                            .fun_decls
                            .remove(trait_method.skip_binder.id)
                            .unwrap()
                    })
                    .collect();
                let impl_trait_id = Some(trait_impl.impl_trait.id);
                if impl_trait_id == self.fn_trait_id {
                    fn_decl = methods.pop();
                } else if impl_trait_id == self.fn_mut_trait_id {
                    fn_mut_decl = methods.pop();
                } else if impl_trait_id == self.fn_once_trait_id {
                    fn_once_decl = methods.pop();
                }
            }
            if let Some(decl) = fn_decl {
                (decl, true)
            } else if let Some(decl) = fn_mut_decl {
                (decl, true)
            } else {
                (fn_once_decl.unwrap(), false)
            }
        };

        let prev_in_spec = ctx.in_spec_body;
        ctx.in_spec_body = true;
        self.transform_function_with_regions(ctx, &mut fn_method_decl, host_region_count);
        ctx.in_spec_body = prev_in_spec;
        assert_eq!(fn_method_decl.signature.inputs.len(), 2);
        assert_eq!(fn_method_decl.signature.output, crate::ast::Ty::mk_bool());
        let mut spec_body = fn_method_decl.body.to_unstructured().unwrap();

        let tupled_args_id = LocalId::from(2);
        let tupled_args = &spec_body.locals[tupled_args_id];
        let TyKind::Adt(type_ref) = tupled_args.ty.kind() else {
            unreachable!();
        };
        assert!(matches!(type_ref.id, TypeId::Tuple));
        let arg_types = &type_ref.generics.types;
        let arg_locals = arg_types
            .iter_enumerated()
            .map(|(type_var_id, ty)| Local {
                index: tupled_args_id + type_var_id.raw(),
                name: None,
                span: tupled_args.span,
                ty: ty.clone(),
            })
            .collect();
        unpack_local(
            &mut spec_body,
            tupled_args_id,
            arg_locals,
            |place: &Place| {
                let PlaceKind::Projection(place_, ProjectionElem::Field(.., field_id)) =
                    &place.kind
                else {
                    return None;
                };
                let PlaceKind::Local(local_id) = place_.kind else {
                    return None;
                };
                (local_id == tupled_args_id).then_some(field_id.raw())
            },
            &mut ctx.translated.spec_closures,
        );

        let self_id = LocalId::from(1);
        let self_span = spec_body.locals[self_id].span;
        let state_generics = if is_ref_self {
            let TyKind::Ref(_, state_ty, _) = spec_body.locals[self_id].ty.kind() else {
                unreachable!();
            };
            let TyKind::Adt(state_tref) = state_ty.kind() else {
                unreachable!();
            };
            state_tref.generics.clone()
        } else {
            let TyKind::Adt(state_tref) = spec_body.locals[self_id].ty.kind() else {
                unreachable!();
            };
            state_tref.generics.clone()
        };

        assert_eq!(captures.len(), closure_fields.len());
        let mut new_captures = IndexMap::with_capacity(1 + captures.len());
        let capture_locals: Vec<_> = captures
            .into_iter()
            .enumerate()
            .map(|(id, cap_rvalue)| {
                let new_local_id = self_id + id;
                let cap_ty = closure_fields[FieldId::from(id)]
                    .ty
                    .clone()
                    .substitute(&state_generics);
                new_captures.set_slot_extend(new_local_id, cap_rvalue);
                Local {
                    index: new_local_id,
                    name: None,
                    span: self_span,
                    ty: cap_ty,
                }
            })
            .collect();

        unpack_local(
            &mut spec_body,
            self_id,
            capture_locals,
            |place: &Place| {
                let PlaceKind::Projection(base, ProjectionElem::Field(_, field_id)) = &place.kind
                else {
                    return None;
                };
                let matches_self = if is_ref_self {
                    matches!(
                        &base.kind,
                        PlaceKind::Projection(inner, ProjectionElem::Deref)
                            if matches!(inner.kind, PlaceKind::Local(lid) if lid == self_id)
                    )
                } else {
                    matches!(base.kind, PlaceKind::Local(lid) if lid == self_id)
                };
                matches_self.then_some(field_id.raw())
            },
            &mut ctx.translated.spec_closures,
        );

        // Erase closure-introduced item region parameters (indices >= `host_region_count`), keeping
        // any host item region parameters intact, then freshen body/erased regions.
        spec_body.dyn_visit_mut(|region: &mut Region| {
            if let Region::Var(DeBruijnVar::Bound(DeBruijnId::ZERO, rid)) = *region
                && rid.index() >= host_region_count
            {
                *region = Region::Erased;
            }
        });
        let spec_body = BodyBuilder {
            span: Span::default(),
            body: spec_body,
            current_block: BlockId::default(),
            unwind_block: None,
        }
        .build();

        SpecClosure {
            body: Body::Unstructured(spec_body),
            captures: new_captures,
        }
    }

    /// Collects un-erased specification calls (such as `forall!` / `exists!` inside spec bodies)
    /// directly from ULLBC statements.
    fn collect_inline_block_spec_closure(
        &self,
        call_args: Vec<Operand>,
        closure_assigns: Vec<Statement>,
        ctx: &mut TransformCtx,
        host_region_count: usize,
    ) -> SpecClosure {
        let [closure] = call_args.try_into().unwrap();
        let Operand::Move(closure) = closure else {
            unreachable!();
        };
        assert!(matches!(closure.kind, PlaceKind::Local(..)));
        let TyKind::Adt(type_ref) = closure.ty.kind() else {
            unreachable!();
        };
        let TypeId::Adt(closure_type_decl_id) = type_ref.id else {
            unreachable!();
        };

        let mut closure_assigns =
            closure_assigns
                .into_iter()
                .filter_map(|old_statement| match old_statement.kind {
                    StatementKind::Assign(place, rvalue) => Some((place, rvalue)),
                    StatementKind::StorageLive(..)
                    | StatementKind::StorageDead(..)
                    | StatementKind::PlaceMention(..)
                    | StatementKind::Nop => None,
                    _ => unreachable!(),
                });
        let (_, rvalue) = closure_assigns.next_back().unwrap();
        let Rvalue::Aggregate(.., closure_env) = rvalue else {
            unreachable!();
        };
        let mut old_captures = IndexMap::new();
        for (place, rvalue) in closure_assigns {
            if let PlaceKind::Local(local_id) = place.kind {
                old_captures.set_slot_extend(local_id, rvalue);
            }
        }
        let captures = closure_env
            .into_iter()
            .map(|operand| match &operand {
                Operand::Move(place)
                    if matches!(place.kind, PlaceKind::Local(lid) if old_captures.get(lid).is_some()) =>
                {
                    let PlaceKind::Local(lid) = place.kind else {
                        unreachable!();
                    };
                    old_captures[lid].clone()
                }
                Operand::Move(place) | Operand::Copy(place) => {
                    Rvalue::Use(Operand::Copy(place.clone()), WithRetag::No)
                }
                Operand::Const(..) => Rvalue::Use(operand, WithRetag::No),
            })
            .collect();

        self.build_spec_closure(ctx, host_region_count, closure_type_decl_id, captures)
    }

    fn transform_function_with_regions(
        &self,
        ctx: &mut TransformCtx,
        func_decl: &mut FunDecl,
        host_region_count: usize,
    ) {
        if !self.transformed.lock().unwrap().insert(func_decl.def_id) {
            return;
        }

        let mut pending = ctx.pending_specs.remove(&func_decl.def_id);
        if let Some(pending) = &mut pending {
            for pre in mem::take(&mut pending.preconditions) {
                let spec_closure = self.build_spec_closure(
                    ctx,
                    host_region_count,
                    pre.closure_type_decl_id,
                    pre.captures,
                );
                func_decl.specs.preconditions.push(spec_closure);
            }
            for post in mem::take(&mut pending.postconditions) {
                let spec_closure = self.build_spec_closure(
                    ctx,
                    host_region_count,
                    post.closure_type_decl_id,
                    post.captures,
                );
                func_decl.specs.postconditions.push(spec_closure);
            }
        }

        if func_decl.body.as_unstructured().is_none() {
            return;
        }
        let facts = func_decl
            .body
            .as_unstructured()
            .and_then(|b| crate::transform::borrowck::take(ctx, b));
        let body = func_decl.body.as_unstructured_mut().unwrap();

        for block in &mut body.body {
            let TerminatorKind::Call {
                call:
                    Call {
                        ref func,
                        ref mut args,
                        ref dest,
                    },
                target,
                ..
            } = block.terminator.kind
            else {
                continue;
            };
            let FnOperand::Regular(func_ptr) = func else {
                continue;
            };
            let FnPtrKind::Fun(FunId::Builtin(builtin_func_id)) = *func_ptr.kind else {
                continue;
            };

            let mut take_and_collect = |ctx: &mut TransformCtx| {
                self.collect_inline_block_spec_closure(
                    mem::take(args),
                    mem::take(&mut block.statements),
                    ctx,
                    host_region_count,
                )
            };
            match builtin_func_id {
                BuiltinFunId::SpecEntry => {
                    let anchor_id = extract_entry_anchor_id(args);
                    if anchor_id != 0
                        && let Some((kind, pending_spec)) = pending
                            .as_mut()
                            .and_then(|p| p.inline_asserts.remove(&anchor_id))
                    {
                        let spec_closure = self.build_spec_closure(
                            ctx,
                            host_region_count,
                            pending_spec.closure_type_decl_id,
                            pending_spec.captures,
                        );
                        block.terminator.kind = TerminatorKind::ContractAssert {
                            kind,
                            spec_closure_id: ctx.translated.spec_closures.push(spec_closure),
                            target,
                        };
                    } else {
                        block.terminator.kind = TerminatorKind::Goto { target };
                    }
                }
                BuiltinFunId::SpecPrecondition => {
                    func_decl.specs.preconditions.push(take_and_collect(ctx));
                    block.terminator.kind = TerminatorKind::Goto { target };
                }
                BuiltinFunId::SpecPostcondition => {
                    func_decl.specs.postconditions.push(take_and_collect(ctx));
                    block.terminator.kind = TerminatorKind::Goto { target };
                }
                BuiltinFunId::SpecAssert => {
                    let spec_closure = take_and_collect(ctx);
                    block.terminator.kind = TerminatorKind::ContractAssert {
                        kind: ContractAssertKind::Assert,
                        spec_closure_id: ctx.translated.spec_closures.push(spec_closure),
                        target,
                    };
                }
                BuiltinFunId::SpecAssume => {
                    let spec_closure = take_and_collect(ctx);
                    block.terminator.kind = TerminatorKind::ContractAssert {
                        kind: ContractAssertKind::Assume,
                        spec_closure_id: ctx.translated.spec_closures.push(spec_closure),
                        target,
                    };
                }
                BuiltinFunId::SpecForAll => {
                    let spec_closure = take_and_collect(ctx);
                    block.terminator.kind = TerminatorKind::Quant {
                        kind: QuantKind::ForAll,
                        spec_closure_id: ctx.translated.spec_closures.push(spec_closure),
                        dest: dest.clone(),
                        target,
                    };
                }
                BuiltinFunId::SpecExists => {
                    let spec_closure = take_and_collect(ctx);
                    block.terminator.kind = TerminatorKind::Quant {
                        kind: QuantKind::Exists,
                        spec_closure_id: ctx.translated.spec_closures.push(spec_closure),
                        dest: dest.clone(),
                        target,
                    };
                }
                _ => (),
            }
        }

        if let Some(body) = func_decl.body.as_unstructured() {
            crate::transform::borrowck::put(ctx, body, facts);
        }
    }
}

impl UllbcPass for Transform {
    fn transform_function(&self, ctx: &mut TransformCtx, func_decl: &mut FunDecl) {
        let host_region_count = func_decl.generics.regions.len();
        self.transform_function_with_regions(ctx, func_decl, host_region_count);
    }
}

/// Extracts the `u32` anchor argument passed to `trust2_contract::internal::entry(anchor_id)`.
fn extract_entry_anchor_id(args: &[Operand]) -> u32 {
    let Some(Operand::Const(const_expr)) = args.first() else {
        return 0;
    };
    let ConstantExprKind::Literal(Literal::Scalar(ScalarValue::Unsigned(_, val))) =
        &const_expr.kind
    else {
        return 0;
    };
    *val as u32
}

fn unpack_local(
    body: &mut ExprBody,
    id: LocalId,
    replace_with: Vec<Local>,
    mut match_field: impl FnMut(&Place) -> Option<usize>,
    spec_closures: &mut IndexMap<SpecClosureId, SpecClosure>,
) {
    for block in &mut body.body {
        block.statements.retain(|stmt| {
            !matches!(
                stmt.kind,
                StatementKind::StorageLive(lid) | StatementKind::StorageDead(lid) if lid == id
            )
        });
    }

    let count = replace_with.len();
    body.locals.arg_count = body.locals.arg_count + count - 1;

    // Step 1: Shift all existing locals `> id` up by `+count` (non-negative, so none can ever
    // alias `id` even when `count == 0`) and insert `replace_with` at `(id + 1)..=(id + count)`.
    let shift_up = |local_id: &mut LocalId| {
        if *local_id > id {
            *local_id += count;
        }
    };
    body.body.dyn_visit_in_body_mut(shift_up);
    body.body
        .dyn_visit_in_body(|&spec_closure_id: &SpecClosureId| {
            spec_closures[spec_closure_id]
                .captures
                .dyn_visit_in_body_mut(shift_up);
        });

    let inserted = replace_with
        .into_iter()
        .enumerate()
        .map(|(offset, local)| Local {
            index: id + 1 + offset,
            ..local
        });
    body.locals.locals.splice((id + 1)..(id + 1), inserted);
    for (idx, local) in body.locals.locals.iter_mut().enumerate() {
        local.index = LocalId::from(idx);
    }

    // Step 2: Rewrite field projections of `id` (which is still uniquely at `id`) to `id + 1 + k`.
    let mut rewrite_place = |place: &mut Place| {
        if let Some(field_idx) = match_field(place) {
            *place = body.locals.place_for_var(id + 1 + field_idx);
        }
    };
    body.body.dyn_visit_in_body_mut(&mut rewrite_place);
    body.body
        .dyn_visit_in_body(|&spec_closure_id: &SpecClosureId| {
            spec_closures[spec_closure_id]
                .captures
                .dyn_visit_in_body_mut(&mut rewrite_place);
        });

    // Step 3: Remove slot `id` and shift all locals `> id` down by 1.
    body.locals.locals.splice(id..=id, std::iter::empty());
    for (idx, local) in body.locals.locals.iter_mut().enumerate() {
        local.index = LocalId::from(idx);
    }
    let shift_down = |local_id: &mut LocalId| {
        assert_ne!(*local_id, id, "unpacked local `{id}` remained referenced");
        if *local_id > id {
            *local_id = LocalId::from(local_id.index() - 1);
        }
    };
    body.body.dyn_visit_in_body_mut(shift_down);
    body.body
        .dyn_visit_in_body(|&spec_closure_id: &SpecClosureId| {
            spec_closures[spec_closure_id]
                .captures
                .dyn_visit_in_body_mut(shift_down);
        });
}

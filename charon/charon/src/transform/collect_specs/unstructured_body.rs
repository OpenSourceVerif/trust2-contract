use crate::{
    ast::{
        Body, BodyVisitable, BuiltinFunId, Call, ContractAssertKind, FnOperand, FnPtrKind, FunDecl,
        FunId, FunSpecs, Local, LocalId, Locals, Operand, Place, PlaceKind, ProjectionElem, Rvalue,
        Span, SpecClosure, TraitDeclId, TranslatedCrate, Ty, TyKind, TypeId,
        ullbc_ast_utils::BodyBuilder,
    },
    ids::IndexMap,
    transform::{TransformCtx, ctx::UllbcPass},
    ullbc_ast::{BlockId, ExprBody, Statement, StatementKind, TerminatorKind},
};

use std::mem;

pub struct Transform {
    fn_trait_id: Option<TraitDeclId>,
}

impl Transform {
    pub fn new(ctx: &TransformCtx) -> Self {
        let fn_trait_id = 'a: {
            for (trait_id, trait_) in ctx.translated.trait_decls.iter_enumerated() {
                if trait_
                    .item_meta
                    .name
                    .equals_ref_name(&["core", "ops", "function", "Fn"])
                {
                    break 'a Some(trait_id);
                }
            }
            None
        };
        Self { fn_trait_id }
    }
}

impl UllbcPass for Transform {
    fn transform_function(&self, ctx: &mut TransformCtx, func_decl: &mut FunDecl) {
        let Some(body) = func_decl.body.as_unstructured_mut() else {
            return;
        };
        let FunSpecs {
            preconditions,
            postconditions,
        } = &mut func_decl.specs;

        let crate_ = &mut ctx.translated;
        let collect_spec_closure =
            |call_args: Vec<_>, closure_assigns: Vec<Statement>, crate_: &mut TranslatedCrate| {
                let closure_type = {
                    let [closure] = call_args.try_into().unwrap();
                    let Operand::Move(closure) = closure else {
                        unreachable!();
                    };
                    assert!(matches!(closure.kind, PlaceKind::Local(..)));
                    closure.ty
                };
                let TyKind::Adt(type_ref) = closure_type.kind() else {
                    unreachable!();
                };
                let TypeId::Adt(closure_type_decl_id) = type_ref.id else {
                    unreachable!();
                };
                crate_.type_decls.remove(closure_type_decl_id);

                let fn_method_decl = {
                    let trait_impl_ids: Vec<_> = crate_
                        .trait_impls
                        .iter_enumerated()
                        .filter_map(|(trait_impl_id, trait_impl)| {
                            let TyKind::Adt(type_ref) =
                                trait_impl.impl_trait.self_ty(crate_).unwrap().kind()
                            else {
                                return None;
                            };
                            let TypeId::Adt(type_decl_id) = type_ref.id else {
                                return None;
                            };
                            if type_decl_id == closure_type_decl_id {
                                Some(trait_impl_id)
                            } else {
                                None
                            }
                        })
                        .collect();
                    let trait_method_decls: Vec<_> = trait_impl_ids
                        .into_iter()
                        .flat_map(|trait_impl_id| {
                            let trait_impl = crate_.trait_impls.remove(trait_impl_id).unwrap();
                            let trait_method_decls: Vec<_> = trait_impl
                                .methods
                                .into_iter()
                                .map(|trait_method| {
                                    crate_
                                        .fun_decls
                                        .remove(trait_method.skip_binder.id)
                                        .unwrap()
                                })
                                .collect();
                            if trait_impl.impl_trait.id == self.fn_trait_id.unwrap() {
                                trait_method_decls
                            } else {
                                Vec::new()
                            }
                        })
                        .collect();

                    let [trait_method_decl] = trait_method_decls.try_into().unwrap();
                    trait_method_decl
                };
                assert!(fn_method_decl.signature.inputs.len() == 2);
                assert!(fn_method_decl.signature.output == Ty::mk_bool());
                let mut spec_body = fn_method_decl.body.to_unstructured().unwrap();

                fn unpack_local<T: BodyVisitable>(
                    body: &mut ExprBody,
                    id: LocalId,
                    replace_with: Vec<Local>,
                    mut update_body: impl FnMut(&mut T, &Locals),
                ) {
                    let delta = replace_with.len() as isize - 1;
                    body.locals.arg_count = body.locals.arg_count.strict_add_signed(delta);
                    body.dyn_visit_in_body_mut(|local_id: &mut LocalId| {
                        if *local_id > id {
                            *local_id = local_id.raw().strict_add_signed(delta).into();
                        }
                    });
                    body.locals.locals.splice(id..=id, replace_with);
                    body.body
                        .dyn_visit_in_body_mut(|node| update_body(node, &body.locals));
                }

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
                    |place: &mut Place, locals| {
                        let PlaceKind::Projection(place_, ProjectionElem::Field(.., field_id)) =
                            &place.kind
                        else {
                            return;
                        };
                        let PlaceKind::Local(local_id) = place_.kind else {
                            return;
                        };
                        if local_id != tupled_args_id {
                            return;
                        }

                        *place = locals.place_for_var(tupled_args_id + field_id.raw());
                    },
                );

                let self_id = LocalId::from(1);
                let mut closure_assigns = closure_assigns.into_iter().filter_map(|old_statement| {
                    match old_statement.kind {
                        StatementKind::Assign(place, rvalue) => Some((place, rvalue)),
                        StatementKind::StorageLive(..) | StatementKind::StorageDead(..) => None,
                        _ => unreachable!(),
                    }
                });
                let (_, rvalue) = closure_assigns.next_back().unwrap();
                let Rvalue::Aggregate(.., closure_env) = rvalue else {
                    unreachable!();
                };
                let mut old_captures = IndexMap::new();
                for (place, rvalue) in closure_assigns {
                    let PlaceKind::Local(local_id) = place.kind else {
                        unreachable!();
                    };
                    old_captures.set_slot_extend(local_id, rvalue);
                }
                let mut new_captures = IndexMap::with_capacity(1 + closure_env.len());
                unpack_local(
                    &mut spec_body,
                    self_id,
                    closure_env
                        .iter()
                        .enumerate()
                        .map(|(id, operand)| {
                            let Operand::Move(place) = operand else {
                                unreachable!();
                            };
                            let PlaceKind::Local(old_local_id) = place.kind else {
                                unreachable!();
                            };
                            let new_local_id = self_id + id;
                            new_captures
                                .set_slot_extend(new_local_id, old_captures[old_local_id].clone());
                            Local {
                                index: new_local_id,
                                ..body.locals[old_local_id].clone()
                            }
                        })
                        .collect(),
                    |place: &mut Place, locals| {
                        let PlaceKind::Projection(place_, ProjectionElem::Field(_, field_id)) =
                            &place.kind
                        else {
                            return;
                        };
                        let PlaceKind::Projection(place__, ProjectionElem::Deref) = &place_.kind
                        else {
                            return;
                        };
                        let PlaceKind::Local(local_id) = place__.kind else {
                            return;
                        };
                        if local_id != self_id {
                            return;
                        }

                        *place = locals.place_for_var(self_id + field_id.raw());
                    },
                );

                // Freshen regions.
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
            };
        for block in &mut body.body {
            let TerminatorKind::Call {
                call:
                    Call {
                        ref func,
                        ref mut args,
                        ..
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

            let mut take_and_collect = |crate_| {
                collect_spec_closure(mem::take(args), mem::take(&mut block.statements), crate_)
            };
            match builtin_func_id {
                BuiltinFunId::SpecEntry => {
                    block.terminator.kind = TerminatorKind::Goto { target };
                }
                BuiltinFunId::SpecPrecondition => {
                    preconditions.push(take_and_collect(crate_));
                    block.terminator.kind = TerminatorKind::Goto { target };
                }
                BuiltinFunId::SpecPostcondition => {
                    postconditions.push(take_and_collect(crate_));
                    block.terminator.kind = TerminatorKind::Goto { target };
                }
                BuiltinFunId::SpecAssert => {
                    let spec_closure = take_and_collect(crate_);
                    block.terminator.kind = TerminatorKind::ContractAssert {
                        kind: ContractAssertKind::Assert,
                        spec_closure_id: crate_.spec_closures.push(spec_closure),
                        target,
                    };
                }
                BuiltinFunId::SpecAssume => {
                    let spec_closure = take_and_collect(crate_);
                    block.terminator.kind = TerminatorKind::ContractAssert {
                        kind: ContractAssertKind::Assume,
                        spec_closure_id: crate_.spec_closures.push(spec_closure),
                        target,
                    };
                }
                _ => (),
            }
        }
    }
}

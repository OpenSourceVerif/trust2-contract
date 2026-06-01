use crate::{
    ast::{
        Body, BodyVisitable, BuiltinFunId, Call, FieldId, FnOperand, FnPtrKind, FunDecl, FunId,
        FunSpecs, LocalId, Operand, Place, PlaceKind, ProjectionElem, Rvalue, Span, TraitDeclId,
        Ty, TyKind, TypeId,
    },
    ids::{IndexMap, IndexVec},
    transform::{TransformCtx, ctx::UllbcPass},
    ullbc_ast::{
        BlockData, BlockId, Postcondition, Statement, StatementKind, Terminator, TerminatorKind,
        ullbc_ast_utils::BodyBuilder,
    },
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
        let mut collect_spec_closure =
            |call_span, is_precondition, call_args: Vec<_>, old_statements: Vec<Statement>| {
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

                let arg_count = body.locals.arg_count;
                spec_body.locals.arg_count = arg_count;
                spec_body.dyn_visit_in_body_mut(|local_id: &mut LocalId| {
                    if *local_id != 0 {
                        *local_id += arg_count;
                    }
                });
                spec_body.locals.locals.splice(
                    LocalId::from(1)..LocalId::from(1),
                    (*body.locals.locals)[LocalId::from(1)..=LocalId::from(arg_count)].to_vec(),
                );

                let mut old_assigns = old_statements.into_iter().filter_map(|old_statement| {
                    match old_statement.kind {
                        StatementKind::Assign(place, rvalue) => {
                            Some((old_statement.span, place, rvalue))
                        }
                        StatementKind::StorageLive(..) | StatementKind::StorageDead(..) => None,
                        _ => unreachable!(),
                    }
                });
                let closure_assign = old_assigns.next_back().unwrap();
                let mut local_id_map = IndexMap::new();
                let new_statements = old_assigns
                    .map(|(span, old_place, rvalue)| {
                        let PlaceKind::Local(old_local_id) = old_place.kind else {
                            unreachable!();
                        };
                        let new_place = spec_body
                            .locals
                            .new_var(None, body.locals[old_local_id].ty.clone());
                        local_id_map.set_slot_extend(old_local_id, new_place.clone());
                        Statement::new(span, StatementKind::Assign(new_place, rvalue))
                    })
                    .collect();
                spec_body.body.insert(
                    BlockId::ZERO,
                    BlockData {
                        statements: new_statements,
                        terminator: Terminator::goto(call_span, BlockId::ZERO),
                    },
                );
                spec_body
                    .body
                    .dyn_visit_in_body_mut(|block_id: &mut BlockId| *block_id += 1);
                let Rvalue::Aggregate(.., closure_env_map) = closure_assign.2 else {
                    unreachable!();
                };
                let closure_env_map: IndexVec<FieldId, _> = closure_env_map
                    .into_iter()
                    .map(|operand| {
                        let Operand::Move(place) = operand else {
                            unreachable!();
                        };
                        let PlaceKind::Local(local_id) = place.kind else {
                            unreachable!();
                        };
                        local_id_map[local_id].clone()
                    })
                    .collect();
                spec_body.body.dyn_visit_in_body_mut(|place: &mut Place| {
                    let PlaceKind::Projection(place_, ProjectionElem::Field(_, field_id)) =
                        &place.kind
                    else {
                        return;
                    };
                    let PlaceKind::Projection(place__, ProjectionElem::Deref) = &place_.kind else {
                        return;
                    };
                    let PlaceKind::Local(local_id) = place__.kind else {
                        return;
                    };
                    if local_id != arg_count + 1 {
                        return;
                    }

                    *place = closure_env_map[*field_id].clone();
                });

                // Freshen regions.
                let mut spec_body = BodyBuilder {
                    span: Span::default(),
                    body: spec_body,
                    current_block: BlockId::default(),
                    unwind_block: None,
                }
                .build();

                if is_precondition {
                    preconditions.push(Body::Unstructured(spec_body));
                } else {
                    spec_body.locals.arg_count += 1;
                    let arg_id = LocalId::from(arg_count + 1);
                    let self_id = LocalId::from(arg_count + 2);
                    spec_body.dyn_visit_in_body_mut(|local_id: &mut LocalId| {
                        if *local_id == arg_id {
                            *local_id = self_id;
                        } else if *local_id == self_id {
                            *local_id = arg_id;
                        }
                    });
                    spec_body.locals.locals.swap(arg_id, self_id);
                    let tupled_args = &mut spec_body.locals[arg_id];
                    tupled_args.name = None;
                    let TyKind::Adt(type_ref) = tupled_args.ty.kind() else {
                        unreachable!();
                    };
                    assert!(matches!(type_ref.id, TypeId::Tuple));
                    let [ty] = type_ref.generics.types.as_raw_slice() else {
                        unreachable!();
                    };
                    tupled_args.ty = ty.clone();
                    spec_body.body.dyn_visit_in_body_mut(|place: &mut Place| {
                        let PlaceKind::Projection(place_, _) = &place.kind else {
                            return;
                        };
                        let PlaceKind::Local(local_id) = place_.kind else {
                            return;
                        };
                        if local_id != arg_id {
                            return;
                        }

                        *place = place_.as_ref().clone();
                    });
                    postconditions.push(Postcondition {
                        arg_id,
                        body: Body::Unstructured(spec_body),
                    });
                }
            };
        for block in &mut body.body {
            let Terminator {
                span,
                kind:
                    TerminatorKind::Call {
                        call:
                            Call {
                                ref func,
                                ref mut args,
                                ..
                            },
                        target,
                        ..
                    },
                ..
            } = block.terminator
            else {
                continue;
            };
            let FnOperand::Regular(func_ptr) = func else {
                continue;
            };
            let FnPtrKind::Fun(FunId::Builtin(builtin_func_id)) = *func_ptr.kind else {
                continue;
            };

            match builtin_func_id {
                BuiltinFunId::SpecEntry => {
                    block.terminator.kind = TerminatorKind::Goto { target };
                }
                BuiltinFunId::SpecPrecondition => {
                    let args = mem::take(args);
                    let new_block: BlockData = BlockData::new_goto(span, target);
                    let old_block = mem::replace(block, new_block);
                    collect_spec_closure(span, true, args, old_block.statements);
                }
                BuiltinFunId::SpecPostcondition => {
                    let args = mem::take(args);
                    let new_block: BlockData = BlockData::new_goto(span, target);
                    let old_block = mem::replace(block, new_block);
                    collect_spec_closure(span, false, args, old_block.statements);
                }
                _ => (),
            }
        }
    }
}

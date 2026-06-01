use crate::{
    ast::{TraitMethodId, TranslatedCrate, TyKind, TypeDecl, TypeId, TypeSpecBodyId},
    ids::IndexMap,
    transform::{TransformCtx, ctx::TransformPass},
};

pub struct Transform;
impl TransformPass for Transform {
    fn transform_ctx(&self, ctx: &mut TransformCtx) {
        fn collect_trait_impl(
            crate_: &mut TranslatedCrate,
            trait_name: &[&str],
            mut collect: impl FnMut(&mut TypeDecl, IndexMap<TraitMethodId, TypeSpecBodyId>),
        ) {
            let trait_ids: Vec<_> = crate_
                .trait_decls
                .iter_enumerated()
                .filter_map(|(trait_id, trait_)| {
                    if trait_.item_meta.name.equals_ref_name(trait_name) {
                        Some(trait_id)
                    } else {
                        None
                    }
                })
                .collect();
            for trait_id in trait_ids {
                let trait_ = crate_.trait_decls.remove(trait_id).unwrap();
                for method in trait_.methods {
                    crate_.fun_decls.remove(method.skip_binder.item.id);
                }

                let trait_impl_ids: Vec<_> = crate_
                    .trait_impls
                    .iter_enumerated()
                    .filter_map(|(trait_impl_id, trait_impl)| {
                        if trait_impl.impl_trait.id == trait_id {
                            Some(trait_impl_id)
                        } else {
                            None
                        }
                    })
                    .collect();
                for trait_impl_id in trait_impl_ids {
                    let trait_impl = crate_.trait_impls.remove(trait_impl_id).unwrap();
                    let self_type = trait_impl.impl_trait.self_ty(crate_).unwrap();
                    let TyKind::Adt(type_ref) = self_type.kind() else {
                        unreachable!();
                    };
                    let TypeId::Adt(type_decl_id) = type_ref.id else {
                        unreachable!();
                    };
                    let body_ids = trait_impl.methods.map(|impl_method| {
                        crate_.type_spec_bodies.push(
                            crate_
                                .fun_decls
                                .remove(impl_method.skip_binder.id)
                                .unwrap()
                                .body,
                        )
                    });
                    collect(&mut crate_.type_decls[type_decl_id], body_ids);
                }
            }
        }

        collect_trait_impl(
            &mut ctx.translated,
            &["trust2_contract", "internal", "TypeInvariant"],
            |type_decl, body_ids| type_decl.specs.invariants.extend(body_ids),
        );
    }
}

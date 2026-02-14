use crate::{
    ast::{FunDeclId, TranslatedCrate, TyKind, TypeDecl, TypeId},
    transform::{TransformCtx, ctx::TransformPass},
};

pub struct Transform;
impl TransformPass for Transform {
    fn transform_ctx(&self, ctx: &mut TransformCtx) {
        fn collect_trait_impl(
            ctx: &mut TranslatedCrate,
            trait_name: &[&str],
            mut collect: impl FnMut(&mut TypeDecl, FunDeclId),
        ) {
            // let trait_ids: Vec<_> = ctx
            //     .trait_decls
            //     .iter_indexed()
            //     .filter_map(|(trait_id, r#trait)|
            //         if r#trait.item_meta.name.equals_ref_name(trait_name) {
            //             Some(trait_id)
            //         } else {
            //             None
            //         }
            //     )
            //     .collect();
            // for trait_id in trait_ids {
            //     let r#trait = ctx.trait_decls.remove(trait_id).unwrap();
            //     for trait_method in r#trait.methods() {
            //         ctx.fun_decls.remove(trait_method.skip_binder.item.id);
            //     }

            //     let trait_impl_ids: Vec<_> = ctx
            //         .trait_impls
            //         .iter_indexed()
            //         .filter_map(|(trait_impl_id, trait_impl)|
            //             if trait_impl.impl_trait.id == trait_id {
            //                 Some(trait_impl_id)
            //             } else {
            //                 None
            //             }
            //         )
            //         .collect();
            //     for trait_impl_id in trait_impl_ids {
            //         let trait_impl = ctx.trait_impls.remove(trait_impl_id).unwrap();
            //         let self_ty = trait_impl.impl_trait.self_ty(ctx).unwrap();
            //         let TyKind::Adt(self_ty) = self_ty.kind() else {
            //             continue;
            //         };
            //         let TypeId::Adt(type_id) = self_ty.id else {
            //             continue;
            //         };
            //         let self_ty = &mut ctx.type_decls[type_id];
            //         for (_, trait_impl_method) in trait_impl.methods {
            //             collect(self_ty, trait_impl_method.skip_binder);
            //         }
            //     }
            // }

            for (trait_id, r#trait) in ctx.trait_decls.iter_indexed() {
                if !r#trait.item_meta.name.equals_ref_name(trait_name) {
                    continue;
                }
                for (_, trait_impl) in ctx.trait_impls.iter_indexed() {
                    if trait_impl.impl_trait.id != trait_id {
                        continue;
                    }
                    let self_ty = trait_impl.impl_trait.self_ty(ctx).unwrap();
                    let TyKind::Adt(self_ty) = self_ty.kind() else {
                        continue;
                    };
                    let TypeId::Adt(type_id) = self_ty.id else {
                        continue;
                    };
                    let self_ty = &mut ctx.type_decls[type_id];
                    for (_, trait_impl_method) in trait_impl.methods.iter() {
                        collect(self_ty, trait_impl_method.skip_binder.id);
                    }
                }
            }
        }

        let ctx = &mut ctx.translated;
        collect_trait_impl(
            ctx,
            &["trust2_contract", "internal", "TypeInvariant"],
            |self_ty, fun_id| self_ty.specs.invariants.push(fun_id),
        );
    }
}

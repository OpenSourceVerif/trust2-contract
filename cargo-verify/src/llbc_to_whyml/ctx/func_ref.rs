//! 函数引用（`FnPtr`）的翻译。
//!
//! 函数引用可能指向普通函数、内建函数或 trait 方法；trait 方法需要通过
//! trait 实现表解析到具体方法，再按普通函数处理。

use super::Ctx;

use crate::llbc_to_whyml::{
    builder::{Builder, ExprBuilder, TermBuilder},
    name_map::LIB_BUILTIN,
};

use charon_lib::ast::{
    BuiltinFunId, FnPtr, FnPtrKind, FunDeclId, FunId, GenericArgs, RefKind, TraitRef, TraitRefKind,
};

use why3_ptree::{
    loc::Position,
    ptree::{Expr, Term},
    ptree_helpers,
};

impl<'a> Ctx<'a> {
    /// 翻译函数引用。
    pub(super) fn translate_func_ref<T: Builder>(&self, func_ref: &FnPtr) -> T::T {
        let translate_builtin_func_ref = |builtin_func_id| {
            let builtin_func_ident = match builtin_func_id {
                BuiltinFunId::BoxNew => "box_new",
                BuiltinFunId::SpecEntry
                | BuiltinFunId::SpecPrecondition
                | BuiltinFunId::SpecPostcondition
                | BuiltinFunId::SpecAssert
                | BuiltinFunId::SpecAssume
                | BuiltinFunId::SpecForAll
                | BuiltinFunId::SpecExists => unreachable!(),
                BuiltinFunId::SpecImplies | BuiltinFunId::SpecOld => todo!(),
                BuiltinFunId::ArrayToSliceShared => todo!(),
                BuiltinFunId::ArrayToSliceMut => todo!(),
                BuiltinFunId::ArrayRepeat => todo!(),
                BuiltinFunId::Index(..) => todo!(),
                BuiltinFunId::PtrFromParts(RefKind::Shared) => "ptr_from_parts_shared",
                BuiltinFunId::PtrFromParts(RefKind::Mut) => "ptr_from_parts_mut",
            };
            T::var(
                Position::default(),
                ptree_helpers::qualid(Box::new([
                    LIB_BUILTIN.1.as_str().into(),
                    builtin_func_ident.into(),
                ])),
            )
        };
        let translate_trait_method_ref =
            |trait_ref: &TraitRef, trait_method_id, generic_args| match &trait_ref.kind {
                TraitRefKind::TraitImpl(trait_impl_ref) => {
                    let generic_args = trait_impl_ref.generics.clone().concat(generic_args);
                    self.translate_func_decl_ref::<T>(
                        self.crate_.trait_impls[trait_impl_ref.id].methods[trait_method_id]
                            .skip_binder
                            .id,
                        &generic_args,
                    )
                }
                TraitRefKind::Clause(..)
                | TraitRefKind::ParentClause(..)
                | TraitRefKind::ItemClause(..)
                | TraitRefKind::SelfId
                | TraitRefKind::BuiltinOrAuto { .. }
                | TraitRefKind::Dyn => todo!(),
                TraitRefKind::Unknown(..) => unreachable!(),
            };
        match &*func_ref.kind {
            FnPtrKind::Fun(FunId::Regular(func_decl_id)) => {
                self.translate_func_decl_ref::<T>(*func_decl_id, &func_ref.generics)
            }
            FnPtrKind::Fun(FunId::Builtin(builtin_func_id)) => {
                translate_builtin_func_ref(*builtin_func_id)
            }
            FnPtrKind::Trait(trait_ref, trait_method_id) => {
                translate_trait_method_ref(trait_ref, *trait_method_id, &func_ref.generics)
            }
        }
    }

    /// `translate_func_ref` 的 `Expr` 版本。
    pub(super) fn translate_func_ref_to_expr(&self, func_ref: &FnPtr) -> Expr {
        self.translate_func_ref::<ExprBuilder>(func_ref)
    }

    /// `translate_func_ref` 的 `Term` 版本。
    pub(super) fn translate_func_ref_to_term(&self, func_ref: &FnPtr) -> Term {
        self.translate_func_ref::<TermBuilder>(func_ref)
    }

    /// 由函数声明的名字生成 WhyML 标识符（要求泛型实参为空）。
    fn translate_func_decl_ref<T: Builder>(
        &self,
        func_decl_id: FunDeclId,
        generic_args: &GenericArgs,
    ) -> T::T {
        assert!(generic_args.types.is_empty());
        assert!(generic_args.const_generics.is_empty());
        assert!(generic_args.trait_refs.is_empty());

        let names = &self.name_map.func_decl_names[func_decl_id];

        T::var(
            Position::default(),
            ptree_helpers::qualid(Box::new([names.item_ident.as_str().into()])),
        )
    }
}

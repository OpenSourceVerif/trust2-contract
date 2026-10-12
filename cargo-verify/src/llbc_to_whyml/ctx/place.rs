//! 位置表达式（`Place`）与操作数（`Operand`）的翻译。
//!
//! 位置与操作数同样可能在可执行代码（`Expr`）和规格（`Term`）中出现，
//! 因此这类翻译统一通过 [`Builder`] 抽象共享同一份实现。

use super::Ctx;

use crate::llbc_to_whyml::{
    builder::{Builder, ExprBuilder, TermBuilder},
    error::Result,
    ident::{
        tuple_field_accessor_ident, variant_constructor_accessor_ident,
        variant_constructor_field_accessor_ident,
    },
    name_map::{ConstructorIdents, TypeDeclSubIdents},
    prelude::LIB_TUPLE_IDENT,
};

use charon_lib::ast::{FieldProjKind, GlobalDeclRef, Operand, Place, PlaceKind, ProjectionElem};

use why3_ptree::{
    loc::Position,
    ptree::{Expr, Qualid, Term},
    ptree_helpers,
};

impl<'a> Ctx<'a> {
    /// 翻译位置表达式：局部变量、字段/元组投影、解引用、全局量。
    ///
    /// WhyML 没有元组投影语法，因此这里只记录用到的元组投影，
    /// 由调用方在 `Tuple` 模块中生成对应的访问函数；枚举的字段访问
    /// 也依赖类型声明翻译时生成的访问函数。
    pub(super) fn translate_place<T: Builder>(&mut self, place: &Place) -> T::T {
        let translate_local = |local_id| {
            T::var(
                Position::default(),
                Qualid(Box::new([self.get_local(local_id).clone()])),
            )
        };
        let translate_proj = |self_: &mut Self, base, proj: &_| {
            let base = self_.translate_place::<T>(base);
            let mut translate_field_proj = |base, proj_kind, field_id| {
                let translate_type_decl_field_proj =
                    |base, type_decl_id, variant_id_opt: Option<_>| {
                        let names = &self_.name_map.type_decl_names[type_decl_id];

                        match &names.sub_idents {
                            TypeDeclSubIdents::Record(field_idents) => T::app(
                                Position::default(),
                                ptree_helpers::qualid(Box::new([field_idents[field_id]
                                    .as_str()
                                    .into()])),
                                Box::new([base]),
                            ),
                            TypeDeclSubIdents::Variant(constructor_idents) => {
                                let ConstructorIdents {
                                    constructor_ident,
                                    record_idents_opt,
                                } = &constructor_idents[variant_id_opt.unwrap()];
                                match record_idents_opt {
                                    None => T::apply(
                                        Position::default(),
                                        T::var(
                                            Position::default(),
                                            Qualid(Box::new([
                                                variant_constructor_field_accessor_ident(
                                                    constructor_ident,
                                                    field_id,
                                                ),
                                            ])),
                                        ),
                                        base,
                                    ),
                                    Some((_record_ident, record_field_idents)) => T::app(
                                        Position::default(),
                                        ptree_helpers::qualid(Box::new([record_field_idents
                                            [field_id]
                                            .as_str()
                                            .into()])),
                                        Box::new([T::apply(
                                            Position::default(),
                                            T::var(
                                                Position::default(),
                                                Qualid(Box::new([
                                                    variant_constructor_accessor_ident(
                                                        constructor_ident,
                                                    ),
                                                ])),
                                            ),
                                            base,
                                        )]),
                                    ),
                                }
                            }
                            TypeDeclSubIdents::Abstract => unreachable!(),
                        }
                    };
                let translate_tuple_field_proj = |self_: &mut Self, base, arity| {
                    self_.record_tuple_field_access(arity, field_id);
                    T::apply(
                        Position::default(),
                        T::var(
                            Position::default(),
                            Qualid(Box::new([
                                LIB_TUPLE_IDENT.clone(),
                                tuple_field_accessor_ident(arity, field_id),
                            ])),
                        ),
                        base,
                    )
                };
                match proj_kind {
                    FieldProjKind::Adt(type_decl_id, variant_id_opt) => {
                        translate_type_decl_field_proj(base, type_decl_id, variant_id_opt)
                    }
                    FieldProjKind::Tuple(arity) => translate_tuple_field_proj(self_, base, arity),
                }
            };
            match proj {
                ProjectionElem::Deref => base,
                ProjectionElem::Field(proj_kind, field_id) => {
                    translate_field_proj(base, *proj_kind, *field_id)
                }
                ProjectionElem::PtrMetadata => todo!(),
                ProjectionElem::Index { .. } | ProjectionElem::Subslice { .. } => unreachable!(),
            }
        };
        match &place.kind {
            PlaceKind::Local(local_id) => translate_local(*local_id),
            PlaceKind::Projection(base, proj) => translate_proj(self, base, proj),
            PlaceKind::Global(global_ref) => self.translate_global_ref::<T>(global_ref),
        }
    }

    /// `translate_place` 的 `Expr` 版本。
    pub(super) fn translate_place_to_expr(&mut self, place: &Place) -> Expr {
        self.translate_place::<ExprBuilder>(place)
    }

    /// `translate_place` 的 `Term` 版本。
    pub(super) fn translate_place_to_term(&mut self, place: &Place) -> Term {
        self.translate_place::<TermBuilder>(place)
    }

    /// 翻译对全局量的引用（要求其泛型实参为空）。
    fn translate_global_ref<T: Builder>(&self, global_ref: &GlobalDeclRef) -> T::T {
        assert!(global_ref.generics.types.is_empty());
        assert!(global_ref.generics.const_generics.is_empty());
        assert!(global_ref.generics.trait_refs.is_empty());

        let names = &self.name_map.global_names[global_ref.id];

        T::var(
            Position::default(),
            ptree_helpers::qualid(Box::new([names.item_ident.as_str().into()])),
        )
    }

    /// 翻译操作数：位置的拷贝/移动，或常量表达式。
    pub(super) fn translate_operand<T: Builder>(&mut self, operand: &Operand) -> Result<T::T> {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => Ok(self.translate_place::<T>(place)),
            Operand::Const(const_expr) => self.translate_const_expr::<T>(const_expr),
        }
    }

    /// `translate_operand` 的 `Expr` 版本。
    pub(super) fn translate_operand_to_expr(&mut self, operand: &Operand) -> Result<Expr> {
        self.translate_operand::<ExprBuilder>(operand)
    }

    /// `translate_operand` 的 `Term` 版本。
    pub(super) fn translate_operand_to_term(&mut self, operand: &Operand) -> Result<Term> {
        self.translate_operand::<TermBuilder>(operand)
    }
}

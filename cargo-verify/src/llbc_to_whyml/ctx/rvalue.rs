//! 右值（`Rvalue`）的翻译。
//!
//! 包括借用、二元/一元运算（按值的类型调用对应支撑库中的运算）、
//! 以及聚合（结构体、枚举、元组）的构造。

use super::Ctx;

use itertools::Itertools;

use crate::llbc_to_whyml::{
    builder::{Builder, ExprBuilder, TermBuilder},
    error::{Error, Result},
    ident::{EQUAL, GREATER, GREATER_EQUAL, LESS, LESS_EQUAL, NOT_EQUAL},
    name_map::{ConstructorIdents, TypeDeclSubIdents},
};

use charon_lib::{
    ast::{
        AggregateKind, BinOp, BorrowKind, NullOp, Operand, OverflowMode, Rvalue, TyKind,
        TypeDeclRef, TypeId, UnOp,
    },
    ids::IndexVec,
};

use why3_ptree::{
    loc::Position,
    ptree::{Expr, Term},
    ptree_helpers,
};

impl<'a> Ctx<'a> {
    /// 翻译右值。
    ///
    /// 借用直接翻译为被借用的位置；运算根据操作数类型调用支撑库中的函数
    /// （比较运算翻译为中缀运算符，其他运算翻译为函数调用）；
    /// 聚合则翻译成记录/构造子调用/元组。
    fn translate_rvalue<T: Builder>(&mut self, rvalue: &Rvalue) -> Result<T::T> {
        let translate_ref = |self_: &mut Self, place, kind, _ptr_metadata| {
            let place = self_.translate_place::<T>(place);
            match kind {
                BorrowKind::Shared | BorrowKind::UniqueImmutable => place,
                BorrowKind::Mut => todo!(),
                BorrowKind::TwoPhaseMut => todo!(),
                BorrowKind::Shallow => todo!(),
            }
        };
        let translate_binary_op = |self_: &mut Self, operator, lhs: &Operand, rhs: &Operand| {
            let lhs_type = lhs.ty();
            let lhs = self_.translate_operand::<T>(lhs)?;
            let rhs_type = rhs.ty();
            let rhs = self_.translate_operand::<T>(rhs)?;
            let lhs_type = match lhs_type.kind() {
                TyKind::Literal(lit_type) => *lit_type,
                _ => todo!(),
            };
            let lhs_type_name = self_.translate_literal_type_name(lhs_type).as_str();
            let rhs_type = match rhs_type.kind() {
                TyKind::Literal(lit_type) => *lit_type,
                _ => todo!(),
            };
            let apply = |lhs, rhs, func_ident: &str| {
                self_.translate_apply::<T>(
                    T::var(
                        Position::default(),
                        ptree_helpers::qualid(Box::new([lhs_type_name.into(), func_ident.into()])),
                    ),
                    [lhs, rhs],
                )
            };
            let infix = |lhs, rhs, operator_ident| {
                T::scope(
                    Position::default(),
                    ptree_helpers::qualid(Box::new([lhs_type_name.into()])),
                    T::infix(Position::default(), lhs, operator_ident, rhs),
                )
            };
            let apply_rhs_int = |lhs, rhs, func_ident: &str| {
                apply(
                    lhs,
                    T::apply(
                        Position::default(),
                        T::var(
                            Position::default(),
                            ptree_helpers::qualid(Box::new([
                                self_.translate_literal_type_name(rhs_type).as_str().into(),
                                "to_int".into(),
                            ])),
                        ),
                        rhs,
                    ),
                    func_ident,
                )
            };
            Ok(match operator {
                BinOp::BitXor => apply(lhs, rhs, "bit_xor"),
                BinOp::BitAnd => apply(lhs, rhs, "bit_and"),
                BinOp::BitOr => apply(lhs, rhs, "bit_or"),
                BinOp::Eq => infix(lhs, rhs, EQUAL.clone()),
                BinOp::Lt => infix(lhs, rhs, LESS.clone()),
                BinOp::Le => infix(lhs, rhs, LESS_EQUAL.clone()),
                BinOp::Ne => infix(lhs, rhs, NOT_EQUAL.clone()),
                BinOp::Ge => infix(lhs, rhs, GREATER_EQUAL.clone()),
                BinOp::Gt => infix(lhs, rhs, GREATER.clone()),
                BinOp::Add(OverflowMode::Panic | OverflowMode::UB) => apply(lhs, rhs, "add_strict"),
                BinOp::Add(OverflowMode::Wrap) => apply(lhs, rhs, "add_wrapping"),
                BinOp::Sub(OverflowMode::Panic | OverflowMode::UB) => apply(lhs, rhs, "sub_strict"),
                BinOp::Sub(OverflowMode::Wrap) => apply(lhs, rhs, "sub_wrapping"),
                BinOp::Mul(OverflowMode::Panic | OverflowMode::UB) => apply(lhs, rhs, "mul_strict"),
                BinOp::Mul(OverflowMode::Wrap) => apply(lhs, rhs, "mul_wrapping"),
                BinOp::Div(OverflowMode::Panic | OverflowMode::UB) => apply(lhs, rhs, "div_strict"),
                BinOp::Div(OverflowMode::Wrap) => unreachable!(),
                BinOp::Rem(OverflowMode::Panic | OverflowMode::UB) => apply(lhs, rhs, "rem_strict"),
                BinOp::Rem(OverflowMode::Wrap) => unreachable!(),
                BinOp::AddChecked => apply(lhs, rhs, "add_overflowing"),
                BinOp::SubChecked => apply(lhs, rhs, "sub_overflowing"),
                BinOp::MulChecked => apply(lhs, rhs, "mul_overflowing"),
                BinOp::Shl(OverflowMode::Panic | OverflowMode::UB) => {
                    apply_rhs_int(lhs, rhs, "shl_strict")
                }
                BinOp::Shl(OverflowMode::Wrap) => apply_rhs_int(lhs, rhs, "shl_unbounded"),
                BinOp::Shr(OverflowMode::Panic | OverflowMode::UB) => {
                    apply_rhs_int(lhs, rhs, "shr_strict")
                }
                BinOp::Shr(OverflowMode::Wrap) => apply_rhs_int(lhs, rhs, "shr_unbounded"),
                BinOp::Offset => return Err(Error::RawPointer),
                BinOp::Cmp => apply(lhs, rhs, "cmp"),
            })
        };
        let translate_unary_op = |self_: &mut Self, operator: &_, operand: &Operand| {
            let operand_type = operand.ty();
            let operand = self_.translate_operand::<T>(operand)?;
            let operand_type = match operand_type.kind() {
                TyKind::Literal(lit_type) => *lit_type,
                _ => todo!(),
            };
            let apply = |func_ident: &str| {
                T::apply(
                    Position::default(),
                    T::var(
                        Position::default(),
                        ptree_helpers::qualid(Box::new([
                            self_
                                .translate_literal_type_name(operand_type)
                                .as_str()
                                .into(),
                            func_ident.into(),
                        ])),
                    ),
                    operand,
                )
            };
            Ok(match operator {
                UnOp::Not => apply("bit_not"),
                UnOp::Neg(OverflowMode::Panic | OverflowMode::UB) => unreachable!(),
                UnOp::Neg(OverflowMode::Wrap) => apply("neg"),
                UnOp::Cast(..) => todo!(),
            })
        };
        let translate_nullary_op = |operator: &_, _ty| match operator {
            NullOp::SizeOf => todo!(),
            NullOp::AlignOf => todo!(),
            NullOp::OffsetOf(_type_ref, _variant_id_opt, _field_id) => todo!(),
            NullOp::UbChecks => todo!(),
            NullOp::OverflowChecks => todo!(),
            NullOp::ContractChecks => todo!(),
        };
        let translate_aggregate = |self_: &mut Self, kind: &_, operands: &[_]| {
            let translate_type_decl_aggregate =
                |self_: &mut Self, type_decl_id, variant_id_opt: Option<_>, _field_id_opt| {
                    let names = &self_.name_map.type_decl_names[type_decl_id];

                    let translate_struct_aggregate =
                        |self_: &mut Self, field_idents: &IndexVec<_, _>| {
                            Ok(T::record(
                                Position::default(),
                                field_idents
                                    .iter()
                                    .zip(operands)
                                    .map(|(field_ident, operand): (&String, _)| {
                                        Ok((
                                            ptree_helpers::qualid(Box::new([field_ident
                                                .as_str()
                                                .into()])),
                                            self_.translate_operand::<T>(operand)?,
                                        ))
                                    })
                                    .try_collect()?,
                            ))
                        };
                    let translate_enum_aggregate =
                        |self_: &mut Self,
                         ConstructorIdents {
                             constructor_ident,
                             record_idents_opt,
                         }: &_| {
                            let constructor_expr = T::var(
                                Position::default(),
                                ptree_helpers::qualid(Box::new([constructor_ident
                                    .as_str()
                                    .into()])),
                            );
                            Ok(match record_idents_opt {
                                None => {
                                    let operands: Vec<_> = operands
                                        .iter()
                                        .map(|operand| self_.translate_operand::<T>(operand))
                                        .try_collect()?;
                                    self_.translate_apply::<T>(constructor_expr, operands)
                                }
                                Some((_record_ident, record_field_idents)) => T::apply(
                                    Position::default(),
                                    constructor_expr,
                                    translate_struct_aggregate(self_, record_field_idents)?,
                                ),
                            })
                        };
                    match &names.sub_idents {
                        TypeDeclSubIdents::Record(field_idents) => {
                            translate_struct_aggregate(self_, field_idents)
                        }
                        TypeDeclSubIdents::Variant(constructor_idents) => translate_enum_aggregate(
                            self_,
                            &constructor_idents[variant_id_opt.unwrap()],
                        ),
                        TypeDeclSubIdents::Abstract => unreachable!(),
                    }
                };
            let translate_tuple_aggregate = |self_: &mut Self| {
                Ok(T::tuple(
                    Position::default(),
                    operands
                        .iter()
                        .map(|operand| self_.translate_operand::<T>(operand))
                        .try_collect()?,
                ))
            };
            match kind {
                AggregateKind::Adt(
                    TypeDeclRef {
                        id: TypeId::Adt(type_decl_id),
                        ..
                    },
                    variant_id_opt,
                    field_id_opt,
                ) => translate_type_decl_aggregate(
                    self_,
                    *type_decl_id,
                    *variant_id_opt,
                    *field_id_opt,
                ),
                AggregateKind::Adt(
                    TypeDeclRef {
                        id: TypeId::Tuple, ..
                    },
                    _,
                    _,
                ) => translate_tuple_aggregate(self_),
                AggregateKind::Adt(
                    TypeDeclRef {
                        id: TypeId::Builtin(..),
                        ..
                    },
                    _,
                    _,
                ) => unreachable!(),
                AggregateKind::Array(..) => todo!(),
                AggregateKind::RawPtr(..) => Err(Error::RawPointer),
            }
        };

        match rvalue {
            Rvalue::Use(operand, _) => self.translate_operand::<T>(operand),
            Rvalue::Ref {
                place,
                kind,
                ptr_metadata,
            } => Ok(translate_ref(self, place, *kind, ptr_metadata)),
            Rvalue::RawPtr {
                place: _,
                kind: _,
                ptr_metadata: _,
            } => Err(Error::RawPointer),
            Rvalue::BinaryOp(operator, lhs, rhs) => translate_binary_op(self, *operator, lhs, rhs),
            Rvalue::UnaryOp(operator, operand) => translate_unary_op(self, operator, operand),
            Rvalue::NullaryOp(operator, ty) => translate_nullary_op(operator, ty),
            Rvalue::Discriminant(..) => todo!(),
            Rvalue::Aggregate(kind, operands) => translate_aggregate(self, kind, operands),
            Rvalue::Len(..) => todo!(),
            Rvalue::Repeat(..) => unreachable!(),
        }
    }

    /// `translate_rvalue` 的 `Expr` 版本。
    pub(super) fn translate_rvalue_to_expr(&mut self, rvalue: &Rvalue) -> Result<Expr> {
        self.translate_rvalue::<ExprBuilder>(rvalue)
    }

    /// `translate_rvalue` 的 `Term` 版本。
    pub(super) fn translate_rvalue_to_term(&mut self, rvalue: &Rvalue) -> Result<Term> {
        self.translate_rvalue::<TermBuilder>(rvalue)
    }
}

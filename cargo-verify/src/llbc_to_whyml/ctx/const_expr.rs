//! 常量表达式与字面量的翻译。

use super::Ctx;

use crate::llbc_to_whyml::{
    builder::{Builder, ExprBuilder},
    error::{Error, Result},
};

use charon_lib::ast::{ConstantExpr, ConstantExprKind, FloatValue, Literal, ScalarValue};

use itertools::Itertools;

use malachite::{Integer, base::num::basic::traits::Zero};

use rustc_apfloat::{
    ExpInt, Float,
    ieee::{Quad, QuadS, Semantics},
};

use why3_ptree::{
    constant::Constant,
    loc::Position,
    number::{IntConstant, IntLiteralKind, RealConstant, RealLiteralKind, RealValue},
    ptree::Expr,
    ptree_helpers,
};

impl<'a> Ctx<'a> {
    /// 翻译常量表达式。目前支持字面量、函数调用与函数引用，
    /// 其余形式（数组、trait 常量等）尚未支持。
    pub(super) fn translate_const_expr<T: Builder>(
        &self,
        const_expr: &ConstantExpr,
    ) -> Result<T::T> {
        let translate_call = |func_ref, args: &Vec<_>| {
            let args: Vec<_> = args
                .iter()
                .map(|arg| self.translate_const_expr::<T>(arg))
                .try_collect()?;
            Ok(self.translate_apply::<T>(self.translate_func_ref::<T>(func_ref), args))
        };
        match &const_expr.kind {
            ConstantExprKind::Literal(literal) => Ok(self.translate_literal::<T>(literal)),
            ConstantExprKind::Adt(..) => Ok(T::unit_value()),
            ConstantExprKind::Array(..) => unreachable!(),
            ConstantExprKind::Global(..) => unreachable!(),
            ConstantExprKind::TraitConst(..) => todo!(),
            ConstantExprKind::VTableRef(..) => unreachable!(),
            ConstantExprKind::Ref(..) | ConstantExprKind::Ptr(..) => unreachable!(),
            ConstantExprKind::Var(..) => todo!(),
            ConstantExprKind::Call(func_ref, args) => translate_call(func_ref, args),
            ConstantExprKind::FnDef(func_ref) => Ok(self.translate_func_ref::<T>(func_ref)),
            ConstantExprKind::FnPtr(..) => unreachable!(),
            ConstantExprKind::TypeId(..) => todo!(),
            ConstantExprKind::PtrNoProvenance(..) => unreachable!(),
            ConstantExprKind::RawMemory(..) => Err(Error::RawBytesConst),
            ConstantExprKind::Opaque(..) => unreachable!(),
        }
    }

    /// `translate_const_expr` 的 `Expr` 版本。
    pub(super) fn translate_const_expr_to_expr(&self, const_expr: &ConstantExpr) -> Result<Expr> {
        self.translate_const_expr::<ExprBuilder>(const_expr)
    }

    /// 翻译字面量。
    ///
    /// 整数用对应的支撑库（如 `I32.of_int`）构造，浮点数按 IEEE754 格式
    /// 拆成尾数与二进制指数，字符按 Unicode 码点表示为整数，字符串直接作为字符串常量。
    fn translate_literal<T: Builder>(&self, literal: &Literal) -> T::T {
        let translate_int_literal = |int_literal| {
            let (lib_alias, value) = match int_literal {
                ScalarValue::Unsigned(uint_type, value) => {
                    (self.translate_uint_type_name(uint_type), value.into())
                }
                ScalarValue::Signed(int_type, value) => {
                    (self.translate_int_type_name(int_type), value.into())
                }
            };
            T::apply(
                Position::default(),
                T::var(
                    Position::default(),
                    ptree_helpers::qualid(Box::new([lib_alias.as_str().into(), "of_int".into()])),
                ),
                T::const_(
                    Position::default(),
                    Constant::Int(IntConstant {
                        kind: IntLiteralKind::Unk,
                        int: value,
                    }),
                ),
            )
        };
        let translate_float_literal = |float_literal: &FloatValue| {
            let value: Quad = float_literal.value.parse().unwrap();
            if !value.is_finite() {
                todo!();
            }
            let raw = value.to_bits();

            let exp = (raw << 1 >> Quad::PRECISION) as ExpInt;
            let mut significand = (raw << (1 + QuadS::EXP_BITS) >> (1 + QuadS::EXP_BITS)) as i128;
            let exp = if exp == 0 {
                Quad::MIN_EXP
            } else {
                significand |= 1 << (Quad::PRECISION - 1);
                Quad::MIN_EXP - 1 + exp
            } - (Quad::PRECISION - 1) as ExpInt;
            if value.is_negative() {
                significand = -significand;
            }

            T::const_(
                Position::default(),
                Constant::Real(RealConstant {
                    kind: RealLiteralKind::Unk,
                    real: RealValue {
                        sig: significand.into(),
                        pow2: exp.into(),
                        pow5: Integer::ZERO,
                    },
                }),
            )
        };
        let translate_bool_literal = |bool_literal| {
            if bool_literal {
                T::true_()
            } else {
                T::false_()
            }
        };
        let translate_char_literal = |char_literal| {
            T::const_(
                Position::default(),
                Constant::Int(IntConstant {
                    kind: IntLiteralKind::Unk,
                    int: u32::from(char_literal).into(),
                }),
            )
        };
        let translate_str_literal =
            |str_literal: &str| T::const_(Position::default(), Constant::Str(str_literal.into()));
        match literal {
            Literal::Scalar(int_literal) => translate_int_literal(*int_literal),
            Literal::Float(float_literal) => translate_float_literal(float_literal),
            Literal::Bool(bool_literal) => translate_bool_literal(*bool_literal),
            Literal::Char(char_literal) => translate_char_literal(*char_literal),
            Literal::ByteStr(..) => todo!(),
            Literal::Str(str_literal) => translate_str_literal(str_literal),
        }
    }
}

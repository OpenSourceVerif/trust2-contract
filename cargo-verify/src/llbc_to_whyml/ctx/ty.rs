//! LLBC 类型到 WhyML 类型的翻译。

use super::Ctx;

use crate::llbc_to_whyml::{
    error::{Error, Result},
    name_map::{
        LIB_BOOL, LIB_CHAR, LIB_I8, LIB_I16, LIB_I32, LIB_I64, LIB_I128, LIB_U8, LIB_U16, LIB_U32,
        LIB_U64, LIB_U128,
    },
    prelude::{EMPTY, STRING, UNIT_TYPE},
};

use charon_lib::ast::{
    BuiltinTy, FunSig, GenericArgs, IntTy, LiteralTy, RefKind, Ty, TyKind, TypeDbVar, TypeDeclRef,
    TypeId, UIntTy,
};

use why3_ptree::{ptree::Pty, ptree_helpers};

impl<'a> Ctx<'a> {
    /// 翻译类型变量（泛型参数）；它由 `with_generic_params` 绑定在栈上。
    fn translate_type_var(&self, type_var: TypeDbVar) -> Pty {
        let TypeDbVar::Bound(db_id, id) = type_var else {
            unreachable!();
        };
        Pty::Tyvar(self.type_param_stack[db_id][id].clone())
    }

    // Currently, this is only used on `BuiltinTy` and `TypeId::Tuple`.
    fn translate_generic_args(&self, generic_args: &GenericArgs) -> Result<Vec<Pty>> {
        assert!(generic_args.const_generics.is_empty());
        assert!(generic_args.trait_refs.is_empty());
        generic_args
            .types
            .iter()
            .map(|ty| self.translate_type(ty))
            .collect()
    }

    /// 翻译 LLBC 类型。
    ///
    /// 注意可变引用被表示为一对值：新值与它「指向」的原值。
    pub(super) fn translate_type(&self, ty: &Ty) -> Result<Pty> {
        let translate_ref_type = |ty, ref_kind| {
            let ty = self.translate_type(ty)?;
            Ok(match ref_kind {
                RefKind::Shared => ty,
                RefKind::Mut => Pty::Tuple(Box::new([ty.clone(), ty])),
            })
        };
        let translate_func_type = |func_sig: &FunSig| {
            let mut ty = self.translate_type(&func_sig.output)?;
            if func_sig.inputs.is_empty() {
                ty = Pty::Arrow(Box::new(UNIT_TYPE.clone()), Box::new(ty));
            } else {
                for param_type in &func_sig.inputs {
                    ty = Pty::Arrow(Box::new(self.translate_type(param_type)?), Box::new(ty));
                }
            }
            Ok(ty)
        };
        let translate_array_type = |_ty| {
            todo!();
            // Ok(Pty::Tyapp(
            //     ARRAY.clone(),
            //     Box::new([self.translate_type(ty)?]),
            // ))
        };
        match ty.kind() {
            TyKind::Adt(type_ref) => self.translate_type_ref(type_ref),
            TyKind::TypeVar(type_var) => Ok(self.translate_type_var(*type_var)),
            TyKind::Literal(lit_type) => Ok(self.translate_literal_type(*lit_type)),
            TyKind::Never => Ok(EMPTY.clone()),
            TyKind::Ref(_region, ty, ref_kind) => translate_ref_type(ty, *ref_kind),
            TyKind::RawPtr(..) => Err(Error::RawPointer),
            TyKind::TraitType(..) => todo!(),
            TyKind::DynTrait(..) => todo!(),
            TyKind::FnPtr(func_sig) => translate_func_type(&func_sig.skip_binder),
            TyKind::FnDef(..) => unreachable!(),
            TyKind::PtrMetadata(..) => todo!(),
            TyKind::Array(ty, _n) => translate_array_type(ty),
            TyKind::Slice(..) => todo!(),
            TyKind::Pattern(ty, _pattern) => self.translate_type(ty),
            TyKind::Error(..) => unreachable!(),
        }
    }

    /// 翻译具名类型引用：结构体/枚举、元组与内建类型。
    fn translate_type_ref(&self, type_ref: &TypeDeclRef) -> Result<Pty> {
        let TypeDeclRef {
            id,
            generics: generic_args,
        } = type_ref;
        let translate_builtin_type = |builtin_type: &_| {
            Ok(match builtin_type {
                BuiltinTy::Box => self
                    .translate_generic_args(generic_args)?
                    .into_iter()
                    .next()
                    .unwrap(),
                BuiltinTy::Str => STRING.clone(),
            })
        };
        match id {
            TypeId::Adt(type_decl_id) => {
                let names = &self.name_map.type_decl_names[*type_decl_id];

                Ok(Pty::Tyapp(
                    ptree_helpers::qualid(Box::new([names.item_ident.as_str().into()])),
                    Box::new([]),
                ))
            }
            TypeId::Tuple => Ok(Pty::Tuple(
                self.translate_generic_args(generic_args)?.into(),
            )),
            TypeId::Builtin(builtin_type) => translate_builtin_type(builtin_type),
        }
    }

    /// 翻译基本类型（整数、布尔、字符）为对应支撑库中的类型。
    fn translate_literal_type(&self, lit_type: LiteralTy) -> Pty {
        Pty::Tyapp(
            ptree_helpers::qualid(Box::new([
                self.translate_literal_type_name(lit_type).as_str().into(),
                "t".into(),
            ])),
            Box::new([]),
        )
    }

    /// 基本类型对应的支撑库模块别名。
    pub(super) fn translate_literal_type_name(&self, lit_type: LiteralTy) -> &String {
        match lit_type {
            LiteralTy::Int(int_type) => self.translate_int_type_name(int_type),
            LiteralTy::UInt(uint_type) => self.translate_uint_type_name(uint_type),
            LiteralTy::Float(_float_type) => todo!(),
            LiteralTy::Bool => &LIB_BOOL.1,
            LiteralTy::Char => &LIB_CHAR.1,
        }
    }

    /// 有符号整数类型对应的支撑库模块别名。
    pub(super) fn translate_int_type_name(&self, int_type: IntTy) -> &String {
        &match int_type {
            IntTy::Isize => todo!(),
            IntTy::I8 => &LIB_I8,
            IntTy::I16 => &LIB_I16,
            IntTy::I32 => &LIB_I32,
            IntTy::I64 => &LIB_I64,
            IntTy::I128 => &LIB_I128,
        }
        .1
    }

    /// 无符号整数类型对应的支撑库模块别名。
    pub(super) fn translate_uint_type_name(&self, uint_type: UIntTy) -> &String {
        &match uint_type {
            UIntTy::Usize => todo!(),
            UIntTy::U8 => &LIB_U8,
            UIntTy::U16 => &LIB_U16,
            UIntTy::U32 => &LIB_U32,
            UIntTy::U64 => &LIB_U64,
            UIntTy::U128 => &LIB_U128,
        }
        .1
    }
}

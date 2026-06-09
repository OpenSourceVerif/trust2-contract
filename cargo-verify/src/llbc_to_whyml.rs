use name_map::{
    ConstructorIdents, EMPTY_TYPE_NAME, LIB_BOOL, LIB_BUILTIN, LIB_CHAR, LIB_DIR, LIB_I8, LIB_I16,
    LIB_I32, LIB_I64, LIB_I128, LIB_TUPLE, LIB_U8, LIB_U16, LIB_U32, LIB_U64, LIB_U128, LocalNames,
    NameMap, TypeDeclSubIdents, TypeParamNames,
};

use bumpalo::Bump;
use charon_lib::{
    ast::{
        self, AggregateKind, Assert, BinOp, BindingStack, Body, BorrowKind, BoxedArgs,
        BuiltinFunId, BuiltinTy, Call, ConstantExpr, ConstantExprKind, CopyNonOverlapping,
        DeclarationGroup, FieldId, FieldProjKind, FloatValue, FnOperand, FnPtr, FnPtrKind,
        FunDeclId, FunId, FunSig, GDeclarationGroup, GenericArgs, GlobalDeclRef, ItemId, Literal,
        LiteralTy, LocalId, NullOp, Operand, Place, PlaceKind, ProjectionElem, RefKind, Rvalue,
        ScalarValue, TranslatedCrate, Ty, TyKind, TypeDbVar, TypeDeclId, TypeDeclKind, TypeDeclRef,
        TypeId, TypeVarId, UIntTy, UnOp, VariantId,
    },
    ids::IndexVec,
    llbc_ast::{Block, ExprBody, Statement, StatementKind, Switch},
    ullbc_ast::{AbortKind, GlobalDeclId, IntTy, ItemSource, OverflowMode},
};
use include_dir::{Dir, include_dir};
use itertools::Itertools;
use malachite::{Integer, base::num::basic::traits::Zero};
use ocaml_format::{Doc, FormattingOptions};
use rustc_apfloat::{
    ExpInt, Float,
    ieee::{Quad, QuadS, Semantics},
};
use why3_ptree::{
    constant::Constant,
    expr::RsKind,
    ident::{self, OP_EQU},
    ity::Mask,
    loc::Position,
    mlw_printer,
    number::{IntConstant, IntLiteralKind, RealConstant, RealLiteralKind, RealValue},
    pmodule::REF_ATTR,
    ptree::{
        self, Attr, Binder, Decl, Expr, ExprDesc, Fundef, Ghost, Ident, MlwFile, Param, PatDesc,
        Pattern, Pty, Qualid, Spec, TypeDef, Visibility,
    },
    ptree_helpers,
};

use std::{
    collections::{HashMap, HashSet},
    error,
    fmt::{self, Display, Formatter},
    fs::{self, File},
    io::Write,
    iter,
    path::Path,
    result,
    sync::LazyLock,
};

mod name_map;

#[derive(Debug)]
pub enum Error {
    MixedDeclGroup(Vec<ItemId>),
    Union(TypeDeclId),
    RawPointer,
    RawBytesConst,
    InlineAsm,
}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Error::MixedDeclGroup(item_ids) => {
                write!(
                    f,
                    "unsupported mixed declaration group: {}",
                    item_ids.iter().format(", ")
                )
            }
            Error::Union(type_decl_id) => {
                write!(f, "unsupported union: {type_decl_id}")
            }
            Error::RawPointer => write!(f, "unsupported raw pointer"),
            Error::RawBytesConst => write!(f, "unsupported constant in raw byte representation"),
            Error::InlineAsm => write!(f, "unsupported inline assembly"),
        }
    }
}

pub type Result<T> = result::Result<T, Error>;

pub fn translate_crates(
    crates: &mut HashMap<String, TranslatedCrate>,
    why3_out_dir: &Path,
) -> anyhow::Result<()> {
    fn write_file(path: impl AsRef<Path>, whyml_file: &MlwFile) -> anyhow::Result<()> {
        let mut file = File::create(path)?;
        let arena = Bump::new();
        let mut doc = Doc::new();
        mlw_printer::pp_mlw_file(None, &mut doc, &arena, whyml_file);
        write!(file, "{}", doc.display(&FormattingOptions::default()))?;
        Ok(())
    }

    let mut tuple_field_accesses = HashSet::new();
    for (crate_name_, crate_) in crates {
        let (whyml_file, tuple_field_accesses_) = translate_crate(crate_)?;

        write_file(why3_out_dir.join(format!("{crate_name_}.mlw")), &whyml_file)?;

        tuple_field_accesses.extend(tuple_field_accesses_);
    }

    const WHYML_LIB: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/llbc_to_whyml/whyml_lib");
    let whyml_lib_out_dir = why3_out_dir.join("whyml_lib");
    fs::create_dir_all(&whyml_lib_out_dir)?;
    WHYML_LIB.extract(&whyml_lib_out_dir)?;

    fn translate_tuple_field_access(arity: usize, field_id: FieldId) -> Decl {
        Decl::Let(
            tuple_field_accessor_ident(arity, field_id),
            false,
            RsKind::None,
            Box::new(ptree_helpers::expr(
                Position::default(),
                ExprDesc::Fun(
                    ptree_helpers::one_binder(
                        Position::default(),
                        None,
                        None,
                        name_map::local_temp_name(0).into(),
                    ),
                    None,
                    WILDCARD.clone(),
                    Mask::Visible,
                    ptree_helpers::empty_spec(),
                    Box::new(ptree_helpers::expr(
                        Position::default(),
                        ExprDesc::Match(
                            Box::new(ptree_helpers::evar(
                                Position::default(),
                                Qualid(Box::new([local_temp_ident(0)])),
                            )),
                            Box::new([(
                                ptree_helpers::pat(
                                    Position::default(),
                                    PatDesc::Tuple(
                                        (0..arity)
                                            .map(|field_id_| {
                                                if field_id_ == field_id {
                                                    ptree_helpers::pat_var(
                                                        Position::default(),
                                                        local_temp_ident(1),
                                                    )
                                                } else {
                                                    WILDCARD.clone()
                                                }
                                            })
                                            .collect(),
                                    ),
                                ),
                                ptree_helpers::evar(
                                    Position::default(),
                                    Qualid(Box::new([local_temp_ident(1)])),
                                ),
                            )]),
                            Box::new([]),
                        ),
                    )),
                ),
            )),
        )
    }

    let mut tuple_path = whyml_lib_out_dir;
    tuple_path.push("tuple.mlw");
    let tuple_decls = tuple_field_accesses
        .into_iter()
        .map(|(arity, field_id)| translate_tuple_field_access(arity, field_id))
        .collect();
    let tuple_whyml_file =
        MlwFile::Modules(Box::new([(translate_ident("Tuple".into()), tuple_decls)]));
    write_file(tuple_path, &tuple_whyml_file)?;

    Ok(())
}

fn translate_crate(crate_: &mut TranslatedCrate) -> Result<(MlwFile, HashSet<(usize, FieldId)>)> {
    let name_map = name_map::build(crate_);

    let mut whyml_decls = Vec::new();
    whyml_decls.push(IMPORTS.clone());
    whyml_decls.push(IMPORT_REF.clone());

    let mut ctx = Ctx {
        crate_,
        name_map: &name_map,
        whyml_decls: &mut whyml_decls,
        tuple_field_accesses: HashSet::new(),
        type_param_stack: BindingStack::empty(),
        locals: None,
        loop_depth: 0,
    };
    for decl_group in crate_.ordered_decls.as_ref().unwrap() {
        ctx.translate_decl_group(decl_group)?;
    }

    let tuple_field_accesses = ctx.tuple_field_accesses;
    Ok((MlwFile::Decls(whyml_decls.into()), tuple_field_accesses))
}

fn translate_ident(ident: Box<str>) -> Ident {
    ptree_helpers::ident(None, Position::default(), ident)
}

struct Ctx<'a> {
    crate_: &'a TranslatedCrate,
    name_map: &'a NameMap,
    whyml_decls: &'a mut Vec<Decl>,
    tuple_field_accesses: HashSet<(usize, FieldId)>,
    type_param_stack: BindingStack<IndexVec<TypeVarId, Ident>>,
    locals: Option<IndexVec<LocalId, Ident>>,
    loop_depth: usize,
}

impl<'a> Ctx<'a> {
    fn record_tuple_field_access(&mut self, arity: usize, field_id: FieldId) {
        self.tuple_field_accesses.insert((arity, field_id));
    }

    fn push_decl(&mut self, decl: Decl) {
        self.whyml_decls.push(decl);
    }

    fn extend_decls(&mut self, decls: impl IntoIterator<Item = Decl>) {
        self.whyml_decls.extend(decls);
    }

    fn with_generic_params<T>(
        &mut self,
        type_param_names: &TypeParamNames,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.type_param_stack
            .push(type_param_names.map_ref(|ident| translate_ident(ident.as_str().into())));
        let result = f(self);
        self.type_param_stack.pop();
        result
    }

    fn set_locals(&mut self, local_names: &IndexVec<LocalId, String>) {
        self.locals = Some(local_names.map_ref(|ident| translate_ident(ident.as_str().into())));
    }

    fn get_local(&self, local_id: LocalId) -> &Ident {
        &self.locals.as_ref().unwrap()[local_id]
    }

    fn enter_loop<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.loop_depth += 1;
        let result = f(self);
        self.loop_depth -= 1;
        result
    }

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

    fn translate_type(&self, ty: &Ty) -> Result<Pty> {
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
        let translate_array_type = |ty| {
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
            TyKind::Pattern(ty, pattern) => self.translate_type(ty),
            TyKind::Error(..) => unreachable!(),
        }
    }

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

    fn translate_literal_type(&self, lit_type: LiteralTy) -> Pty {
        Pty::Tyapp(
            ptree_helpers::qualid(Box::new([
                self.translate_literal_type_name(lit_type).as_str().into(),
                "t".into(),
            ])),
            Box::new([]),
        )
    }

    fn translate_literal_type_name(&self, lit_type: LiteralTy) -> &String {
        match lit_type {
            LiteralTy::Int(int_type) => self.translate_int_type_name(int_type),
            LiteralTy::UInt(uint_type) => self.translate_uint_type_name(uint_type),
            LiteralTy::Float(_float_type) => todo!(),
            LiteralTy::Bool => &LIB_BOOL.1,
            LiteralTy::Char => &LIB_CHAR.1,
        }
    }

    fn translate_int_type_name(&self, int_type: IntTy) -> &String {
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

    fn translate_uint_type_name(&self, uint_type: UIntTy) -> &String {
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

    fn translate_type_decl(
        &mut self,
        type_decl_id: TypeDeclId,
    ) -> Result<(Vec<ptree::TypeDecl>, Vec<Decl>)> {
        let type_decl = &self.crate_.type_decls[type_decl_id];
        let names = &self.name_map.type_decl_names[type_decl_id];

        assert!(type_decl.generics.types.is_empty());
        assert!(type_decl.generics.const_generics.is_empty());
        assert!(type_decl.generics.trait_clauses.is_empty());
        assert!(type_decl.generics.trait_type_constraints.is_empty());
        self.with_generic_params(&names.type_param_names, |self_| {
            let translate_fields = |fields: &IndexVec<_, _>, idents: &IndexVec<_, String>| -> Result<_> {
                let translate_field = |field: &ast::Field, ident: &str| {
                    Ok(ptree::Field {
                        loc: Position::default(),
                        ident: translate_ident(ident.into()),
                        pty: self_.translate_type(&field.ty)?,
                        mutable: true,
                        ghost: false,
                    })
                };
                fields.iter().zip(idents).map(|(field, ident)| translate_field(field, ident)).collect()
            };
            let translate_struct = |fields| {
                let TypeDeclSubIdents::Record(field_idents) = &names.sub_idents else {
                    unreachable!();
                };

                Ok(ptree::TypeDecl {
                    loc: Position::default(),
                    ident: translate_ident(names.item_ident.as_str().into()),
                    params: self_.type_param_stack.innermost().as_raw_slice().into(),
                    vis: Visibility::Public,
                    r#mut: false,
                    inv: Box::new([]),
                    wit: None,
                    def: TypeDef::Record(translate_fields(fields, field_idents)?),
                })
            };
            let translate_enum = |variants: &IndexVec<_, _>| {
                let TypeDeclSubIdents::Variant(variant_idents) = &names.sub_idents else {
                    unreachable!();
                };

                let item_ident = translate_ident(names.item_ident.as_str().into());
                let type_param_idents: Box<[_]> = self_.type_param_stack.innermost().as_raw_slice().into();
                let translate_variant = |variant: &ast::Variant, constructor_idents: &ConstructorIdents| -> Result<_> {
                    let ConstructorIdents { constructor_ident, record_idents_opt } = constructor_idents;
                    let constructor_ident_ = translate_ident(constructor_ident.as_str().into());
                    let translate_tuple_like_variant_field = |variant_field_id, variant_field: &ast::Field| {
                        let constructor_field_type = self_.translate_type(&variant_field.ty)?;
                        Ok((
                            Param(
                                Position::default(),
                                None,
                                false,
                                constructor_field_type.clone(),
                            ),
                            Decl::Let(
                                variant_constructor_field_accessor_ident(constructor_ident, variant_field_id),
                                false,
                                RsKind::None,
                                Box::new(ptree_helpers::expr(Position::default(), ExprDesc::Fun(
                                    ptree_helpers::one_binder(Position::default(), None, Some(Pty::Tyapp(Qualid(Box::new([item_ident.clone()])), Box::new([]))), name_map::local_temp_name(0).into()),
                                    Some(constructor_field_type),
                                    WILDCARD.clone(),
                                    Mask::Visible,
                                    ptree_helpers::empty_spec(),
                                    Box::new(ptree_helpers::expr(Position::default(), ExprDesc::Match(
                                        Box::new(ptree_helpers::evar(Position::default(), Qualid(Box::new([local_temp_ident(0)])))),
                                        Box::new([
                                            (
                                                ptree_helpers::pat(
                                                    Position::default(),
                                                    PatDesc::App(
                                                        Qualid(Box::new([constructor_ident_.clone()])),
                                                        variant.fields.indices().map(|variant_field_id_| {
                                                            if variant_field_id_ == variant_field_id {
                                                                ptree_helpers::pat_var(Position::default(), local_temp_ident(1))
                                                            } else {
                                                                WILDCARD.clone()
                                                            }
                                                        }).collect(),
                                                    ),
                                                ),
                                                ptree_helpers::evar(Position::default(), Qualid(Box::new([local_temp_ident(1)]))),
                                            ),
                                            (WILDCARD.clone(), ABSURD.clone()),
                                        ]),
                                        Box::new([]),
                                    ))),
                                ))),
                            ),
                        ))
                    };
                    let translate_struct_like_variant = |record_ident: &str, record_field_idents| {
                        let record_ident = translate_ident(record_ident.into());
                        let record_type = Pty::Tyapp(Qualid(Box::new([record_ident.clone()])), Box::new([]));
                        Ok((
                            Some(ptree::TypeDecl {
                                loc: Position::default(),
                                ident: record_ident,
                                params: type_param_idents.clone(),
                                vis: Visibility::Public,
                                r#mut: false,
                                inv: Box::new([]),
                                wit: None,
                                def: TypeDef::Record(translate_fields(&variant.fields, record_field_idents)?),
                            }),
                            Box::new([Param(
                                Position::default(),
                                None,
                                false,
                                record_type.clone(),
                            )]) as Box<[_]>,
                            vec![Decl::Let(
                                variant_constructor_accessor_ident(constructor_ident),
                                false,
                                RsKind::None,
                                Box::new(ptree_helpers::expr(Position::default(), ExprDesc::Fun(
                                    ptree_helpers::one_binder(Position::default(), None, Some(Pty::Tyapp(Qualid(Box::new([item_ident.clone()])), Box::new([]))), name_map::local_temp_name(0).into()),
                                    Some(record_type),
                                    WILDCARD.clone(),
                                    Mask::Visible,
                                    ptree_helpers::empty_spec(),
                                    Box::new(ptree_helpers::expr(Position::default(), ExprDesc::Match(
                                        Box::new(ptree_helpers::evar(Position::default(), Qualid(Box::new([local_temp_ident(0)])))),
                                        Box::new([
                                            (
                                                ptree_helpers::pat(
                                                    Position::default(),
                                                    PatDesc::App(
                                                        Qualid(Box::new([constructor_ident_.clone()])),
                                                        Box::new([
                                                            ptree_helpers::pat_var(Position::default(), local_temp_ident(1)),
                                                        ]),
                                                    ),
                                                ),
                                                ptree_helpers::evar(Position::default(), Qualid(Box::new([local_temp_ident(1)]))),
                                            ),
                                            (WILDCARD.clone(), ABSURD.clone()),
                                        ]),
                                        Box::new([]),
                                    ))),
                                ))),
                            )],
                        ))
                    };
                    let (record_opt, constructor_params, accessor_decls): (_, Box<[_]>, _) = match record_idents_opt {
                        None => {
                            let (constructor_params, accessor_decls): (Vec<_>, _) = variant
                                .fields
                                .iter_enumerated()
                                .map(|(variant_field_id, variant_field)| translate_tuple_like_variant_field(variant_field_id, variant_field))
                                .collect::<Result<_>>()?;
                            (None, constructor_params.into(), accessor_decls)
                        }
                        Some((record_ident, record_field_idents)) => translate_struct_like_variant(record_ident, record_field_idents)?,
                    };
                    Ok((
                        record_opt,
                        (
                            Position::default(),
                            constructor_ident_,
                            constructor_params,
                        ),
                        accessor_decls,
                    ))
                };
                let mut type_decls = Vec::new();
                let mut accessor_decls = Vec::new();
                let constructors = variants
                    .iter()
                    .zip(variant_idents)
                    .map(|(variant, constructor_idents)| {
                        let (record_opt, constructor, accessor_decls_) = translate_variant(variant, constructor_idents)?;
                        type_decls.extend(record_opt);
                        accessor_decls.extend(accessor_decls_);
                        Ok(constructor)
                    })
                    .collect::<Result<_>>()?;
                type_decls.push(ptree::TypeDecl {
                    loc: Position::default(),
                    ident: item_ident,
                    params: type_param_idents,
                    vis: Visibility::Public,
                    r#mut: false,
                    inv: Box::new([]),
                    wit: None,
                    def: TypeDef::Algebraic(constructors),
                });
                Ok((type_decls, accessor_decls))
            };
            let translate_opaque_type = || {
                ptree::TypeDecl {
                    loc: Position::default(),
                    ident: translate_ident(names.item_ident.as_str().into()),
                    params: names.type_param_names.iter().map(|ident| translate_ident(ident.as_str().into())).collect(),
                    vis: Visibility::Abstract,
                    r#mut: false,
                    inv: Box::new([]),
                    wit: None,
                    def: TypeDef::Record(Box::new([])),
                }
            };
            match &type_decl.kind {
                TypeDeclKind::Struct(fields) => Ok((vec![translate_struct(fields)?], Vec::new())),
                TypeDeclKind::Enum(variants) => translate_enum(variants),
                TypeDeclKind::Union(..) => Err(Error::Union(type_decl_id)),
                TypeDeclKind::Opaque => Ok((vec![translate_opaque_type()], Vec::new())),
                TypeDeclKind::Alias(..) => Ok((Vec::new(), Vec::new())),
                TypeDeclKind::Error(..) => unreachable!(),
            }
        })
    }

    fn translate_block(&mut self, block: &Block) -> Result<Expr> {
        let mut expr = UNIT_VALUE.clone();
        for stmt in block.statements.iter().rev() {
            expr = self.translate_statement(stmt, expr)?;
        }
        Ok(expr)
    }

    fn translate_statement(&mut self, stmt: &Statement, trailing_expr: Expr) -> Result<Expr> {
        fn sequence_expr(trailing_expr: Expr, expr: Expr) -> Expr {
            ptree_helpers::expr(
                Position::default(),
                ExprDesc::Sequence(Box::new(expr), Box::new(trailing_expr)),
            )
        }

        fn translate_assign(self_: &mut Ctx, trailing_expr: Expr, dst: &Place, expr: Expr) -> Expr {
            let expr = ptree_helpers::expr(
                Position::default(),
                ExprDesc::Assign(Box::new([(self_.translate_place(dst), None, expr)])),
            );
            sequence_expr(trailing_expr, expr)
        }

        fn translate_assign_rvalue(
            self_: &mut Ctx,
            trailing_expr: Expr,
            place: &Place,
            rvalue: &Rvalue,
        ) -> Result<Expr> {
            let rvalue = self_.translate_rvalue(rvalue)?;
            Ok(translate_assign(self_, trailing_expr, place, rvalue))
        }

        fn translate_copy_nonoverlapping(
            CopyNonOverlapping { src, dst, count }: &CopyNonOverlapping,
        ) -> Expr {
            todo!()
        }

        fn translate_place_mention(self_: &mut Ctx, trailing_expr: Expr, place: &Place) -> Expr {
            let expr = self_.translate_place(place);
            sequence_expr(trailing_expr, expr)
        }

        fn translate_abort(_: &AbortKind) -> Expr {
            ABSURD.clone()
        }

        fn translate_assert(
            self_: &mut Ctx,
            trailing_expr: Expr,
            assert: &Assert,
            on_failure: &AbortKind,
        ) -> Result<Expr> {
            let cond = self_.translate_operand(&assert.cond)?;
            let abort = translate_abort(on_failure);
            let expr = if assert.expected {
                ptree_helpers::expr(
                    Position::default(),
                    ExprDesc::If(
                        Box::new(cond),
                        Box::new(UNIT_VALUE.clone()),
                        Box::new(abort),
                    ),
                )
            } else {
                ptree_helpers::expr(
                    Position::default(),
                    ExprDesc::If(
                        Box::new(cond),
                        Box::new(abort),
                        Box::new(UNIT_VALUE.clone()),
                    ),
                )
            };
            Ok(sequence_expr(trailing_expr, expr))
        }

        fn translate_call(
            self_: &mut Ctx,
            trailing_expr: Expr,
            Call {
                func,
                args,
                dest: dst,
            }: &Call,
        ) -> Result<Expr> {
            let func = match func {
                FnOperand::Regular(func_ref) => self_.translate_func_ref(func_ref),
                FnOperand::Dynamic(operand) => self_.translate_operand(operand)?,
            };
            let args: Vec<_> = args
                .iter()
                .map(|arg| self_.translate_operand(arg))
                .collect::<Result<_>>()?;
            let expr = self_.translate_apply(func, args);
            Ok(translate_assign(self_, trailing_expr, dst, expr))
        }

        let translate_return = || {
            ptree_helpers::expr(
                Position::default(),
                ExprDesc::Raise(
                    RETURN_LABEL.clone(),
                    Some(Box::new(ptree_helpers::evar(
                        Position::default(),
                        Qualid(Box::new([self.get_local(LocalId::ZERO).clone()])),
                    ))),
                ),
            )
        };
        let translate_break = |level| {
            ptree_helpers::expr(
                Position::default(),
                ExprDesc::Raise(
                    Qualid(Box::new([break_exn_ident(self.loop_depth - level)])),
                    None,
                ),
            )
        };
        let translate_continue = |level| {
            ptree_helpers::expr(
                Position::default(),
                ExprDesc::Raise(
                    Qualid(Box::new([continue_exn_ident(self.loop_depth - level)])),
                    None,
                ),
            )
        };

        fn translate_switch(self_: &mut Ctx, trailing_expr: Expr, switch: &Switch) -> Result<Expr> {
            fn translate_if(
                self_: &mut Ctx,
                cond: &Operand,
                block_t: &Block,
                block_f: &Block,
            ) -> Result<Expr> {
                Ok(ptree_helpers::expr(
                    Position::default(),
                    ExprDesc::If(
                        Box::new(self_.translate_operand(cond)?),
                        Box::new(self_.translate_block(block_t)?),
                        Box::new(self_.translate_block(block_f)?),
                    ),
                ))
            }

            fn translate_match(
                self_: &mut Ctx,
                scrutinee: &Place,
                arms: &[(Vec<VariantId>, Block)],
                otherwise: &Option<Block>,
            ) -> Result<Expr> {
                let scrutinee_ident = local_temp_ident(0);

                fn translate_discriminants_cond(
                    scrutinee: &Ident,
                    discriminants: &[VariantId],
                ) -> Expr {
                    let discriminant = discriminants.last().unwrap();
                    let expr = ptree_helpers::expr(
                        Position::default(),
                        ExprDesc::Infix(
                            Box::new(ptree_helpers::evar(
                                Position::default(),
                                Qualid(Box::new([scrutinee.clone()])),
                            )),
                            EQUAL.clone(),
                            Box::new(ptree_helpers::econst(
                                Position::default(),
                                discriminant.raw() as isize,
                            )),
                        ),
                    );
                    if discriminants.len() == 1 {
                        expr
                    } else {
                        ptree_helpers::expr(
                            Position::default(),
                            ExprDesc::Or(
                                Box::new(expr),
                                Box::new(translate_discriminants_cond(
                                    scrutinee,
                                    &discriminants[..discriminants.len() - 1],
                                )),
                            ),
                        )
                    }
                }

                let mut expr = match otherwise {
                    Some(block) => self_.translate_block(block)?,
                    None => ABSURD.clone(),
                };
                for (discriminants, block) in arms.iter().rev() {
                    expr = ptree_helpers::expr(
                        Position::default(),
                        ExprDesc::If(
                            Box::new(translate_discriminants_cond(
                                &scrutinee_ident,
                                discriminants,
                            )),
                            Box::new(self_.translate_block(block)?),
                            Box::new(expr),
                        ),
                    );
                }
                Ok(ptree_helpers::expr(
                    Position::default(),
                    ExprDesc::Let(
                        scrutinee_ident,
                        false,
                        RsKind::None,
                        Box::new(self_.translate_place(scrutinee)),
                        Box::new(expr),
                    ),
                ))
            }

            let expr = match switch {
                Switch::If(cond, block_t, block_f) => translate_if(self_, cond, block_t, block_f)?,
                Switch::SwitchInt(..) => unreachable!(),
                Switch::Match(scrutinee, arms, otherwise) => {
                    translate_match(self_, scrutinee, arms, otherwise)?
                }
            };
            Ok(sequence_expr(trailing_expr, expr))
        }

        fn translate_loop(self_: &mut Ctx, trailing_expr: Expr, block: &Block) -> Result<Expr> {
            fn surround_exn(exn_ident: Ident, expr: Expr) -> Expr {
                ptree_helpers::expr(
                    Position::default(),
                    ExprDesc::Exn(
                        exn_ident.clone(),
                        UNIT_TYPE.clone(),
                        Mask::Visible,
                        Box::new(ptree_helpers::expr(
                            Position::default(),
                            ExprDesc::Match(
                                Box::new(expr),
                                Box::new([]),
                                Box::new([(
                                    Qualid(Box::new([exn_ident])),
                                    None,
                                    UNIT_VALUE.clone(),
                                )]),
                            ),
                        )),
                    ),
                )
            }

            let expr = self_.enter_loop(|self_| {
                Ok(surround_exn(
                    break_exn_ident(self_.loop_depth),
                    ptree_helpers::expr(
                        Position::default(),
                        ExprDesc::While(
                            Box::new(TRUE.clone()),
                            Box::new([]),
                            Box::new([]),
                            Box::new(surround_exn(
                                continue_exn_ident(self_.loop_depth),
                                self_.translate_block(block)?,
                            )),
                        ),
                    ),
                ))
            })?;
            Ok(sequence_expr(trailing_expr, expr))
        }

        match &stmt.kind {
            StatementKind::Assign(place, rvalue) => {
                translate_assign_rvalue(self, trailing_expr, place, rvalue)
            }
            StatementKind::SetDiscriminant(..) => unreachable!(),
            StatementKind::CopyNonOverlapping(copy_nonoverlapping) => {
                Ok(translate_copy_nonoverlapping(copy_nonoverlapping))
            }
            StatementKind::StorageLive(..) | StatementKind::StorageDead(..) => Ok(trailing_expr),
            StatementKind::PlaceMention(place) => {
                Ok(translate_place_mention(self, trailing_expr, place))
            }
            StatementKind::Drop(..) => Ok(trailing_expr), // Hack
            StatementKind::Assert { assert, on_failure } => {
                translate_assert(self, trailing_expr, assert, on_failure)
            }
            StatementKind::InlineAsm { .. } => Err(Error::InlineAsm),
            StatementKind::Call(call) => translate_call(self, trailing_expr, call),
            StatementKind::Abort(abort_kind) => Ok(translate_abort(abort_kind)),
            StatementKind::Return => Ok(translate_return()),
            StatementKind::Break(level) => Ok(translate_break(*level)),
            StatementKind::Continue(level) => Ok(translate_continue(*level)),
            StatementKind::Nop => Ok(trailing_expr),
            StatementKind::Switch(switch) => translate_switch(self, trailing_expr, switch),
            StatementKind::Loop(block) => translate_loop(self, trailing_expr, block),
            StatementKind::Error(..) => unreachable!(),
        }
    }

    fn translate_place(&mut self, place: &Place) -> Expr {
        let translate_local = |local_id| {
            ptree_helpers::evar(
                Position::default(),
                Qualid(Box::new([self.get_local(local_id).clone()])),
            )
        };

        fn translate_proj(self_: &mut Ctx, base: &Place, proj: &ProjectionElem) -> Expr {
            let base = self_.translate_place(base);
            let mut translate_field_proj = |base, proj_kind, field_id| {
                let translate_type_decl_field_proj =
                    |base, type_decl_id, variant_id_opt: Option<_>| {
                        let names = &self_.name_map.type_decl_names[type_decl_id];

                        match &names.sub_idents {
                            TypeDeclSubIdents::Record(field_idents) => ptree_helpers::eapp(
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
                                    None => ptree_helpers::eapply(
                                        Position::default(),
                                        ptree_helpers::evar(
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
                                    Some((_record_ident, record_field_idents)) => {
                                        ptree_helpers::eapp(
                                            Position::default(),
                                            ptree_helpers::qualid(Box::new([record_field_idents
                                                [field_id]
                                                .as_str()
                                                .into()])),
                                            Box::new([ptree_helpers::eapply(
                                                Position::default(),
                                                ptree_helpers::evar(
                                                    Position::default(),
                                                    Qualid(Box::new([
                                                        variant_constructor_accessor_ident(
                                                            constructor_ident,
                                                        ),
                                                    ])),
                                                ),
                                                base,
                                            )]),
                                        )
                                    }
                                }
                            }
                            TypeDeclSubIdents::Abstract => unreachable!(),
                        }
                    };
                let translate_tuple_field_proj = |self_: &mut Ctx, base, arity| {
                    self_.record_tuple_field_access(arity, field_id);
                    ptree_helpers::eapply(
                        Position::default(),
                        ptree_helpers::evar(
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
        }

        match &place.kind {
            PlaceKind::Local(local_id) => translate_local(*local_id),
            PlaceKind::Projection(base, proj) => translate_proj(self, base, proj),
            PlaceKind::Global(global_ref) => self.translate_global_ref(global_ref),
        }
    }

    fn translate_global_ref(&self, global_ref: &GlobalDeclRef) -> Expr {
        assert!(global_ref.generics.types.is_empty());
        assert!(global_ref.generics.const_generics.is_empty());
        assert!(global_ref.generics.trait_refs.is_empty());

        let names = &self.name_map.global_names[global_ref.id];

        ptree_helpers::evar(
            Position::default(),
            ptree_helpers::qualid(Box::new([names.item_ident.as_str().into()])),
        )
    }

    fn translate_operand(&mut self, operand: &Operand) -> Result<Expr> {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => Ok(self.translate_place(place)),
            Operand::Const(const_expr) => self.translate_const_expr(const_expr),
        }
    }

    fn translate_const_expr(&self, const_expr: &ConstantExpr) -> Result<Expr> {
        match &const_expr.kind {
            ConstantExprKind::Literal(literal) => Ok(self.translate_literal(literal)),
            ConstantExprKind::Adt(..) => unreachable!(),
            ConstantExprKind::Array(..) => todo!(),
            ConstantExprKind::Global(..) => unreachable!(),
            ConstantExprKind::TraitConst(..) => todo!(),
            ConstantExprKind::VTableRef(..) => todo!(),
            ConstantExprKind::Ref(..) | ConstantExprKind::Ptr(..) => unreachable!(),
            ConstantExprKind::Var(..) => unreachable!(),
            ConstantExprKind::FnDef(func_ref) => Ok(self.translate_func_ref(func_ref)),
            ConstantExprKind::FnPtr(..) => unreachable!(),
            ConstantExprKind::PtrNoProvenance(..) => unreachable!(),
            ConstantExprKind::RawMemory(..) => Err(Error::RawBytesConst),
            ConstantExprKind::Opaque(..) => unreachable!(),
        }
    }

    fn translate_literal(&self, literal: &Literal) -> Expr {
        fn translate_int_literal(self_: &Ctx, int_literal: ScalarValue) -> Expr {
            let (lib_alias, value) = match int_literal {
                ScalarValue::Unsigned(uint_type, value) => {
                    (self_.translate_uint_type_name(uint_type), value.into())
                }
                ScalarValue::Signed(int_type, value) => {
                    (self_.translate_int_type_name(int_type), value.into())
                }
            };
            ptree_helpers::eapply(
                Position::default(),
                ptree_helpers::evar(
                    Position::default(),
                    ptree_helpers::qualid(Box::new([lib_alias.as_str().into(), "of_int".into()])),
                ),
                ptree_helpers::expr(
                    Position::default(),
                    ExprDesc::Const(Constant::Int(IntConstant {
                        kind: IntLiteralKind::Unk,
                        int: value,
                    })),
                ),
            )
        }

        fn translate_float_literal(float_literal: &FloatValue) -> Expr {
            let value: Quad = float_literal.value.parse().unwrap();
            if !value.is_finite() {
                return ABSURD.clone();
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

            ptree_helpers::expr(
                Position::default(),
                ExprDesc::Const(Constant::Real(RealConstant {
                    kind: RealLiteralKind::Unk,
                    real: RealValue {
                        sig: significand.into(),
                        pow2: exp.into(),
                        pow5: Integer::ZERO,
                    },
                })),
            )
        }

        fn translate_bool_literal(bool_literal: bool) -> Expr {
            if bool_literal {
                TRUE.clone()
            } else {
                FALSE.clone()
            }
        }

        fn translate_char_literal(char_literal: char) -> Expr {
            ptree_helpers::expr(
                Position::default(),
                ExprDesc::Const(Constant::Int(IntConstant {
                    kind: IntLiteralKind::Unk,
                    int: u32::from(char_literal).into(),
                })),
            )
        }

        fn translate_str_literal(str_literal: &str) -> Expr {
            ptree_helpers::expr(
                Position::default(),
                ExprDesc::Const(Constant::Str(str_literal.into())),
            )
        }

        match literal {
            Literal::Scalar(int_literal) => translate_int_literal(self, *int_literal),
            Literal::Float(float_literal) => translate_float_literal(float_literal),
            Literal::Bool(bool_literal) => translate_bool_literal(*bool_literal),
            Literal::Char(char_literal) => translate_char_literal(*char_literal),
            Literal::ByteStr(..) => todo!(),
            Literal::Str(str_literal) => translate_str_literal(str_literal),
        }
    }

    fn translate_func_ref(&self, func_ref: &FnPtr) -> Expr {
        fn translate_builtin_func_ref(builtin_func_id: BuiltinFunId) -> Expr {
            let builtin_func_ident = match builtin_func_id {
                BuiltinFunId::BoxNew => "box_new",
                BuiltinFunId::SpecEntry
                | BuiltinFunId::SpecPrecondition
                | BuiltinFunId::SpecPostcondition => unreachable!(),
                BuiltinFunId::SpecForall
                | BuiltinFunId::SpecExists
                | BuiltinFunId::SpecImplies
                | BuiltinFunId::SpecOld => todo!(),
                BuiltinFunId::SpecAssert | BuiltinFunId::SpecAssume => todo!(),
                BuiltinFunId::ArrayToSliceShared => todo!(),
                BuiltinFunId::ArrayToSliceMut => todo!(),
                BuiltinFunId::ArrayRepeat => todo!(),
                BuiltinFunId::Index(..) => todo!(),
                BuiltinFunId::PtrFromParts(RefKind::Shared) => "ptr_from_parts_shared",
                BuiltinFunId::PtrFromParts(RefKind::Mut) => "ptr_from_parts_mut",
            };
            ptree_helpers::evar(
                Position::default(),
                ptree_helpers::qualid(Box::new([
                    LIB_BUILTIN.1.as_str().into(),
                    builtin_func_ident.into(),
                ])),
            )
        }

        match &*func_ref.kind {
            FnPtrKind::Fun(FunId::Regular(func_decl_id)) => {
                self.translate_func_decl_ref(*func_decl_id, &func_ref.generics)
            }
            FnPtrKind::Fun(FunId::Builtin(builtin_func_id)) => {
                translate_builtin_func_ref(*builtin_func_id)
            }
            FnPtrKind::Trait(_, _, func_decl_id) => {
                self.translate_func_decl_ref(*func_decl_id, &func_ref.generics)
            }
        }
    }

    fn translate_func_decl_ref(&self, func_decl_id: FunDeclId, generic_args: &BoxedArgs) -> Expr {
        assert!(generic_args.types.is_empty());
        assert!(generic_args.const_generics.is_empty());
        assert!(generic_args.trait_refs.is_empty());

        let names = &self.name_map.func_decl_names[func_decl_id];

        ptree_helpers::evar(
            Position::default(),
            ptree_helpers::qualid(Box::new([names.item_ident.as_str().into()])),
        )
    }

    fn translate_rvalue(&mut self, rvalue: &Rvalue) -> Result<Expr> {
        fn translate_ref(
            self_: &mut Ctx,
            place: &Place,
            kind: BorrowKind,
            ptr_metadata: &Operand,
        ) -> Expr {
            let place = self_.translate_place(place);
            match kind {
                BorrowKind::Shared | BorrowKind::UniqueImmutable => place,
                BorrowKind::Mut => todo!(),
                BorrowKind::TwoPhaseMut => todo!(),
                BorrowKind::Shallow => todo!(),
            }
        }

        fn translate_binary_op(
            self_: &mut Ctx,
            operator: BinOp,
            lhs: &Operand,
            rhs: &Operand,
        ) -> Result<Expr> {
            let lhs_type = lhs.ty();
            let lhs = self_.translate_operand(lhs)?;
            let rhs_type = rhs.ty();
            let rhs = self_.translate_operand(rhs)?;
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
                self_.translate_apply(
                    ptree_helpers::evar(
                        Position::default(),
                        ptree_helpers::qualid(Box::new([lhs_type_name.into(), func_ident.into()])),
                    ),
                    [lhs, rhs],
                )
            };
            let infix = |lhs, rhs, operator_ident| {
                ptree_helpers::expr(
                    Position::default(),
                    ExprDesc::Scope(
                        ptree_helpers::qualid(Box::new([lhs_type_name.into()])),
                        Box::new(ptree_helpers::expr(
                            Position::default(),
                            ExprDesc::Infix(Box::new(lhs), operator_ident, Box::new(rhs)),
                        )),
                    ),
                )
            };
            let apply_rhs_int = |lhs, rhs, func_ident: &str| {
                self_.translate_apply(
                    ptree_helpers::evar(
                        Position::default(),
                        ptree_helpers::qualid(Box::new([lhs_type_name.into(), func_ident.into()])),
                    ),
                    [
                        lhs,
                        ptree_helpers::eapply(
                            Position::default(),
                            ptree_helpers::evar(
                                Position::default(),
                                ptree_helpers::qualid(Box::new([
                                    self_.translate_literal_type_name(rhs_type).as_str().into(),
                                    "to_int".into(),
                                ])),
                            ),
                            rhs,
                        ),
                    ],
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
        }

        fn translate_unary_op(self_: &mut Ctx, operator: &UnOp, operand: &Operand) -> Result<Expr> {
            let operand_type = operand.ty();
            let operand = self_.translate_operand(operand)?;
            let operand_type = match operand_type.kind() {
                TyKind::Literal(lit_type) => *lit_type,
                _ => todo!(),
            };
            let apply = |func_ident: &str| {
                ptree_helpers::eapply(
                    Position::default(),
                    ptree_helpers::evar(
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
        }

        fn translate_nullary_op(operator: &NullOp, ty: &Ty) -> Result<Expr> {
            match operator {
                NullOp::SizeOf => todo!(),
                NullOp::AlignOf => todo!(),
                NullOp::OffsetOf(type_ref, variant_id_opt, field_id) => todo!(),
                NullOp::UbChecks => todo!(),
                NullOp::OverflowChecks => todo!(),
                NullOp::ContractChecks => todo!(),
            }
        }

        fn translate_aggregate(
            self_: &mut Ctx,
            kind: &AggregateKind,
            operands: &Vec<Operand>,
        ) -> Result<Expr> {
            let translate_type_decl_aggregate =
                |self_: &mut Ctx, type_decl_id, variant_id_opt: Option<_>, _field_id_opt| {
                    let names = &self_.name_map.type_decl_names[type_decl_id];

                    let translate_struct_aggregate =
                        |self_: &mut Ctx, field_idents: &IndexVec<_, _>| {
                            Ok(ptree_helpers::expr(
                                Position::default(),
                                ExprDesc::Record(
                                    field_idents
                                        .iter()
                                        .zip(operands)
                                        .map(|(field_ident, operand): (&String, _)| {
                                            Ok((
                                                ptree_helpers::qualid(Box::new([field_ident
                                                    .as_str()
                                                    .into()])),
                                                self_.translate_operand(operand)?,
                                            ))
                                        })
                                        .collect::<Result<_>>()?,
                                ),
                            ))
                        };
                    let translate_enum_aggregate =
                        |self_: &mut Ctx,
                         ConstructorIdents {
                             constructor_ident,
                             record_idents_opt,
                         }: &_| {
                            let constructor_expr = ptree_helpers::evar(
                                Position::default(),
                                ptree_helpers::qualid(Box::new([constructor_ident
                                    .as_str()
                                    .into()])),
                            );
                            Ok(match record_idents_opt {
                                None => {
                                    let operands: Vec<_> = operands
                                        .iter()
                                        .map(|operand| self_.translate_operand(operand))
                                        .collect::<Result<_>>()?;
                                    self_.translate_apply(constructor_expr, operands)
                                }
                                Some((_record_ident, record_field_idents)) => {
                                    ptree_helpers::eapply(
                                        Position::default(),
                                        constructor_expr,
                                        translate_struct_aggregate(self_, record_field_idents)?,
                                    )
                                }
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
            let translate_tuple_aggregate = |self_: &mut Ctx| {
                Ok(ptree_helpers::expr(
                    Position::default(),
                    ExprDesc::Tuple(
                        operands
                            .iter()
                            .map(|operand| self_.translate_operand(operand))
                            .collect::<Result<_>>()?,
                    ),
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
        }

        match rvalue {
            Rvalue::Use(operand) => self.translate_operand(operand),
            Rvalue::Ref {
                place,
                kind,
                ptr_metadata,
            } => Ok(translate_ref(self, place, *kind, ptr_metadata)),
            Rvalue::RawPtr {
                place,
                kind,
                ptr_metadata,
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

    fn translate_apply(&self, func: Expr, args: impl IntoIterator<Item = Expr>) -> Expr {
        args.into_iter().fold(func, |expr, arg| {
            ptree_helpers::eapply(Position::default(), expr, arg)
        })
    }

    fn translate_func_decl(&mut self, func_decl_id: FunDeclId) -> Result<Option<FuncData>> {
        let func_decl = &self.crate_.fun_decls[func_decl_id];

        if matches!(func_decl.src, ItemSource::TraitDecl { .. }) {
            return Ok(None);
        }
        // Hack
        if func_decl.signature.is_unsafe {
            return Ok(None);
        }

        let names = &self.name_map.func_decl_names[func_decl_id];

        assert!(func_decl.generics.types.is_empty());
        assert!(func_decl.generics.const_generics.is_empty());
        assert!(func_decl.generics.trait_clauses.is_empty());
        assert!(func_decl.generics.trait_type_constraints.is_empty());
        self.with_generic_params(&names.type_param_names, |self_| {
            let func_sig = &func_decl.signature;
            let translate_structured_body = |self_: &mut Self, body: &ExprBody| {
                let LocalNames::Concrete(local_names) = &names.local_names else {
                    unreachable!();
                };

                self_.set_locals(local_names);
                let body_expr = self_.translate_block(&body.body)?;
                let local_idents = self_.locals.as_ref().unwrap();
                let params = {
                    if func_sig.inputs.is_empty() {
                        ptree_helpers::unit_binder(Position::default())
                    } else {
                        local_idents
                            .iter()
                            .skip(1)
                            .zip(&func_sig.inputs)
                            .map(|(param_ident, param_type)| {
                                Ok(Binder(
                                    Position::default(),
                                    Some(param_ident.clone()),
                                    false,
                                    Some(self_.translate_type(param_type)?),
                                ))
                            })
                            .collect::<Result<_>>()?
                    }
                };
                let body_expr = local_idents
                    .indices()
                    .rev()
                    .filter(|local_id| {
                        *local_id == LocalId::ZERO || *local_id > body.locals.arg_count
                    })
                    .try_fold(body_expr, |body_expr, local_id| {
                        Ok(ptree_helpers::expr(
                            Position::default(),
                            ExprDesc::Let(
                                Ident {
                                    ats: Box::new([Attr::Str(REF_ATTR.clone())]),
                                    ..local_idents[local_id].clone()
                                },
                                false,
                                RsKind::None,
                                Box::new(ptree_helpers::expr(
                                    Position::default(),
                                    ExprDesc::Any(
                                        Box::new([]),
                                        RsKind::None,
                                        Some(Pty::Ref(Box::new([
                                            self_.translate_type(&body.locals[local_id].ty)?
                                        ]))),
                                        WILDCARD.clone(),
                                        Mask::Visible,
                                        ptree_helpers::empty_spec(),
                                    ),
                                )),
                                Box::new(body_expr),
                            ),
                        ))
                    })?;
                Ok((params, Some(body_expr)))
            };
            let translate_abstract_body = || {
                let LocalNames::Abstract(param_names_opt) = &names.local_names else {
                    unreachable!();
                };

                let params = {
                    if func_sig.inputs.is_empty() {
                        ptree_helpers::unit_binder(Position::default())
                    } else {
                        param_names_opt
                            .iter()
                            .zip(&func_sig.inputs)
                            .map(|(param_name_opt, param_type)| {
                                Ok(Binder(
                                    Position::default(),
                                    param_name_opt.as_ref().map(|param_name| {
                                        translate_ident(param_name.as_str().into())
                                    }),
                                    false,
                                    Some(self_.translate_type(param_type)?),
                                ))
                            })
                            .collect::<Result<_>>()?
                    }
                };
                Ok((params, None))
            };
            let (params, body) = match &func_decl.body {
                Body::Unstructured(..) => unreachable!(),
                Body::Structured(body) => translate_structured_body(self_, body)?,
                Body::TargetDispatch(..) => todo!(),
                Body::TraitMethodWithoutDefault => unreachable!(),
                Body::Extern(..) | Body::Intrinsic { .. } | Body::Opaque | Body::Missing => {
                    translate_abstract_body()?
                }
                Body::Error(..) => unreachable!(),
            };
            Ok(Some(FuncData(
                translate_ident(names.item_ident.as_str().into()),
                false,
                RsKind::None,
                params,
                self_.translate_type(&func_sig.output)?,
                WILDCARD.clone(),
                Mask::Visible,
                ptree_helpers::empty_spec(),
                body,
            )))
        })
    }

    fn translate_global(&mut self, global_id: GlobalDeclId) -> Decl {
        let global = &self.crate_.global_decls[global_id];
        let names = &self.name_map.global_names[global_id];
        let init_func_decl_names = &self.name_map.func_decl_names[global.init];

        assert!(global.generics.types.is_empty());
        assert!(global.generics.const_generics.is_empty());
        assert!(global.generics.trait_clauses.is_empty());
        assert!(global.generics.trait_type_constraints.is_empty());
        self.with_generic_params(&names.type_param_names, |_| {
            Decl::Let(
                translate_ident(names.item_ident.as_str().into()),
                false,
                RsKind::None,
                Box::new(ptree_helpers::eapply(
                    Position::default(),
                    ptree_helpers::evar(
                        Position::default(),
                        ptree_helpers::qualid(Box::new([init_func_decl_names
                            .item_ident
                            .as_str()
                            .into()])),
                    ),
                    UNIT_VALUE.clone(),
                )),
            )
        })
    }

    fn translate_decl_group(&mut self, decl_group: &DeclarationGroup) -> Result<()> {
        let translate_type_decl_group = |self_: &mut Self, type_decl_ids: &[_]| {
            let mut type_decls = Vec::new();
            let mut accessor_decls = Vec::new();
            for &type_decl_id in type_decl_ids {
                let (type_decls_, accessor_decls_) = self_.translate_type_decl(type_decl_id)?;
                type_decls.extend(type_decls_);
                accessor_decls.extend(accessor_decls_);
            }
            self_.push_decl(Decl::Type(type_decls.into()));
            self_.extend_decls(accessor_decls);
            Ok(())
        };
        let translate_func_decl_group = |self_: &mut Self, func_decl_group: &_| {
            match func_decl_group {
                GDeclarationGroup::NonRec(func_decl_id) => {
                    let Some(FuncData(ident, ghost, kind, params, ret_type, pat, mask, spec, body)) =
                        self_.translate_func_decl(*func_decl_id)?
                    else {
                        return Ok(());
                    };
                    self_.push_decl(Decl::Let(
                        ident,
                        ghost,
                        kind,
                        Box::new(ptree_helpers::expr(
                            Position::default(),
                            if let Some(body) = body {
                                ExprDesc::Fun(
                                    params,
                                    Some(ret_type),
                                    pat,
                                    mask,
                                    spec,
                                    Box::new(body),
                                )
                            } else {
                                ExprDesc::Any(
                                    params
                                        .into_iter()
                                        .map(|param| {
                                            ptree::Param(
                                                param.0,
                                                param.1,
                                                param.2,
                                                param.3.unwrap(),
                                            )
                                        })
                                        .collect(),
                                    RsKind::None,
                                    Some(ret_type),
                                    pat,
                                    mask,
                                    spec,
                                )
                            },
                        )),
                    ));
                }
                GDeclarationGroup::Rec(func_decl_ids) => {
                    let mut func_decls = Vec::new();
                    for &func_decl_id in func_decl_ids {
                        let Some(FuncData(
                            ident,
                            ghost,
                            kind,
                            params,
                            ret_type,
                            pat,
                            mask,
                            spec,
                            body,
                        )) = self_.translate_func_decl(func_decl_id)?
                        else {
                            continue;
                        };
                        func_decls.push(Fundef(
                            ident,
                            ghost,
                            kind,
                            params,
                            Some(ret_type),
                            pat,
                            mask,
                            spec,
                            body.unwrap(),
                        ));
                    }
                    self_.push_decl(Decl::Rec(func_decls.into()));
                }
            }
            Ok(())
        };
        let translate_global_group = |self_: &mut Self, global_ids: &[_]| {
            let [global_id] = global_ids else {
                unreachable!();
            };

            let decl = self_.translate_global(*global_id);
            self_.push_decl(decl);
        };
        match decl_group {
            DeclarationGroup::Type(type_decl_group) => {
                translate_type_decl_group(self, type_decl_group.get_ids())
            }
            DeclarationGroup::Fun(func_decl_group) => {
                translate_func_decl_group(self, func_decl_group)
            }
            DeclarationGroup::Global(global_group) => {
                translate_global_group(self, global_group.get_ids());
                Ok(())
            }
            DeclarationGroup::TraitDecl(..) => Ok(()),
            DeclarationGroup::TraitImpl(..) => Ok(()),
            DeclarationGroup::Mixed(decl_group) => {
                Err(Error::MixedDeclGroup(decl_group.get_ids().into()))
            }
        }
    }
}

struct FuncData(
    Ident,
    Ghost,
    RsKind,
    Box<[Binder]>,
    Pty,
    Pattern,
    Mask,
    Spec,
    Option<Expr>,
);

static IMPORTS: LazyLock<Decl> = LazyLock::new(|| {
    Decl::Useimport(
        Position::default(),
        false,
        [
            &*LIB_BOOL,
            &*LIB_BUILTIN,
            &*LIB_CHAR,
            &*LIB_I8,
            &*LIB_I16,
            &*LIB_I32,
            &*LIB_I64,
            &*LIB_I128,
            &*LIB_TUPLE,
            &*LIB_U8,
            &*LIB_U16,
            &*LIB_U32,
            &*LIB_U64,
            &*LIB_U128,
        ]
        .into_iter()
        .map(|(path, alias)| {
            (
                ptree_helpers::qualid(
                    iter::once(LIB_DIR)
                        .chain((*path).iter().copied())
                        .map(|ident| ident.into())
                        .collect(),
                ),
                Some(translate_ident(alias.as_str().into())),
            )
        })
        .collect(),
    )
});

static IMPORT_REF: LazyLock<Decl> = LazyLock::new(|| {
    Decl::Useimport(
        Position::default(),
        false,
        Box::new([(
            ptree_helpers::qualid(Box::new(["ref".into(), "Ref".into()])),
            None,
        )]),
    )
});

static LIB_TUPLE_IDENT: LazyLock<Ident> =
    LazyLock::new(|| translate_ident(LIB_TUPLE.1.as_str().into()));

fn local_temp_ident(i: usize) -> Ident {
    translate_ident(name_map::local_temp_name(i).into())
}

fn tuple_field_accessor_ident(arity: usize, field_id: FieldId) -> Ident {
    translate_ident(name_map::tuple_field_accessor_ident(arity, field_id).into())
}

fn variant_constructor_field_accessor_ident(constructor_ident: &str, field_id: FieldId) -> Ident {
    translate_ident(
        name_map::variant_constructor_field_accessor_ident(constructor_ident, field_id).into(),
    )
}

fn variant_constructor_accessor_ident(constructor_ident: &str) -> Ident {
    translate_ident(name_map::variant_constructor_accessor_ident(constructor_ident).into())
}

fn break_exn_ident(loop_depth: usize) -> Ident {
    translate_ident(name_map::break_exn_name(loop_depth).into())
}

fn continue_exn_ident(loop_depth: usize) -> Ident {
    translate_ident(name_map::continue_exn_name(loop_depth).into())
}

static STRING: LazyLock<Pty> = LazyLock::new(|| {
    Pty::Tyapp(
        ptree_helpers::qualid(Box::new(["string".into()])),
        Box::new([]),
    )
});

static EMPTY: LazyLock<Pty> = LazyLock::new(|| {
    Pty::Tyapp(
        ptree_helpers::qualid(Box::new([EMPTY_TYPE_NAME.into()])),
        Box::new([]),
    )
});

static UNIT_TYPE: LazyLock<Pty> = LazyLock::new(|| Pty::Tuple(Box::new([])));

// static ARRAY: LazyLock<Qualid> =
//     LazyLock::new(|| Qualid(Box::new([LIB_IDENT.clone(), translate_ident("array".into())])));

static WILDCARD: LazyLock<Pattern> =
    LazyLock::new(|| ptree_helpers::pat(Position::default(), PatDesc::Wild));

static ABSURD: LazyLock<Expr> =
    LazyLock::new(|| ptree_helpers::expr(Position::default(), ExprDesc::Absurd));

static UNIT_VALUE: LazyLock<Expr> =
    LazyLock::new(|| ptree_helpers::expr(Position::default(), ExprDesc::Tuple(Box::new([]))));

static TRUE: LazyLock<Expr> =
    LazyLock::new(|| ptree_helpers::expr(Position::default(), ExprDesc::True));

static FALSE: LazyLock<Expr> =
    LazyLock::new(|| ptree_helpers::expr(Position::default(), ExprDesc::False));

static EQUAL: LazyLock<Ident> = LazyLock::new(|| translate_ident(OP_EQU.clone()));

static LESS: LazyLock<Ident> = LazyLock::new(|| translate_ident(ident::op_infix("<")));

static LESS_EQUAL: LazyLock<Ident> = LazyLock::new(|| translate_ident(ident::op_infix("<=")));

static GREATER: LazyLock<Ident> = LazyLock::new(|| translate_ident(ident::op_infix(">")));

static GREATER_EQUAL: LazyLock<Ident> = LazyLock::new(|| translate_ident(ident::op_infix(">=")));

static NOT_EQUAL: LazyLock<Ident> = LazyLock::new(|| translate_ident(ident::op_infix("<>")));

static RETURN_LABEL: LazyLock<Qualid> =
    LazyLock::new(|| ptree_helpers::qualid(Box::new([ptree_helpers::RETURN_ID.into()])));

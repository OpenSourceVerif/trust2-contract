//! 类型声明（结构体、枚举、不透明类型等）的翻译。

use super::Ctx;

use itertools::Itertools;

use crate::llbc_to_whyml::{
    error::{Error, Result},
    ident::{
        local_temp_ident, translate_ident, variant_constructor_accessor_ident,
        variant_constructor_field_accessor_ident,
    },
    name_map::{self, ConstructorIdents, TypeDeclSubIdents},
    prelude::{ABSURD_EXPR, WILDCARD},
};

use charon_lib::{
    ast::{self, TypeDeclId, TypeDeclKind},
    formatter::FmtCtx,
    ids::IndexVec,
    pretty::FmtWithCtx,
};

use why3_ptree::{
    expr::RsKind,
    ity::Mask,
    loc::Position,
    ptree::{self, Decl, ExprDesc, Param, PatDesc, Pty, Qualid, TypeDef, Visibility},
    ptree_helpers,
};

impl<'a> Ctx<'a> {
    /// 翻译一个类型声明。
    ///
    /// 结构体直接翻译成 WhyML 的记录类型；枚举翻译成代数类型，同时
    /// 为每个构造子生成「取出各字段」的访问函数（WhyML 没有记录投影语法）。
    /// 返回 `(类型声明, 访问函数声明)`，其中枚举会额外生成记录类型的辅助声明。
    pub(super) fn translate_type_decl(
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
                    let translate_tuple_like_variant_field = |variant_field_id: ast::FieldId, variant_field: &ast::Field| {
                        let constructor_field_type = self_.translate_type(&variant_field.ty)?;
                        Ok((
                            Param(
                                Position::default(),
                                // 注意：构造子的参数只能是无名的（见 `translate_enum` 的说明）。
                                None,
                                false,
                                constructor_field_type.clone(),
                            ),
                            Decl::Let(
                                variant_constructor_field_accessor_ident(constructor_ident, variant_field_id),
                                false,
                                RsKind::Func,
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
                                            (WILDCARD.clone(), ABSURD_EXPR.clone()),
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
                                // 同上：构造子的参数只能是无名的。
                                None,
                                false,
                                record_type.clone(),
                            )]) as Box<[_]>,
                            vec![Decl::Let(
                                variant_constructor_accessor_ident(constructor_ident),
                                false,
                                RsKind::Func,
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
                                            (WILDCARD.clone(), ABSURD_EXPR.clone()),
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
                                .try_collect()?;
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
                    .try_collect()?;
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
                TypeDeclKind::Enum(variants) => {
                    // 目前只支持「无字段变体」（C 风格枚举）。带字段的变体
                    // （`Ctor(T, ...)` 与 `Ctor { .. }`）现在生不出合法的 WhyML：
                    // 构造子的字段在语法树里只能表示成无名参数（`Param(_, None, ..)`），
                    // 而 why3-ptree 0.3.0 的打印机对无名参数会整段跳过，输出 `| Ctor `，
                    // Why3 于是报 `Function Ctor expects no arguments`；若改成有名参数，
                    // WhyML 又把它当作类型的字段，要求所有构造子都声明同名字段
                    // （`Field x is missing in some constructors`），对枚举不可能满足。
                    // 所以在翻译阶段就报错，而不是生成过不了类型检查的 WhyML。
                    if let Some(variant) =
                        variants.iter().find(|variant| !variant.fields.is_empty())
                    {
                        return Err(Error::EnumVariantWithFields(format!(
                            "{}::{}",
                            type_decl.item_meta.name.with_ctx(&FmtCtx::new()),
                            variant.name
                        )));
                    }
                    translate_enum(variants)
                }
                TypeDeclKind::Union(..) => Err(Error::Union(type_decl_id)),
                TypeDeclKind::Opaque => Ok((vec![translate_opaque_type()], Vec::new())),
                TypeDeclKind::Alias(..) => Ok((Vec::new(), Vec::new())),
                TypeDeclKind::Error(..) => unreachable!(),
            }
        })
    }
}

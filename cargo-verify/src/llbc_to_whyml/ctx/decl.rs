//! 函数、全局量与声明组的翻译。

use super::Ctx;

use crate::llbc_to_whyml::{
    error::{Error, Result},
    ident::translate_ident,
    name_map::{FuncSpecNames, LocalNames},
    prelude::WILDCARD,
};

use charon_lib::{
    ast::{
        Body, DeclarationGroup, FunDeclId, FunSpecs, GDeclarationGroup, GlobalDeclId, ItemSource,
        LocalId, SpecClosure,
    },
    llbc_ast::ExprBody,
};

use itertools::Itertools;

use why3_ptree::{
    expr::RsKind,
    ity::Mask,
    loc::Position,
    pmodule::REF_ATTR,
    ptree::{
        self, Attr, Binder, Decl, Expr, ExprDesc, Fundef, Ghost, Ident, Pattern, Post, Pre, Pty,
        Spec,
    },
    ptree_helpers,
};

impl<'a> Ctx<'a> {
    /// 翻译一个函数声明。
    ///
    /// trait 声明中的方法与 `unsafe` 函数（见代码中的 hack）会被跳过；
    /// 有函数体的函数翻译成 `let`，函数体以 `ref` 局部变量模拟可变局部变量；
    /// 只有签名（或不可见函数体）的函数则只翻译成抽象声明 `val`。
    /// 返回值中的 `Option<Expr>` 为 `None` 时表示该函数没有可见的函数体。
    fn translate_func_decl(&mut self, func_decl_id: FunDeclId) -> Result<Option<FuncData>> {
        let func_decl = &self.crate_.fun_decls[func_decl_id];

        if matches!(func_decl.src, ItemSource::TraitDecl { .. }) {
            return Ok(None);
        }
        // Hack
        if func_decl.signature.is_unsafe {
            return Ok(None);
        }
        if func_decl.signature.is_variadic {
            return Err(Error::VariadicFunc);
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

                self_.with_locals(local_names, |self_| {
                    fn translate_precondition(
                        self_: &mut Ctx,
                        precondition: &SpecClosure,
                        local_names: &LocalNames,
                    ) -> Result<Pre> {
                        self_.translate_spec_closure(precondition, local_names)
                    }

                    fn translate_postcondition(
                        self_: &mut Ctx,
                        postcondition: &SpecClosure,
                        local_names: &LocalNames,
                    ) -> Result<Post> {
                        let LocalNames::Concrete(local_names_) = local_names else {
                            unreachable!();
                        };
                        let arg_id = postcondition
                            .non_captured_argument_ids()
                            .exactly_one()
                            .unwrap();
                        Ok(Post(
                            Position::default(),
                            Box::new([(
                                ptree_helpers::pat_var(
                                    Position::default(),
                                    translate_ident(local_names_[arg_id].as_str().into()),
                                ),
                                self_.translate_spec_closure(postcondition, local_names)?,
                            )]),
                        ))
                    }

                    let FunSpecs {
                        preconditions,
                        postconditions,
                    } = &func_decl.specs;
                    let FuncSpecNames {
                        precondition_local_names,
                        postcondition_local_names,
                    } = &names.spec_names;
                    let specs = Spec {
                        pre: preconditions
                            .iter()
                            .zip(precondition_local_names)
                            .map(|(precondition, local_names)| {
                                translate_precondition(self_, precondition, local_names)
                            })
                            .try_collect()?,
                        post: postconditions
                            .iter()
                            .zip(postcondition_local_names)
                            .map(|(postcondition, local_names)| {
                                translate_postcondition(self_, postcondition, local_names)
                            })
                            .try_collect()?,
                        xpost: Box::new([]),
                        reads: Box::new([]),
                        writes: Box::new([]),
                        alias: Box::new([]),
                        variant: Box::new([]),
                        checkrw: false,
                        diverge: false,
                        partial: false,
                    };
                    let body_expr: Expr = self_.translate_block_to_expr(&body.body)?;
                    let local_idents = self_.locals_stack.last().unwrap();
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
                                .try_collect()?
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
                    Ok((params, specs, Some(body_expr)))
                })
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
                            .try_collect()?
                    }
                };
                Ok((params, ptree_helpers::empty_spec(), None))
            };
            let (params, specs, body) = match &func_decl.body {
                Body::Unstructured(..) => unreachable!(),
                Body::Structured(body) => translate_structured_body(self_, body)?,
                Body::TargetDispatch(..) => todo!(),
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
                specs,
                body,
            )))
        })
    }

    /// 翻译一个全局常量。
    fn translate_global(&mut self, global_id: GlobalDeclId) -> Result<Decl> {
        let global = &self.crate_.global_decls[global_id];
        let names = &self.name_map.global_names[global_id];

        assert!(global.generics.types.is_empty());
        assert!(global.generics.const_generics.is_empty());
        assert!(global.generics.trait_clauses.is_empty());
        assert!(global.generics.trait_type_constraints.is_empty());
        self.with_generic_params(&names.type_param_names, |self_| {
            Ok(Decl::Let(
                translate_ident(names.item_ident.as_str().into()),
                false,
                RsKind::None,
                Box::new(self_.translate_const_expr_to_expr(&global.value)?),
            ))
        })
    }

    /// 翻译一组声明，按声明组的类别分派（类型、函数、全局量）。
    pub(super) fn translate_decl_group(&mut self, decl_group: &DeclarationGroup) -> Result<()> {
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

            let decl = self_.translate_global(*global_id)?;
            self_.push_decl(decl);
            Ok(())
        };
        match decl_group {
            DeclarationGroup::Type(type_decl_group) => {
                translate_type_decl_group(self, type_decl_group.get_ids())
            }
            DeclarationGroup::Fun(func_decl_group) => {
                translate_func_decl_group(self, func_decl_group)
            }
            DeclarationGroup::Global(global_group) => {
                translate_global_group(self, global_group.get_ids())
            }
            DeclarationGroup::TraitDecl(..) => Ok(()),
            DeclarationGroup::TraitImpl(..) => Ok(()),
            DeclarationGroup::Mixed(decl_group) => {
                Err(Error::MixedDeclGroup(decl_group.get_ids().into()))
            }
        }
    }
}

/// 翻译函数声明得到的中间结果，用于构造 WhyML 的 `let` 定义。
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

//! 名字映射的构造：遍历 crate，为所有条目生成互不冲突的 WhyML 名字。
//!
//! 做法是先把所有条目的「路径」（由名字的各段组成）收集到一张反向表中，
//! 路径冲突时加后缀区分（见 [`super::locals::resolve_collision`]），
//! 最后再按条目把名字写回 [`super::NameMap`]。

use charon_lib::{
    ast::{
        FieldId, FunSpecs, GenericParams, ItemId, Name, PathElem, TranslatedCrate, TypeDeclKind,
        VariantId,
    },
    formatter::FmtCtx,
    ids::IndexVec,
    pretty::FmtWithCtx,
};

use std::{collections::HashMap, iter, mem};

use super::{
    ConstructorIdents, FuncDeclNames, FuncSpecNames, GlobalNames, NameMap, TypeDeclNames,
    TypeDeclSubIdents, TypeParamNames,
    keywords::{
        CONSTRUCTOR_PREFIX, TAKEN_WORDS_SET, TYPE_PARAM_PREFIX, camel_to_l, camel_to_q, ensure_l,
        ensure_u, to_whyml, upper_to_l,
    },
    locals::{LocalMapper, resolve_collision},
    names::SEPARATOR,
};

/// 为一个 crate 建立名字映射。
///
/// 先收集所有条目（类型及其成员、函数、全局量）的名字路径，
/// 再消解同名冲突，最后为每个条目生成最终名字。
pub fn build(crate_: &TranslatedCrate) -> NameMap {
    type Id = (ItemId, Option<(usize, Option<Option<usize>>)>);

    let mut inv_map: HashMap<_, Vec<Id>> = HashMap::new();

    enum TypeDeclSubIdentsOpt {
        Record(IndexVec<FieldId, Option<String>>),
        Variant(IndexVec<VariantId, ConstructorIdentsOpt>),
        Abstract,
    }

    struct ConstructorIdentsOpt {
        ident: Option<String>,
        #[allow(clippy::type_complexity)]
        record_idents_opt: Option<(Option<String>, IndexVec<FieldId, Option<String>>)>,
    }

    let mut type_decl_names_opt = crate_
        .type_decls
        .map_ref_indexed(|type_decl_id, type_decl| {
            let item_id = ItemId::from(type_decl_id);
            let path = &type_decl.item_meta.name;
            let path = process_path(path, |ident| camel_to_l(&ident));

            let sub_idents_opt = match &type_decl.kind {
                TypeDeclKind::Struct(fields) => {
                    TypeDeclSubIdentsOpt::Record(fields.map_ref_indexed(|field_id, field| {
                        let field_ident = match &field.name {
                            Some(ident) => ident.into(),
                            None => format!("{field_id}").into(),
                        };
                        let field_ident = to_whyml(&ensure_l(field_ident));
                        let mut field_path = path.clone();
                        field_path.insert(1, field_ident);
                        inv_map
                            .entry(field_path)
                            .or_default()
                            .push((item_id, Some((field_id.into(), None))));

                        None
                    }))
                }
                TypeDeclKind::Enum(variants) => TypeDeclSubIdentsOpt::Variant(
                    variants.map_ref_indexed(|variant_id, variant| {
                        let variant_ident = &variant.name;
                        let constructor_ident =
                            to_whyml(&ensure_u(variant_ident.into(), CONSTRUCTOR_PREFIX));
                        let mut constructor_path = path.clone();
                        constructor_path.insert(1, constructor_ident);

                        let record_idents_opt = if variant
                            .fields
                            .get(0)
                            .is_some_and(|field| field.name.is_some())
                        {
                            let record_ident = to_whyml(&camel_to_l(variant_ident));
                            let mut record_path = path.clone();
                            record_path.insert(1, record_ident);
                            inv_map
                                .entry(record_path)
                                .or_default()
                                .push((item_id, Some((variant_id.into(), Some(None)))));

                            Some((
                                None,
                                variant.fields.map_ref_indexed(|field_id, field| {
                                    let field_ident = field.name.as_ref().unwrap();
                                    let field_ident = to_whyml(&ensure_l(field_ident.into()));
                                    let mut field_path = constructor_path.clone();
                                    field_path.insert(2, field_ident);
                                    inv_map.entry(field_path).or_default().push((
                                        item_id,
                                        Some((variant_id.into(), Some(Some(field_id.into())))),
                                    ));

                                    None
                                }),
                            ))
                        } else {
                            None
                        };

                        inv_map
                            .entry(constructor_path)
                            .or_default()
                            .push((item_id, Some((variant_id.into(), None))));

                        ConstructorIdentsOpt {
                            ident: None,
                            record_idents_opt,
                        }
                    }),
                ),
                TypeDeclKind::Union(..) => todo!(),
                TypeDeclKind::Opaque | TypeDeclKind::Alias(..) => TypeDeclSubIdentsOpt::Abstract,
                TypeDeclKind::Error(..) => unreachable!(),
            };

            inv_map.entry(path).or_default().push((item_id, None));

            (None, sub_idents_opt)
        });
    let mut func_decl_names_opt = crate_.fun_decls.map_ref_indexed(|func_decl_id, func_decl| {
        let item_id = ItemId::from(func_decl_id);
        let path = &func_decl.item_meta.name;
        let path = process_path(path, |ident| ensure_l(ident.into()));
        inv_map.entry(path).or_default().push((item_id, None));

        None
    });
    let mut global_names_opt = crate_.global_decls.map_ref_indexed(|global_id, global| {
        let item_id = ItemId::from(global_id);
        let path = &global.item_meta.name;
        let path = process_path(path, |ident| upper_to_l(&ident));
        inv_map.entry(path).or_default().push((item_id, None));

        None
    });
    let mut spec_body_names_opt = crate_.spec_bodies.map_ref(|_| None);
    let mut spec_closure_names_opt = crate_.spec_closures.map_ref(|_| None);

    let mut set_map = |(item_id, sub_id_opt), path: Vec<_>| {
        let ident = path.join(SEPARATOR);
        match item_id {
            ItemId::Type(type_decl_id) => {
                let names_opt = &mut type_decl_names_opt[type_decl_id];
                match sub_id_opt {
                    None => {
                        names_opt.0 = Some(ident);
                    }
                    Some((sub_id, record_id_opt)) => match &mut names_opt.1 {
                        TypeDeclSubIdentsOpt::Record(field_idents_opt) => {
                            field_idents_opt[sub_id] = Some(ident);
                        }
                        TypeDeclSubIdentsOpt::Variant(variant_idents_opt) => {
                            let constructor_idents_opt = &mut variant_idents_opt[sub_id];
                            match record_id_opt {
                                // 构造子名是「类型名 + 构造子 + 命名空间」用 `''_` 拼起来的，
                                // 第一段是类型名（小写），因此拼接后还要再保证首字母大写，
                                // 否则 WhyML 会拒绝这样的构造子名（WhyML 要求构造子首字母大写）。
                                None => {
                                    constructor_idents_opt.ident =
                                        Some(ensure_u(ident.into(), CONSTRUCTOR_PREFIX))
                                }
                                Some(record_id) => {
                                    let record_idents_opt =
                                        constructor_idents_opt.record_idents_opt.as_mut().unwrap();
                                    match record_id {
                                        None => record_idents_opt.0 = Some(ident),
                                        Some(record_field_id) => {
                                            record_idents_opt.1[record_field_id] = Some(ident);
                                        }
                                    }
                                }
                            }
                        }
                        TypeDeclSubIdentsOpt::Abstract => unreachable!(),
                    },
                }
            }
            ItemId::Fun(func_decl_id) => {
                func_decl_names_opt[func_decl_id] = Some(ident);
            }
            ItemId::Global(global_id) => {
                global_names_opt[global_id] = Some(ident);
            }
            ItemId::TraitDecl(..) | ItemId::TraitImpl(..) => unreachable!(),
        }
    };
    for (path, ids) in inv_map {
        if !(path.len() == 1 && TAKEN_WORDS_SET.contains(path[0].as_str())) && ids.len() == 1 {
            let id = ids[0];
            set_map(id, path);
        } else {
            for (disambiguator, id) in ids.into_iter().enumerate() {
                let mut path = path.clone();
                path[0].push_str(&format!("'{disambiguator}"));
                set_map(id, path);
            }
        }
    }

    let mut local_mapper = LocalMapper {
        spec_closures: &crate_.spec_closures,
        spec_closure_names_opt: &mut spec_closure_names_opt,
    };
    let mut type_decl_names_opt = type_decl_names_opt.into_iter();
    let type_decl_names = crate_.type_decls.map_ref(|type_decl| {
        let (item_ident_opt, sub_idents_opt) = type_decl_names_opt.next().unwrap();
        let item_ident = item_ident_opt.unwrap();

        for &spec_body_id in &type_decl.specs.invariants {
            spec_body_names_opt[spec_body_id] =
                Some(local_mapper.map(&crate_.spec_bodies[spec_body_id], None, 0));
        }

        TypeDeclNames {
            item_ident,
            type_param_names: map_generic_params(&type_decl.generics, 0),
            sub_idents: match sub_idents_opt {
                TypeDeclSubIdentsOpt::Record(record_idents_opt) => {
                    TypeDeclSubIdents::Record(record_idents_opt.map(Option::unwrap))
                }
                TypeDeclSubIdentsOpt::Variant(variant_idents_opt) => {
                    TypeDeclSubIdents::Variant(variant_idents_opt.map(
                        |ConstructorIdentsOpt {
                             ident,
                             record_idents_opt,
                         }| {
                            ConstructorIdents {
                                constructor_ident: ident.unwrap(),
                                record_idents_opt: record_idents_opt.map(
                                    |(record_ident_opt, record_field_idents_opt)| {
                                        (
                                            record_ident_opt.unwrap(),
                                            record_field_idents_opt.map(Option::unwrap),
                                        )
                                    },
                                ),
                            }
                        },
                    ))
                }
                TypeDeclSubIdentsOpt::Abstract => TypeDeclSubIdents::Abstract,
            },
        }
    });
    let mut func_decl_names_opt = func_decl_names_opt.into_iter();
    let func_decl_names = crate_.fun_decls.map_ref(|func_decl| {
        let item_ident = func_decl_names_opt.next().unwrap().unwrap();
        let FunSpecs {
            preconditions,
            postconditions,
        } = &func_decl.specs;
        FuncDeclNames {
            item_ident,
            type_param_names: map_generic_params(&func_decl.generics, 0),
            local_names: local_mapper.map(&func_decl.body, Some(&func_decl.signature), 0),
            spec_names: FuncSpecNames {
                precondition_local_names: preconditions
                    .iter()
                    .map(|precondition| local_mapper.map(&precondition.body, None, 1))
                    .collect(),
                postcondition_local_names: postconditions
                    .iter()
                    .map(|postcondition| local_mapper.map(&postcondition.body, None, 1))
                    .collect(),
            },
        }
    });
    let mut global_names_opt = global_names_opt.into_iter();
    let global_names = crate_.global_decls.map_ref(|global| {
        let item_ident = global_names_opt.next().unwrap().unwrap();
        GlobalNames {
            item_ident,
            type_param_names: map_generic_params(&global.generics, 0),
        }
    });
    let spec_body_names = spec_body_names_opt.map(Option::unwrap);
    let spec_closure_names = spec_closure_names_opt.map(Option::unwrap);
    NameMap {
        type_decl_names,
        func_decl_names,
        global_names,
        spec_body_names,
        spec_closure_names,
    }
}

/// 把 LLBC 的名字路径转换成 WhyML 名字路径：
/// 最后一段用 `conv` 转换，其余各段仅做转义，并且把命名空间各段放到后面。
fn process_path(path: &Name, conv: impl Fn(String) -> String) -> Vec<String> {
    let path = &path.name;

    let last_ident_index = path
        .iter()
        .enumerate()
        .rev()
        .find(|(_i, path_elem)| matches!(path_elem, PathElem::Ident(..)))
        .unwrap()
        .0;

    let fmt_ctx = FmtCtx::new();
    let mut path: Vec<_> = path
        .iter()
        .map(|path_elem| format!("{}", path_elem.with_ctx(&fmt_ctx)))
        .collect();

    iter::once(&conv(mem::take(&mut path[last_ident_index])))
        .chain(&path[last_ident_index + 1..])
        .chain(&path[..last_ident_index])
        .map(|ident| to_whyml(ident))
        .collect()
}

/// 为泛型参数生成类型变量名。
fn map_generic_params(generic_params: &GenericParams, depth: usize) -> TypeParamNames {
    let type_param = &generic_params.types;

    let mut inv_map: HashMap<_, Vec<_>> = HashMap::new();
    let mut type_param_names = type_param.map_ref_indexed(|type_param_id, type_param| {
        let type_param_name = to_whyml(&camel_to_q(&type_param.name, TYPE_PARAM_PREFIX));
        inv_map
            .entry(type_param_name)
            .or_default()
            .push(type_param_id);

        String::default()
    });
    resolve_collision(inv_map, depth, |type_param_id, type_param_name| {
        type_param_names[type_param_id] = type_param_name;
    });
    type_param_names
}

use charon_lib::{
    ast::{
        Body, FieldId, FunDecl, FunDeclId, GenericParams, GlobalDeclId, ItemId, LocalId, Name, PathElem, TranslatedCrate, TypeDeclId, TypeDeclKind, TypeVarId, VariantId
    },
    formatter::FmtCtx,
    ids::{IndexMap, IndexVec},
    pretty::FmtWithCtx,
};
use utils::case;

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    iter, mem,
    rc::Rc,
    sync::LazyLock,
};

const SEPARATOR: &str = "''_";

pub const LIB_NAME: &str = "Lib'";

pub const EMPTY_TYPE_NAME: &str = "a'";

pub fn local_temp_name(i: usize) -> String {
    format!("x{i}'")
}

pub fn tuple_field_accessor_ident(arity: usize, field_id: FieldId) -> String {
    format!("get_tuple{arity}{SEPARATOR}{field_id}'")
}

pub fn variant_constructor_accessor_ident(constructor_ident: &str) -> String {
    format!("get_{constructor_ident}'")
}

pub fn variant_constructor_field_accessor_ident(constructor_ident: &str, field_id: FieldId) -> String {
    format!("get_{constructor_ident}{SEPARATOR}{field_id}'")
}

pub fn break_exn_name(loop_depth: usize) -> String {
    format!("Break'{loop_depth}'")
}

pub fn continue_exn_name(loop_depth: usize) -> String {
    format!("Continue'{loop_depth}'")
}

pub struct NameMap {
    pub root_module_name: String,
    pub module_names: HashSet<Rc<str>>,
    pub type_decl_names: IndexMap<TypeDeclId, TypeDeclNames>,
    pub func_decl_names: IndexMap<FunDeclId, FuncDeclNames>,
    pub global_names: IndexMap<GlobalDeclId, GlobalNames>,
}

pub struct TypeDeclNames {
    pub module_name: Rc<str>,
    pub item_ident: String,
    pub type_param_names: TypeParamNames,
    pub sub_idents: TypeDeclSubIdents,
}

pub enum TypeDeclSubIdents {
    Record(IndexVec<FieldId, String>),
    Variant(IndexVec<VariantId, (String, Option<(String, IndexVec<FieldId, String>)>)>),
    Abstract,
}

pub struct FuncDeclNames {
    pub module_name: Rc<str>,
    pub item_ident: String,
    pub type_param_names: TypeParamNames,
    pub local_names: FuncDeclLocalNames,
}

pub enum FuncDeclLocalNames {
    Concrete(IndexVec<LocalId, String>),
    Abstract(Vec<Option<String>>),
}

pub struct GlobalNames {
    pub module_name: Rc<str>,
    pub item_ident: String,
    pub type_param_names: TypeParamNames,
}

pub type TypeParamNames = IndexVec<TypeVarId, String>;

pub fn build(crate_: &TranslatedCrate) -> NameMap {
    type Id = (ItemId, Option<(usize, Option<Option<usize>>)>);

    let mut inv_maps: HashMap<_, HashMap<_, Vec<Id>>> = HashMap::new();

    enum TypeDeclSubIdentsOpt {
        Record(IndexVec<FieldId, Option<String>>),
        Variant(IndexVec<VariantId, (Option<String>, Option<(Option<String>, IndexVec<FieldId, Option<String>>)>)>),
        Abstract,
    }

    let mut type_decl_names_opt = crate_.type_decls.map_ref_indexed(|type_decl_id, type_decl| {
        let item_id = ItemId::from(type_decl_id);
        let path = &type_decl.item_meta.name;
        let (module_name, path) = process_path(path, |ident| camel_to_l(&ident));
        let inv_map = inv_maps.entry(module_name).or_default();

        let sub_idents_opt = match &type_decl.kind {
            TypeDeclKind::Struct(fields) => TypeDeclSubIdentsOpt::Record(
                fields.map_ref_indexed(|field_id, field| {
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
                }),
            ),
            TypeDeclKind::Enum(variants) => TypeDeclSubIdentsOpt::Variant(
                variants.map_ref_indexed(|variant_id, variant| {
                    let variant_ident = &variant.name;
                    let constructor_ident = to_whyml(&ensure_u(variant_ident.into(), CONSTRUCTOR_PREFIX));
                    let mut constructor_path = path.clone();
                    constructor_path.insert(1, constructor_ident);

                    let record_idents_opt =
                        if variant.fields.get(0).is_some_and(|field| field.name.is_some()) {
                            let record_ident = to_whyml(&camel_to_l(variant_ident));
                            let mut record_path = path.clone();
                            record_path.insert(1, record_ident);
                            inv_map.entry(record_path).or_default().push((item_id, Some((variant_id.into(), Some(None)))));

                            Some((
                                None,
                                variant.fields.map_ref_indexed(|field_id, field| {
                                    let field_ident = field.name.as_ref().unwrap();
                                    let field_ident = to_whyml(&ensure_l(field_ident.into()));
                                    let mut field_path = constructor_path.clone();
                                    field_path.insert(2, field_ident);
                                    inv_map
                                        .entry(field_path)
                                        .or_default()
                                        .push((item_id, Some((variant_id.into(), Some(Some(field_id.into()))))));

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

                    (None, record_idents_opt)
                }),
            ),
            TypeDeclKind::Union(..) => todo!(),
            TypeDeclKind::Opaque | TypeDeclKind::Alias(..) => TypeDeclSubIdentsOpt::Abstract,
            TypeDeclKind::Error(..) => unreachable!(),
        };

        inv_map
            .entry(path)
            .or_default()
            .push((item_id, None));

        (None, sub_idents_opt)
    });
    let mut func_decl_names_opt = crate_.fun_decls.map_ref_indexed(|func_decl_id, func_decl| {
        let item_id = ItemId::from(func_decl_id);
        let path = &func_decl.item_meta.name;
        let (module_name, path) = process_path(path, |ident| ensure_l(ident.into()));
        inv_maps
            .entry(module_name)
            .or_default()
            .entry(path)
            .or_default()
            .push((item_id, None));

        None
    });
    let mut global_names_opt = crate_.global_decls.map_ref_indexed(|global_id, global| {
        let item_id = ItemId::from(global_id);
        let path = &global.item_meta.name;
        let (module_name, path) = process_path(path, |ident| upper_to_l(&ident));
        inv_maps
            .entry(module_name)
            .or_default()
            .entry(path)
            .or_default()
            .push((item_id, None));

        None
    });

    let mut module_idents: HashMap<_, HashSet<_>> = HashMap::new();

    let mut set_map = |(item_id, sub_id_opt),
                        module_name: Rc<str>,
                        path: Vec<_>,
                        module_idents: &mut HashSet<_>| {
        let ident = path.join(SEPARATOR);
        {
            let ident = ident.clone();
            match item_id {
                ItemId::Type(type_decl_id) => {
                    let names_opt = &mut type_decl_names_opt[type_decl_id];
                    match sub_id_opt {
                        None => {
                            names_opt.0 = Some((module_name, ident));
                        }
                        Some((sub_id, record_id_opt)) => {
                            match &mut names_opt.1 {
                                TypeDeclSubIdentsOpt::Record(field_idents_opt) => field_idents_opt[sub_id] = Some(ident),
                                TypeDeclSubIdentsOpt::Variant(variant_idents_opt) => {
                                    let constructor_idents_opt = &mut variant_idents_opt[sub_id];
                                    match record_id_opt {
                                        None => constructor_idents_opt.0 = Some(ident),
                                        Some(record_id) => {
                                            let record_idents_opt = constructor_idents_opt.1.as_mut().unwrap();
                                            match record_id {
                                                None => record_idents_opt.0 = Some(ident),
                                                Some(record_field_id) => record_idents_opt.1[record_field_id] = Some(ident),
                                            }
                                        }
                                    }
                                }
                                TypeDeclSubIdentsOpt::Abstract => unreachable!(),
                            }
                        }
                    }
                }
                ItemId::Fun(func_decl_id) => {
                    func_decl_names_opt[func_decl_id] = Some((module_name, ident));
                }
                ItemId::Global(global_id) => {
                    global_names_opt[global_id] = Some((module_name, ident));
                }
                ItemId::TraitDecl(..) | ItemId::TraitImpl(..) => unreachable!(),
            }
        }
        module_idents.insert(ident);
    };
    for (module_name, inv_map) in inv_maps {
        let module_name: Rc<str> = module_name.into();
        let module_idents = module_idents
            .entry(module_name.clone())
            .insert_entry(HashSet::new())
            .into_mut();
        for (path, ids) in inv_map {
            if !(path.len() == 1 && TAKEN_WORDS_SET.contains(&*path[0])) && ids.len() == 1 {
                let id = ids[0];
                set_map(id, module_name.clone(), path, module_idents);
            } else {
                for (disambiguator, id) in ids.into_iter().enumerate() {
                    let mut path = path.clone();
                    path[0].push_str(&format!("'{disambiguator}"));
                    set_map(id, module_name.clone(), path, module_idents);
                }
            }
        }
    }

    let mut type_decl_names_opt = type_decl_names_opt.into_iter();
    let type_decl_names = crate_.type_decls.map_ref(|type_decl| {
        let (item_path_opt, sub_idents_opt) = type_decl_names_opt.next().unwrap();
        let (module_name, item_ident) = item_path_opt.unwrap();
        let module_idents = &module_idents[&module_name];
        TypeDeclNames {
            module_name,
            item_ident,
            type_param_names: map_generic_params(module_idents, &type_decl.generics),
            sub_idents: match sub_idents_opt {
                TypeDeclSubIdentsOpt::Record(record_idents_opt) => TypeDeclSubIdents::Record(
                    record_idents_opt.map(Option::unwrap),
                ),
                TypeDeclSubIdentsOpt::Variant(variant_idents_opt) => TypeDeclSubIdents::Variant(
                    variant_idents_opt
                        .map(|(constructor_ident_opt, record_idents_opt_opt)| {
                            (constructor_ident_opt.unwrap(), record_idents_opt_opt.map(|(record_ident_opt, record_field_idents_opt)| {
                                (record_ident_opt.unwrap(), record_field_idents_opt.map(Option::unwrap))
                            }))
                        }),
                ),
                TypeDeclSubIdentsOpt::Abstract => TypeDeclSubIdents::Abstract,
            },
        }
    });
    let mut func_decl_names_opt = func_decl_names_opt.into_iter();
    let func_decl_names = crate_.fun_decls.map_ref(|func_decl| {
        let (module_name, item_ident) = func_decl_names_opt.next().unwrap().unwrap();
        let module_idents = &module_idents[&module_name];
        FuncDeclNames {
            module_name,
            item_ident,
            type_param_names: map_generic_params(module_idents, &func_decl.generics),
            local_names: map_locals(module_idents, func_decl),
        }
    });
    let mut global_names_opt = global_names_opt.into_iter();
    let global_names = crate_.global_decls.map_ref(|global| {
        let (module_name, item_ident) = global_names_opt.next().unwrap().unwrap();
        let module_idents = &module_idents[&module_name];
        GlobalNames {
            module_name,
            item_ident,
            type_param_names: map_generic_params(module_idents, &global.generics),
        }
    });
    NameMap {
        root_module_name: crate_to_module(&crate_.crate_name),
        module_names: module_idents.into_keys().collect(),
        type_decl_names,
        func_decl_names,
        global_names,
    }
}

fn crate_to_module(ident: &str) -> String {
    to_whyml(&snake_to_u(ident, MODULE_PREFIX))
}

fn process_path(path: &Name, conv: impl Fn(String) -> String) -> (String, Vec<String>) {
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

    let module_name = crate_to_module(&path[0]);
    let path: Vec<_> = iter::once(&conv(mem::take(&mut path[last_ident_index])))
        .chain(&path[last_ident_index + 1..])
        .chain(&path[1..last_ident_index])
        .map(|ident| to_whyml(ident))
        .collect();

    (module_name, path)
}

fn map_generic_params(
    module_idents: &HashSet<String>,
    generic_params: &GenericParams,
) -> TypeParamNames {
    let type_param = &generic_params.types;

    let mut inv_map: HashMap<_, Vec<_>> = HashMap::new();
    let mut idents = type_param.map_ref_indexed(|type_param_id, type_param| {
        let ident = to_whyml(&camel_to_q(&type_param.name, TYPE_PARAM_PREFIX));
        inv_map.entry(ident).or_default().push(type_param_id);

        String::default()
    });

    resolve_collision(module_idents, inv_map, |type_param_id, ident| {
        idents[type_param_id] = ident;
    });
    idents
}

fn map_locals(module_idents: &HashSet<String>, func_decl: &FunDecl) -> FuncDeclLocalNames {
    match &func_decl.body {
        Body::Unstructured(_body) => unreachable!(),
        Body::Structured(body) => {
            let locals = &body.locals.locals;

            let mut inv_map: HashMap<_, Vec<_>> = HashMap::new();
            let mut idents = locals.map_ref_indexed(|local_id, local| {
                let ident = to_whyml(&ensure_l(format!("{local}").into()));
                inv_map.entry(ident).or_default().push(local_id);

                String::default()
            });

            resolve_collision(module_idents, inv_map, |local_id, ident| {
                idents[local_id] = ident;
            });
            FuncDeclLocalNames::Concrete(idents)
        }
        Body::TargetDispatch(..) => todo!(),
        Body::TraitMethodWithoutDefault | Body::Extern(..) | Body::Opaque | Body::Missing => FuncDeclLocalNames::Abstract(vec![None; func_decl.signature.inputs.len()]),
        Body::Intrinsic { arg_names, .. } => {
            FuncDeclLocalNames::Abstract(arg_names.iter().map(|param_name_opt| {
                param_name_opt.as_ref().map(|param_name| {
                    to_whyml(&ensure_l(param_name.into()))
                })
            }).collect())
        }
        Body::Error(..) => unreachable!(),
    }
}

fn resolve_collision<I: Copy>(module_idents: &HashSet<String>, inv_map: HashMap<String, Vec<I>>, mut set_map: impl FnMut(I, String)) {
    for (ident, ids) in inv_map {
        if !TAKEN_WORDS_SET.contains(ident.as_str())
            && !module_idents.contains(&ident)
            && ids.len() == 1
        {
            let id = ids[0];
            set_map(id, ident);
        } else {
            let mut disambiguator = 0;
            while module_idents.contains(&format!("{ident}'{disambiguator}")) {
                disambiguator += 1;
            }
            for id in ids {
                set_map(id, format!("{ident}'{disambiguator}"));
                disambiguator += 1;
            }
        }
    }
}

const MODULE_PREFIX: &str = "M";

const CONSTRUCTOR_PREFIX: &str = "C";

const TYPE_PARAM_PREFIX: &str = "a";

fn snake_to_u(ident: &str, prefix: &str) -> String {
    if case::is_snake_case(ident) {
        let ident = case::to_camel_case(ident);
        if ident.as_bytes().first().is_some_and(u8::is_ascii_uppercase) {
            ident
        } else {
            format!("{prefix}{ident}")
        }
    } else {
        format!("{prefix}{ident}")
    }
}

fn camel_to_l(ident: &str) -> String {
    if case::is_camel_case(ident) {
        let ident = case::to_snake_case(ident);
        if let Some(c) = ident.as_bytes().first() {
            if c.is_ascii_lowercase() {
                ident
            } else {
                format!("_{ident}")
            }
        } else {
            "__".into()
        }
    } else {
        format!("_{ident}")
    }
}

fn upper_to_l(ident: &str) -> String {
    if case::is_upper_case(ident) {
        let ident = case::to_snake_case(ident);
        if let Some(c) = ident.as_bytes().first() {
            if c.is_ascii_lowercase() {
                ident
            } else {
                format!("_{ident}")
            }
        } else {
            "__".into()
        }
    } else {
        format!("_{ident}")
    }
}

fn camel_to_q(ident: &str, prefix: &str) -> String {
    if case::is_camel_case(ident) {
        let ident = case::to_snake_case(ident);
        if let Some(c) = ident.as_bytes().first() {
            if c.is_ascii_lowercase() {
                ident
            } else {
                format!("{prefix}_{ident}")
            }
        } else {
            prefix.into()
        }
    } else {
        format!("{prefix}_{ident}")
    }
}

fn ensure_l(ident: Cow<str>) -> String {
    if ident.as_bytes()[0].is_ascii_lowercase() {
        ident.into_owned()
    } else {
        format!("_{ident}")
    }
}

fn ensure_u(ident: Cow<str>, prefix: &str) -> String {
    if ident.as_bytes()[0].is_ascii_uppercase() {
        ident.into_owned()
    } else {
        format!("{prefix}{ident}")
    }
}

fn to_whyml(ident: &str) -> String {
    let mut ident_ = String::with_capacity(ident.len());
    for c in ident.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            ident_.push(c);
        } else if c.is_ascii() {
            ident_.push_str("'_");
        } else {
            ident_.push_str(&format!("''{}", c as u32));
        }
    }
    ident_
}

static TAKEN_WORDS_SET: LazyLock<HashSet<&str>> = LazyLock::new(|| {
    const TAKEN_WORDS: &[&str] = &[
        // Keywords
        "abstract",
        "absurd",
        "alias",
        "any",
        "as",
        "assert",
        "assume",
        "at",
        "axiom",
        "begin",
        "break",
        "by",
        "check",
        "clone",
        "coinductive",
        "constant",
        "continue",
        "diverges",
        "do",
        "done",
        "downto",
        "else",
        "end",
        "ensures",
        "epsilon",
        "exception",
        "exists",
        "export",
        "false",
        "for",
        "forall",
        "fun",
        "function",
        "ghost",
        "goal",
        "if",
        "import",
        "in",
        "inductive",
        "invariant",
        "label",
        "lemma",
        "let",
        "match",
        "meta",
        "module",
        "mutable",
        "not",
        "old",
        "partial",
        "predicate",
        "private",
        "pure",
        "raise",
        "raises",
        "reads",
        "rec",
        "requires",
        "return",
        "returns",
        "scope",
        "so",
        "then",
        "theory",
        "to",
        "true",
        "try",
        "type",
        "use",
        "val",
        "variant",
        "while",
        "with",
        "writes",
        "float",
        "range",
        "ref",
        // Pre-defined identifiers
        "int",
        "real",
        "string",
        "bool",
        "True",
        "False",
        "tuple0",
        "Tuple0",
        "unit",
    ];

    let mut map = HashSet::new();
    for &word in TAKEN_WORDS {
        map.insert(word);
    }
    map
});

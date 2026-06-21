use charon_lib::{
    ast::{
        Body, FieldId, FunDeclId, FunSig, FunSpecs, GenericParams, GlobalDeclId, ItemId, LocalId,
        Name, PathElem, SpecBodyId, SpecClosureId, TranslatedCrate, TypeDeclId, TypeDeclKind,
        TypeVarId, VariantId,
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
    sync::LazyLock,
};

const SEPARATOR: &str = "''_";

pub const LIB_DIR: &str = "whyml_lib";

pub fn lib_alias(lib_name: &str) -> String {
    format!("{lib_name}'")
}

pub static LIB_BOOL: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["bool", "Bool"], lib_alias("Bool")));

pub static LIB_BUILTIN: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["builtin", "Builtin"], lib_alias("Builtin")));

pub static LIB_CHAR: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["char", "Char"], lib_alias("Char")));

pub static LIB_I8: LazyLock<(&[&str], String)> = LazyLock::new(|| (&["i8", "I8"], lib_alias("I8")));

pub static LIB_I16: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["i16", "I16"], lib_alias("I16")));

pub static LIB_I32: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["i32", "I32"], lib_alias("I32")));

pub static LIB_I64: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["i64", "I64"], lib_alias("I64")));

pub static LIB_I128: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["i128", "I128"], lib_alias("I128")));

pub static LIB_TUPLE: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["tuple", "Tuple"], lib_alias("Tuple")));

pub static LIB_U8: LazyLock<(&[&str], String)> = LazyLock::new(|| (&["u8", "U8"], lib_alias("U8")));

pub static LIB_U16: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["u16", "U16"], lib_alias("U16")));

pub static LIB_U32: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["u32", "U32"], lib_alias("U32")));

pub static LIB_U64: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["u64", "U64"], lib_alias("U64")));

pub static LIB_U128: LazyLock<(&[&str], String)> =
    LazyLock::new(|| (&["u128", "U128"], lib_alias("U128")));

pub const EMPTY_TYPE_NAME: &str = "a'";

pub fn local_temp_name(i: usize) -> String {
    format!("x{i}'")
}

pub fn local_block_name(i: usize) -> String {
    format!("b{i}'")
}

pub fn tuple_field_accessor_ident(arity: usize, field_id: FieldId) -> String {
    format!("get_tuple{arity}{SEPARATOR}{field_id}")
}

pub fn variant_constructor_accessor_ident(constructor_ident: &str) -> String {
    format!("get_{constructor_ident}''")
}

pub fn variant_constructor_field_accessor_ident(
    constructor_ident: &str,
    field_id: FieldId,
) -> String {
    format!("get_{constructor_ident}{SEPARATOR}{field_id}'")
}

pub fn break_exn_name(loop_depth: usize) -> String {
    format!("Break'{loop_depth}'")
}

pub fn continue_exn_name(loop_depth: usize) -> String {
    format!("Continue'{loop_depth}'")
}

pub struct NameMap {
    pub type_decl_names: IndexMap<TypeDeclId, TypeDeclNames>,
    pub func_decl_names: IndexMap<FunDeclId, FuncDeclNames>,
    pub global_names: IndexMap<GlobalDeclId, GlobalNames>,
    pub spec_body_names: IndexMap<SpecBodyId, LocalNames>,
    pub spec_closure_names: IndexMap<SpecClosureId, LocalNames>,
}

pub struct TypeDeclNames {
    pub item_ident: String,
    pub type_param_names: TypeParamNames,
    pub sub_idents: TypeDeclSubIdents,
}

pub enum TypeDeclSubIdents {
    Record(IndexVec<FieldId, String>),
    Variant(IndexVec<VariantId, ConstructorIdents>),
    Abstract,
}

pub struct ConstructorIdents {
    pub constructor_ident: String,
    pub record_idents_opt: Option<(String, IndexVec<FieldId, String>)>,
}

pub struct FuncDeclNames {
    pub item_ident: String,
    pub type_param_names: TypeParamNames,
    pub local_names: LocalNames,
    pub spec_names: FuncSpecNames,
}

pub struct FuncSpecNames {
    pub precondition_local_names: Vec<LocalNames>,
    pub postcondition_local_names: Vec<LocalNames>,
}

pub struct GlobalNames {
    pub item_ident: String,
    pub type_param_names: TypeParamNames,
}

pub enum LocalNames {
    Concrete(IndexVec<LocalId, String>),
    Abstract(Vec<Option<String>>),
}

pub type TypeParamNames = IndexVec<TypeVarId, String>;

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
                                None => constructor_idents_opt.ident = Some(ident),
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

    let mut type_decl_names_opt = type_decl_names_opt.into_iter();
    let type_decl_names = crate_.type_decls.map_ref(|type_decl| {
        let (item_ident_opt, sub_idents_opt) = type_decl_names_opt.next().unwrap();
        let item_ident = item_ident_opt.unwrap();
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
            local_names: map_locals(&func_decl.body, Some(&func_decl.signature), 0),
            spec_names: FuncSpecNames {
                precondition_local_names: preconditions
                    .iter()
                    .map(|precondition| map_locals(&precondition.body, None, 1))
                    .collect(),
                postcondition_local_names: postconditions
                    .iter()
                    .map(|postcondition| map_locals(&postcondition.body, None, 1))
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
    let spec_body_names = crate_
        .spec_bodies
        .map_ref(|spec_body| map_locals(spec_body, None, 1));
    let spec_closure_names = crate_
        .spec_closures
        .map_ref(|spec_closure| map_locals(&spec_closure.body, None, 1));
    NameMap {
        type_decl_names,
        func_decl_names,
        global_names,
        spec_body_names,
        spec_closure_names,
    }
}

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

fn map_locals(body: &Body, signature_opt: Option<&FunSig>, depth: usize) -> LocalNames {
    match body {
        Body::Unstructured(_body) => unreachable!(),
        Body::Structured(body) => {
            let locals = &body.locals.locals;

            let mut inv_map: HashMap<_, Vec<_>> = HashMap::new();
            let mut local_names = locals.map_ref_indexed(|local_id, local| {
                let local_name = to_whyml(&ensure_l(format!("{local}").into()));
                inv_map.entry(local_name).or_default().push(local_id);

                String::default()
            });

            resolve_collision(inv_map, depth, |local_id, local_name| {
                local_names[local_id] = local_name;
            });
            LocalNames::Concrete(local_names)
        }
        Body::TargetDispatch(..) => todo!(),
        Body::TraitMethodWithoutDefault | Body::Extern(..) | Body::Opaque | Body::Missing => {
            LocalNames::Abstract(vec![None; signature_opt.unwrap().inputs.len()])
        }
        Body::Intrinsic { arg_names, .. } => LocalNames::Abstract(
            arg_names
                .iter()
                .map(|param_name_opt| {
                    param_name_opt
                        .as_ref()
                        .map(|param_name| to_whyml(&ensure_l(param_name.into())))
                })
                .collect(),
        ),
        Body::Error(..) => unreachable!(),
    }
}

fn resolve_collision<I: Copy>(
    inv_map: HashMap<String, Vec<I>>,
    depth: usize,
    mut set_map: impl FnMut(I, String),
) {
    for (name, ids) in inv_map {
        if !TAKEN_WORDS_SET.contains(name.as_str()) && ids.len() == 1 {
            let id = ids[0];
            set_map(id, format!("{name}'{depth}"));
        } else {
            for (disambiguator, id) in ids.into_iter().enumerate() {
                set_map(id, format!("{name}'{disambiguator}'{depth}"));
            }
        }
    }
}

const CONSTRUCTOR_PREFIX: &str = "C";

const TYPE_PARAM_PREFIX: &str = "a";

// fn snake_to_u(ident: &str, prefix: &str) -> String {
//     if case::is_snake_case(ident) {
//         let ident = case::to_camel_case(ident);
//         if ident.as_bytes().first().is_some_and(u8::is_ascii_uppercase) {
//             ident
//         } else {
//             format!("{prefix}{ident}")
//         }
//     } else {
//         ensure_u(ident.into(), prefix)
//     }
// }

fn camel_to_l(ident: &str) -> String {
    if case::is_camel_case(ident) {
        let ident = case::to_snake_case(ident);
        if let Some(c) = ident.as_bytes().first() {
            if is_l(*c) { ident } else { format!("_{ident}") }
        } else {
            "__".into()
        }
    } else {
        ensure_l(ident.into())
    }
}

fn upper_to_l(ident: &str) -> String {
    if case::is_upper_case(ident) {
        let ident = case::to_snake_case(ident);
        if let Some(c) = ident.as_bytes().first() {
            if is_l(*c) { ident } else { format!("_{ident}") }
        } else {
            "__".into()
        }
    } else {
        ensure_l(ident.into())
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
    if is_l(ident.as_bytes()[0]) {
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

fn is_l(c: u8) -> bool {
    c.is_ascii_lowercase() || c == b'_'
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

    let mut word_set = HashSet::new();
    for &word in TAKEN_WORDS {
        word_set.insert(word);
    }
    word_set
});

//! LLBC（Charon 的中间表示）到 WhyML（Why3 的输入语言）的翻译。
//!
//! 整体流程：
//!
//! 1. 对每个 crate 调用 `ctx::translate_crate`，得到对应的 WhyML 文件；
//! 2. 每个 crate 写成一个同名 `.mlw` 文件；
//! 3. 写出 `trust2_contract` 支撑库（内嵌在本二进制中），并为本次翻译用到的
//!    元组投影生成 `tuple.mlw`。
//!
//! 模块划分：
//!
//! - `builder`：`Expr` 与 `Term` 两种输出共用的构造器抽象；
//! - `ctx`：翻译上下文 `Ctx` 与各部分（类型、语句、右值……）的翻译逻辑；
//! - `error`：翻译错误类型；
//! - `ident`：WhyML 标识符的构造；
//! - `name_map`：LLBC 名字到 WhyML 名字的映射；
//! - `prelude`：输出中反复使用的静态项与支撑库导入。

mod builder;
mod ctx;
mod error;
mod ident;
mod name_map;
mod prelude;

pub use error::{Error, Result};

use ctx::translate_crate;
use ident::{local_temp_ident, translate_ident, tuple_field_accessor_ident};
use prelude::WILDCARD;

use bumpalo::Bump;
use charon_lib::ast::{FieldId, TranslatedCrate};
use include_dir::{Dir, include_dir};
use ocaml_format::{Doc, FormattingOptions};
use why3_ptree::{
    expr::RsKind,
    ity::Mask,
    loc::Position,
    mlw_printer,
    ptree::{Decl, ExprDesc, MlwFile, PatDesc, Qualid},
    ptree_helpers,
};

use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Write,
    path::Path,
};

/// 把若干 crate 的 LLBC 翻译成 WhyML 并写到 `why3_out_dir`。
pub fn translate_crates(
    crates: &mut HashMap<String, TranslatedCrate>,
    why3_out_dir: &Path,
) -> anyhow::Result<()> {
    let mut tuple_field_accesses = HashSet::new();
    for (crate_name_, crate_) in crates {
        let (whyml_file, tuple_field_accesses_) = translate_crate(crate_)?;

        write_file(why3_out_dir.join(format!("{crate_name_}.mlw")), &whyml_file)?;

        tuple_field_accesses.extend(tuple_field_accesses_);
    }

    const WHYML_LIB: Dir<'_> =
        include_dir!("$CARGO_MANIFEST_DIR/src/llbc_to_whyml/trust2_contract");
    let whyml_lib_out_dir = why3_out_dir.join("trust2_contract");
    fs::create_dir_all(&whyml_lib_out_dir)?;
    WHYML_LIB.extract(&whyml_lib_out_dir)?;

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

/// 把 WhyML 语法树格式化后写入文件。
fn write_file(path: impl AsRef<Path>, whyml_file: &MlwFile) -> anyhow::Result<()> {
    let mut file = File::create(path)?;
    let arena = Bump::new();
    let mut doc = Doc::new();
    mlw_printer::pp_mlw_file(None, &mut doc, &arena, whyml_file);
    write!(file, "{}", doc.display(&FormattingOptions::default()))?;
    Ok(())
}

/// 生成元组字段访问函数 `get_tuple<arity>''_<field_id>`。
///
/// WhyML 没有元组投影语法，因此每个用到的投影都生成一个模式匹配函数。
fn translate_tuple_field_access(arity: usize, field_id: FieldId) -> Decl {
    Decl::Let(
        tuple_field_accessor_ident(arity, field_id),
        false,
        RsKind::Func,
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

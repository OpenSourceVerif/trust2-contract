use crate::{spec_ast::Spec as PSpec, spec_lower};

use charon_lib::ast::{FunDeclId, TranslatedCrate};
use proc_macro_crate::FoundCrate;

use anyhow::{Context, Result, bail};

use std::{
    collections::{BTreeMap, HashMap},
    ffi::{OsStr, OsString},
    fmt::{self, Display, Formatter},
    fs, iter,
    path::PathBuf,
};

/// A translated crate plus cargo-verify-specific spec lowering output.
pub struct TranslatedCrateBundle {
    pub translated: TranslatedCrate,
    pub lowered_specs: BTreeMap<FunDeclId, PSpec>,
}

impl Display for TranslatedCrateBundle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.translated)?;
        if self.lowered_specs.is_empty() {
            return Ok(());
        }

        writeln!(f)?;
        writeln!(f, "lowered_specs:")?;
        for (fun_id, fun_decl) in self.translated.fun_decls.iter_indexed_values() {
            let Some(spec) = self.lowered_specs.get(&fun_id) else {
                continue;
            };
            writeln!(
                f,
                "  {}:",
                spec_lower::name_to_string(&fun_decl.item_meta.name)
            )?;
            for line in spec.to_string().lines() {
                writeln!(f, "    {line}")?;
            }
        }
        Ok(())
    }
}

pub fn translate_crates(
    charon_out_dir: Option<PathBuf>,
    cargo_build_args: Vec<OsString>,
) -> Result<HashMap<String, TranslatedCrateBundle>> {
    let mut crates = HashMap::new();

    let feature_args: Box<dyn Iterator<Item = OsString>> = {
        let cargo_manifest_dir = utils::cargo_manifest_dir()?;
        let cargo_manifest_dir = cargo_manifest_dir.into_os_string().into_string().unwrap();
        match proc_macro_crate::package_name("trust2-contract", cargo_manifest_dir) {
            Ok(FoundCrate::Name(name)) => {
                Box::new(["--features".into(), format!("{name}/verify").into()].into_iter())
            }
            _ => Box::new(iter::empty()),
        }
    };

    let with_charon_out_dir = |charon_out_dir: PathBuf| {
        charon::main_(
            [
                "charon".into(),
                "cargo".into(),
                "--translate-all-methods".into(),
                "--hide-marker-traits".into(),
                "--hide-allocator".into(),
                "--treat-box-as-builtin".into(),
                "--reconstruct-asserts".into(),
                "--dest".into(),
                charon_out_dir.clone().into(),
                "--".into(),
            ]
            .into_iter()
            .chain(cargo_build_args)
            .chain(feature_args),
        )?;

        for dir_entry in fs::read_dir(&charon_out_dir)? {
            let llbc_file = dir_entry?.path();
            if llbc_file.is_file() && llbc_file.extension() == Some(OsStr::new("llbc")) {
                let translated = charon_lib::deserialize_llbc(&llbc_file)?;
                let lowered_specs = spec_lower::lower_crate_specs(&translated)?;
                let crate_name = translated.crate_name.clone();
                let crate_ = TranslatedCrateBundle {
                    translated,
                    lowered_specs,
                };
                if crates.insert(crate_name.clone(), crate_).is_some() {
                    bail!("duplicate crate name: {crate_name}");
                }
            }
        }

        Ok(())
    };

    if let Some(charon_out_dir) = charon_out_dir {
        with_charon_out_dir(charon_out_dir)?;
    } else {
        let temp_dir = tempfile::tempdir()?;
        let result = with_charon_out_dir(temp_dir.path().into());
        let err_msg = format!(
            "failed to delete temporary directory: {}",
            temp_dir.path().display(),
        );
        temp_dir.close().context(err_msg)?;
        result?;
    }

    Ok(crates)
}

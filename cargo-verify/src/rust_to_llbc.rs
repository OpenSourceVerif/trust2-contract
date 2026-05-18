use charon_lib::ast::TranslatedCrate;
use proc_macro_crate::FoundCrate;
use utils::cargo;

use anyhow::{Result, bail};

use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fs, iter,
    path::PathBuf,
};

pub fn translate_crates(
    charon_out_dir: PathBuf,
    cargo_build_args: Vec<OsString>,
) -> Result<HashMap<String, TranslatedCrate>> {
    let mut crates = HashMap::new();

    let feature_args: Box<dyn Iterator<Item = OsString>> = {
        let cargo_manifest_dir = cargo::cargo_manifest_dir()?;
        let cargo_manifest_dir = cargo_manifest_dir.into_os_string().into_string().unwrap();
        match proc_macro_crate::package_name("trust2-contract", cargo_manifest_dir) {
            Ok(FoundCrate::Name(name)) => {
                Box::new(["--features".into(), format!("{name}/verify").into()].into_iter())
            }
            _ => Box::new(iter::empty()),
        }
    };

    charon::main_(
        [
            "charon".into(),
            "cargo".into(),
            // Do not enable:
            // --unbind-item-vars
            "--abort-on-error".into(),
            "--translate-all-methods".into(),
            "--monomorphize".into(),
            "--treat-box-as-builtin".into(),
            "--precise-drops".into(),
            // "--desugar-drops".into(),
            "--index-to-function-calls".into(),
            "--ops-to-function-calls".into(),
            "--reconstruct-asserts".into(),
            "--hide-allocator".into(),
            "--hide-marker-traits".into(),
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
            let crate_ = charon_lib::deserialize_llbc(&llbc_file)?;
            if let Some(crate_) = crates.insert(crate_.crate_name.clone(), crate_) {
                bail!("duplicate crate name: {}", crate_.crate_name);
            }
        }
    }

    Ok(crates)
}

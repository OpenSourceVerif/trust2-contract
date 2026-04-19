use charon_lib::ast::TranslatedCrate;
use proc_macro_crate::FoundCrate;

use anyhow::{Context, Result, bail};

use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fs, iter,
    path::PathBuf,
};

pub fn translate_crates(
    charon_out_dir: Option<PathBuf>,
    cargo_build_args: Vec<OsString>,
) -> Result<HashMap<String, TranslatedCrate>> {
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
                let crate_ = charon_lib::deserialize_llbc(&llbc_file)?;
                if let Some(crate_) = crates.insert(crate_.crate_name.clone(), crate_) {
                    bail!("duplicate crate name: {}", crate_.crate_name);
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

use charon_lib::ast::TranslatedCrate;
use proc_macro_crate::FoundCrate;

use anyhow::{Context, Result, bail};
use clap::Parser;
use clap_cargo::style::CLAP_STYLING;
use yansi::{Condition, Paint};

use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fs, iter,
    path::PathBuf,
};

#[derive(Parser)]
#[command(bin_name("cargo"), styles(CLAP_STYLING), help_expected(true))]
enum Cli {
    /// Verify contracts specified with trust2-contract package
    Verify(CliConfig),
}

#[derive(Parser)]
struct CliConfig {
    /// Charon LLBC output directory. If not provided, a temporary directory is used and then deleted.
    #[arg(long)]
    charon_out_dir: Option<PathBuf>,
    /// Pretty-print Charon LLBC.
    #[arg(long)]
    charon_pretty_print: bool,
    /// `cargo build` arguments
    #[arg(last(true), value_name("ARGS"))]
    cargo_build_args: Vec<OsString>,
}

fn main() -> Result<()> {
    yansi::whenever(Condition::TTY_AND_COLOR);

    let Cli::Verify(config) = Cli::parse();

    let crates = translate_crates(config.charon_out_dir, config.cargo_build_args)?;

    if config.charon_pretty_print {
        for (crate_name, crate_) in &crates {
            println!("{:=^80}", format!(" {} ", crate_name).cyan().bold());
            println!("{crate_}");
        }
    }

    Ok(())
}

fn translate_crates(
    charon_out_dir: Option<PathBuf>,
    cargo_build_args: Vec<OsString>,
) -> Result<HashMap<String, TranslatedCrate>> {
    let mut crates = HashMap::new();

    let feature_args: Box<dyn Iterator<Item = OsString>> = {
        let cargo_manifest_dir = utils::cargo_manifest_dir()?;
        let cargo_manifest_dir = cargo_manifest_dir.into_os_string().into_string().unwrap();
        match proc_macro_crate::package_name("trust2-contract", cargo_manifest_dir) {
            Ok(FoundCrate::Name(name)) => {
                Box::new(["-F".into(), format!("{name}/verify").into()].into_iter())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parse_empty() {
        let Cli::Verify(config) = Cli::parse_from(["cargo", "verify"]);
        assert_eq!(config.charon_out_dir, None);
        assert!(!config.charon_pretty_print);
        assert_eq!(config.cargo_build_args, Vec::<OsString>::new());
    }

    #[test]
    fn cli_parse_empty_esc() {
        let Cli::Verify(config) = Cli::parse_from(["cargo", "verify", "--"]);
        assert_eq!(config.cargo_build_args, Vec::<OsString>::new());
    }

    #[test]
    fn cli_parse_charon_out_dir() {
        let Cli::Verify(config) =
            Cli::parse_from(["cargo", "verify", "--charon-out-dir", "Charon-LLBC"]);
        assert_eq!(config.charon_out_dir.unwrap(), PathBuf::from("Charon-LLBC"));
    }

    #[test]
    fn cli_parse_charon_pretty_print() {
        let Cli::Verify(config) = Cli::parse_from(["cargo", "verify", "--charon-pretty-print"]);
        assert!(config.charon_pretty_print);
    }

    #[test]
    fn cli_parse_cargo_build_args() {
        let Cli::Verify(config) = Cli::parse_from(["cargo", "verify", "--", "foo", "--bar"]);
        assert_eq!(config.cargo_build_args, ["foo", "--bar"]);
    }
}

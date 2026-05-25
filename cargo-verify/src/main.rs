use anyhow::{Context, Result};
use clap::Parser;
use clap_cargo::style::CLAP_STYLING;
use yansi::{Condition, Paint};

use std::{ffi::OsString, fs, path::PathBuf};

mod llbc_to_whyml;
mod rust_to_llbc;
mod whyml_verify;

#[derive(Parser)]
#[command(bin_name("cargo"), styles(CLAP_STYLING), help_expected(true))]
enum Cli {
    /// Verify contracts specified with trust2-contract package
    Verify(CliConfig),
}

#[derive(Parser)]
struct CliConfig {
    /// Why3 WhyML output directory. If not provided, a temporary directory is used and then deleted.
    #[arg(long)]
    why3_out_dir: Option<PathBuf>,
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

    let mut crates = run_with_dir(
        |charon_out_dir| rust_to_llbc::translate_crates(&charon_out_dir, config.cargo_build_args),
        config.charon_out_dir,
    )??;

    if config.charon_pretty_print {
        for (crate_name, crate_) in &crates {
            println!("{:=^80}", format!(" {crate_name} ").cyan().bold());
            println!("{crate_}");
        }
    }

    run_with_dir(
        |why3_out_dir| {
            llbc_to_whyml::translate_crates(&mut crates, &why3_out_dir)?;

            whyml_verify::verify(&why3_out_dir, &crates)
        },
        config.why3_out_dir,
    )?
}

fn run_with_dir<T>(f: impl FnOnce(PathBuf) -> T, dir: Option<PathBuf>) -> Result<T> {
    Ok(if let Some(dir) = dir {
        fs::create_dir_all(&dir)?;
        f(dir)
    } else {
        let temp_dir = tempfile::tempdir()?;
        let result = f(temp_dir.path().into());
        let err_msg = format!(
            "failed to delete temporary directory: {}",
            temp_dir.path().display(),
        );
        temp_dir.close().context(err_msg)?;
        result
    })
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

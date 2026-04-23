use anyhow::Result;
use clap::Parser;
use clap_cargo::style::CLAP_STYLING;
use yansi::{Condition, Paint};

use std::{ffi::OsString, path::PathBuf};

mod rust_to_llbc;

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

    let crates = rust_to_llbc::translate_crates(config.charon_out_dir, config.cargo_build_args)?;

    if config.charon_pretty_print {
        for (crate_name, crate_) in &crates {
            println!("{:=^80}", format!(" {crate_name} ").cyan().bold());
            println!("{crate_}");
        }
    }

    Ok(())
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

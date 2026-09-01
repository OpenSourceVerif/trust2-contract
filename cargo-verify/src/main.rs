use cargo_verify::{llbc_to_whyml, rust_to_llbc, whyml_verify};

use anyhow::{Context, Result};
use clap::Parser;
use clap_cargo::style::CLAP_STYLING;
use utils::yansi::STDOUT_IS_TTY_AND_COLOR;
use yansi::Paint;

use std::{ffi::OsString, fs, path::PathBuf};

#[derive(Parser)]
#[command(bin_name("cargo"), styles(CLAP_STYLING), help_expected(true))]
enum Cli {
    /// Verify contracts specified with trust2-contract package
    Verify(CliConfig),
}

#[derive(Parser)]
struct CliConfig {
    /// Provers to use. If not provided, Alt-Ergo, CVC5, and Z3 are used.
    #[arg(short = 'P', long = "prover", value_delimiter = ',')]
    provers: Vec<String>,
    /// Extra arguments for `why3find config`
    #[arg(
        long = "why3find-config-arg",
        value_name("WHY3FIND_CONFIG_ARG"),
        allow_hyphen_values = true
    )]
    why3find_config_args: Vec<OsString>,
    /// Extra arguments for `why3find prove`
    #[arg(
        long = "why3find-prove-arg",
        value_name("WHY3FIND_PROVE_ARG"),
        allow_hyphen_values = true
    )]
    why3find_prove_args: Vec<OsString>,
    /// Why3 output directory. If not provided, a temporary directory is used and then deleted.
    #[arg(long)]
    why3_out_dir: Option<PathBuf>,
    /// Charon LLBC output directory. If not provided, a temporary directory is used and then deleted.
    #[arg(long)]
    charon_out_dir: Option<PathBuf>,
    /// Pretty-print Charon LLBC.
    #[arg(long)]
    charon_pretty_print: bool,
    /// Extra arguments for `cargo build`
    #[arg(last(true), value_name("ARGS"))]
    cargo_build_args: Vec<OsString>,
}

fn main() -> Result<()> {
    let Cli::Verify(config) = Cli::parse();

    let mut crates = run_with_dir(
        |charon_out_dir| rust_to_llbc::translate_crates(&charon_out_dir, config.cargo_build_args),
        config.charon_out_dir,
    )??;

    if config.charon_pretty_print {
        for (crate_name, crate_) in &crates {
            println!(
                "{:=^80}",
                format!(" {crate_name} ")
                    .cyan()
                    .bold()
                    .whenever(STDOUT_IS_TTY_AND_COLOR),
            );
            println!("{crate_}");
        }
    }

    run_with_dir(
        |why3_out_dir| {
            llbc_to_whyml::translate_crates(&mut crates, &why3_out_dir)?;

            let provers = if config.provers.is_empty() {
                vec!["alt-ergo", "cvc5", "z3"]
            } else {
                config.provers.iter().map(String::as_str).collect()
            };

            whyml_verify::verify_crates(
                &why3_out_dir,
                &crates,
                &provers,
                &config.why3find_config_args,
                &config.why3find_prove_args,
            )
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
    fn cli_parse_prover_short() {
        let Cli::Verify(config) = Cli::parse_from(["cargo", "verify", "-P", "Z3"]);
        assert_eq!(config.provers, vec!["Z3"]);
    }

    #[test]
    fn cli_parse_prover_long() {
        let Cli::Verify(config) = Cli::parse_from([
            "cargo", "verify", "--prover", "Alt-Ergo", "--prover", "CVC5",
        ]);
        assert_eq!(config.provers, vec!["Alt-Ergo", "CVC5"]);
    }

    #[test]
    fn cli_parse_why3find_args_hyphen() {
        let Cli::Verify(config) = Cli::parse_from([
            "cargo",
            "verify",
            "--why3find-prove-arg",
            "-j",
            "--why3find-prove-arg",
            "8",
        ]);
        assert_eq!(config.why3find_prove_args, ["-j", "8"]);
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

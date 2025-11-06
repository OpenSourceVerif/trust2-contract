use anyhow::Result;
use clap::Parser;
use clap_cargo::style::CLAP_STYLING;
use proc_macro_crate::FoundCrate;

use std::{ffi::OsString, iter};

#[derive(Parser)]
#[command(bin_name("cargo"), styles(CLAP_STYLING), help_expected(true))]
enum Cli {
    /// Verify contracts specified with trust2-contract package
    Verify {
        /// `cargo build` arguments
        #[arg(last(true), value_name("ARGS"))]
        cargo_build_args: Vec<OsString>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let Cli::Verify { cargo_build_args } = cli;

    let cargo_manifest_dir = utils::cargo_manifest_dir()?;
    let cargo_manifest_dir = cargo_manifest_dir.into_os_string().into_string().unwrap();
    let feature_args: Box<dyn Iterator<Item = OsString>> =
        match proc_macro_crate::package_name("trust2-contract", cargo_manifest_dir) {
            Ok(FoundCrate::Name(name)) => {
                Box::new(["-F".into(), format!("{name}/verify").into()].into_iter())
            }
            _ => Box::new(iter::empty()),
        };

    // --monomorphize?
    // --no-ops-to-function-calls?
    // --raw-boxes?
    charon::main_(
        ["charon", "cargo", "--translate-all-methods", "--"]
            .into_iter()
            .map(OsString::from)
            .chain(cargo_build_args)
            .chain(feature_args),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parse_none() {
        let cli = Cli::parse_from(["cargo", "verify"]);
        let Cli::Verify { cargo_build_args } = cli;
        assert_eq!(cargo_build_args, Vec::<OsString>::new());
    }

    #[test]
    fn cli_parse_esc_none() {
        let cli = Cli::parse_from(["cargo", "verify", "--"]);
        let Cli::Verify { cargo_build_args } = cli;
        assert_eq!(cargo_build_args, Vec::<OsString>::new());
    }

    #[test]
    fn cli_parse_some() {
        let cli = Cli::parse_from(["cargo", "verify", "--", "foo", "--bar"]);
        let Cli::Verify { cargo_build_args } = cli;
        assert_eq!(cargo_build_args, ["foo", "--bar"]);
    }
}

//! If passed `cargo build` arguments select profile `foo`, `cargo verify` will use profile `foo-verify`.

#![feature(stmt_expr_attributes)]

use anyhow::Result;
use clap::Parser;
use clap_cargo::style::CLAP_STYLING;
use clap_lex::OsStrExt as _;
use itertools::Itertools;

use std::cell::OnceCell;
use std::ffi::OsString;
use std::ops::Range;

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

    // --monomorphize?
    // --no-ops-to-function-calls?
    // --raw-boxes?
    charon::main_(
        #[rustfmt::skip]
        [
            "charon", "cargo", "--translate-all-methods",
            "--",
            "-F", "trust2-contract/verify",
        ]
        .into_iter()
        .map(OsString::from)
        .chain(change_profile(cargo_build_args)),
    )
}

fn change_profile(mut args: Vec<OsString>) -> Box<dyn Iterator<Item = OsString>> {
    fn parse_profile(args: &[OsString]) -> Option<Option<(&str, Range<usize>)>> {
        let profile = OnceCell::new();

        for (i, arg) in args.iter().enumerate() {
            if arg == "-r" || arg == "--release" {
                profile.set(("release", i..i + 1)).ok()?;
            }
        }

        for (i, arg) in args.iter().enumerate() {
            const PREFIX: &str = "--profile=";
            if let Some(arg) = arg.strip_prefix(PREFIX) {
                profile.set((arg.to_str()?, i..i + 1)).ok()?;
            }
        }
        for (i, (arg0, arg1)) in args.iter().tuple_windows().enumerate() {
            if arg0 == "--profile" {
                profile.set((arg1.to_str()?, i..i + 2)).ok()?;
            }
        }

        Some(profile.into_inner())
    }

    let Some(profile) = parse_profile(&args) else {
        return Box::new(args.into_iter());
    };

    let (profile, args_without_profile): (_, Box<dyn Iterator<Item = OsString>>) = match profile {
        Some((profile, position)) => {
            let profile = profile.to_owned();
            let args_after_profile = args.split_off(position.end);
            args.truncate(position.start);
            (
                profile,
                Box::new(args.into_iter().chain(args_after_profile)),
            )
        }
        None => ("dev".into(), Box::new(args.into_iter())),
    };
    let verify_profile = format!("{profile}-verify");
    Box::new(args_without_profile.chain(
        #[rustfmt::skip]
        [
            "--config".to_owned(), format!(r#"profile.verify.inherits = "{verify_profile}""#),
            "--profile".to_owned(), verify_profile,
        ]
        .into_iter()
        .map(OsString::from),
    ))
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
    fn cli_parse() {
        let cli = Cli::parse_from(["cargo", "verify", "--", "foo", "--bar"]);
        let Cli::Verify { cargo_build_args } = cli;
        assert_eq!(cargo_build_args, ["foo", "--bar"]);
    }

    #[test]
    fn change_profile_invalid_0() {
        let args: Vec<OsString> = vec!["-r".into(), "--release".into()];
        let args: Vec<_> = change_profile(args).collect();
        assert_eq!(args, ["-r", "--release"]);
    }

    #[test]
    fn change_profile_invalid_1() {
        let args: Vec<OsString> = vec!["-r".into(), "--profile=custom".into()];
        let args: Vec<_> = change_profile(args).collect();
        assert_eq!(args, ["-r", "--profile=custom"]);
    }

    #[test]
    fn change_profile_none() {
        let args: Vec<OsString> = vec!["-p".into(), "cargo-verify".into()];
        let args: Vec<_> = change_profile(args).collect();
        assert_eq!(
            args,
            [
                "-p",
                "cargo-verify",
                "--config",
                r#"profile.verify.inherits = "dev-verify""#,
                "--profile",
                "dev-verify",
            ],
        );
    }

    #[test]
    fn change_profile_eq() {
        let args: Vec<OsString> = vec!["--profile=custom".into(), "--foo".into()];
        let args: Vec<_> = change_profile(args).collect();
        assert_eq!(
            args,
            [
                "--foo",
                "--config",
                r#"profile.verify.inherits = "custom-verify""#,
                "--profile",
                "custom-verify",
            ],
        );
    }

    #[test]
    fn change_profile_delim() {
        let args: Vec<OsString> = vec!["--foo".into(), "--profile".into(), "custom".into()];
        let args: Vec<_> = change_profile(args).collect();
        assert_eq!(
            args,
            [
                "--foo",
                "--config",
                r#"profile.verify.inherits = "custom-verify""#,
                "--profile",
                "custom-verify",
            ],
        );
    }
}

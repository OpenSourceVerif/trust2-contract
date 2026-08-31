use anyhow::{Result, anyhow, bail};
use charon_lib::ast::TranslatedCrate;
use utils::yansi::STDERR_IS_TTY_AND_COLOR;
use yansi::Paint;

use std::{
    collections::HashMap,
    io::{self, Write},
    path::Path,
    process::Command,
    time::Instant,
};

pub fn verify_crates(
    why3_out_dir: &Path,
    crates: &HashMap<String, TranslatedCrate>,
    provers: &[&str],
) -> Result<()> {
    let why3find_config_output = Command::new("why3find")
        .args([
            "config".as_ref(),
            "--root".as_ref(),
            why3_out_dir.as_os_str(),
            "--prover".as_ref(),
            provers.join(",").as_ref(),
        ])
        .output()?;
    if !why3find_config_output.status.success() {
        io::stdout().write_all(&why3find_config_output.stdout)?;
        io::stderr().write_all(&why3find_config_output.stderr)?;
        bail!("`why3find config` failed");
    }
    for crate_name in crates.keys() {
        verify_crate(why3_out_dir, crate_name)?;
    }
    Ok(())
}

fn verify_crate(why3_out_dir: &Path, crate_name: &String) -> Result<()> {
    let now = Instant::now();
    let why3find_prove_exit_status = Command::new("why3find")
        .args([
            "prove".as_ref(),
            "--root".as_ref(),
            why3_out_dir.as_os_str(),
            format!("{crate_name}.mlw").as_ref(),
        ])
        .stdout(io::stderr())
        .status()?;
    if why3find_prove_exit_status.success() {
        eprintln!(
            "{:>12} {crate_name} in {}",
            "Verified".green().bold().whenever(STDERR_IS_TTY_AND_COLOR),
            utils::elapsed(now.elapsed()),
        );
        Ok(())
    } else {
        Err(anyhow!("failed to verify crate `{crate_name}`"))
    }
}

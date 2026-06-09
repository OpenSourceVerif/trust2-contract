use anyhow::{Result, bail};
use charon_lib::ast::TranslatedCrate;
use yansi::Paint;

use std::{collections::HashMap, path::Path, process::Command};

pub fn verify(why3_out_dir: &Path, crates: &HashMap<String, TranslatedCrate>) -> Result<()> {
    for crate_name in crates.keys() {
        let whyml_path = why3_out_dir.join(format!("{crate_name}.mlw"));
        let mut prove_cmd = Command::new("why3");
        prove_cmd.args([
            "prove".as_ref(),
            "--prover".as_ref(),
            "cvc5".as_ref(),
            "--library".as_ref(),
            why3_out_dir.as_os_str(),
            "--extra-config".as_ref(),
            why3_out_dir.join("whyml_lib/ext.conf").as_os_str(),
            whyml_path.as_os_str(),
        ]);
        if prove_cmd.status()?.success() {
            eprintln!("{:>12} {crate_name}", "Verified".green().bold());
        } else {
            bail!("failed to verify crate `{crate_name}`");
        }
    }
    Ok(())
}

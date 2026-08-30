use anyhow::{Result, anyhow};
use charon_lib::ast::TranslatedCrate;
use utils::yansi::STDERR_IS_TTY_AND_COLOR;
use yansi::Paint;

use std::{
    collections::HashMap,
    io,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub fn verify_crates(
    why3_out_dir: &Path,
    crates: &HashMap<String, TranslatedCrate>,
    provers: &[String],
) -> Result<()> {
    for crate_name in crates.keys() {
        verify_crate(why3_out_dir, crate_name, provers)?;
    }
    Ok(())
}

fn verify_crate(why3_out_dir: &Path, crate_name: &String, provers: &[String]) -> Result<()> {
    let now = Instant::now();

    let whyml_path = why3_out_dir.join(format!("{crate_name}.mlw"));
    let mut child_processes: Vec<_> = provers
        .iter()
        .map(|prover| {
            Command::new("why3")
                .args([
                    "prove".as_ref(),
                    // "--color".as_ref(),
                    "--library".as_ref(),
                    why3_out_dir.as_os_str(),
                    "--prover".as_ref(),
                    prover.as_ref(),
                    whyml_path.as_os_str(),
                ])
                .stdout(Stdio::piped())
                .spawn()
        })
        .collect::<Result<_, _>>()?;
    let index = 'poll: loop {
        for (index, child_process) in child_processes.iter_mut().enumerate() {
            if child_process.try_wait().unwrap().is_some() {
                break 'poll index;
            }
        }
        thread::sleep(Duration::from_millis(50));
    };
    for child_process in &mut child_processes {
        child_process.kill().unwrap();
        child_process.wait().unwrap();
    }
    let child_process = &mut child_processes[index];

    io::copy(
        child_process.stdout.as_mut().unwrap(),
        &mut io::stderr().lock(),
    )
    .unwrap();
    if child_process.wait().unwrap().success() {
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

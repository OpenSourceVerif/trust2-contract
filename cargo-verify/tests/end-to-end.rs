use libtest_mimic::{Arguments, Failed, Trial};
use utils::why3find_proof::ProofFile;

use std::{
    env,
    fs::{self, File},
    io::{self, Write},
    path::PathBuf,
    process::{Command, ExitCode, Stdio},
};

fn main() -> Result<ExitCode, io::Error> {
    if which::which("why3find").is_err() {
        eprintln!("why3find is nonexistent or not executable");
        return Ok(ExitCode::FAILURE);
    }

    let mut args = Arguments::from_args();
    args.test_threads.get_or_insert(1);

    let mut tests = Vec::new();
    for dir_entry in fs::read_dir("tests/end-to-end")? {
        let dir_entry = dir_entry?;
        if dir_entry.file_type()?.is_dir() {
            let name = dir_entry.file_name();
            let path = dir_entry.path();
            tests.push(Trial::test(name.into_string().unwrap(), || run_test(path)));
        }
    }

    Ok(libtest_mimic::run(&args, tests).exit_code())
}

fn run_test(root_path: PathBuf) -> Result<(), Failed> {
    let mut result_file = File::create(root_path.join("result.txt"))?;
    cargo_verify::run_with_temp_dir(|out_dir| {
        let exit_status = Command::new(env!("CARGO_BIN_EXE_cargo-verify"))
            .current_dir(&root_path)
            .arg("verify")
            .args([
                "--why3-out-dir".as_ref(),
                out_dir.as_os_str(),
                "--why3find-config-arg".as_ref(),
                "--autoload".as_ref(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        writeln!(result_file, "exit status: {exit_status:?}")?;
        writeln!(result_file)?;

        for dir_entry in fs::read_dir(out_dir)? {
            let dir_entry = dir_entry?;
            let crate_mlw_path = dir_entry.path();
            if crate_mlw_path
                .extension()
                .is_some_and(|extension| extension == "mlw")
            {
                let crate_name = crate_mlw_path.file_stem().unwrap().to_str().unwrap();
                writeln!(result_file, "[{crate_name}]")?;
                let proof_path = out_dir.join(format!("why3find.proofs/{crate_name}.json"));
                let proof_string = fs::read_to_string(proof_path)?;
                let proof_file: ProofFile = serde_json::from_str(&proof_string)?;
                for (&goal, proof_cert) in &proof_file.proofs["Top"] {
                    writeln!(
                        result_file,
                        "{goal}: {}",
                        if proof_cert.is_complete() {
                            "✅"
                        } else {
                            "❌"
                        },
                    )?;
                }
                writeln!(result_file)?;
            }
        }

        Ok(())
    })?
}

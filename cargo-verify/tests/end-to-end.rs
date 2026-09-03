use libtest_mimic::{Arguments, Failed, Trial};
use utils::why3find_proof::ProofFile;

use std::{
    env,
    ffi::OsStr,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
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
    let result_path = root_path.join("result.txt");
    if !(option_env!("CI") == Some("true")) {
        let result_file = File::create(result_path)?;
        generate_result(&root_path, result_file)
    } else {
        let expected = fs::read(result_path)?;
        let mut actual = Vec::new();
        generate_result(&root_path, &mut actual)?;
        similar_asserts::assert_eq!(expected: expected, actual: actual);
        Ok(())
    }
}

fn generate_result(root_path: &Path, mut writer: impl io::Write) -> Result<(), Failed> {
    cargo_verify::run_with_temp_dir(|out_dir| {
        let exit_status = Command::new(env!("CARGO_BIN_EXE_cargo-verify"))
            .current_dir(root_path)
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
        writeln!(writer, "exit status: {exit_status:?}")?;
        writeln!(writer)?;

        for dir_entry in fs::read_dir(out_dir)? {
            let dir_entry = dir_entry?;
            let crate_mlw_path = dir_entry.path();
            if crate_mlw_path.extension() == Some(OsStr::new("mlw")) {
                let crate_name = crate_mlw_path.file_stem().unwrap().to_str().unwrap();
                writeln!(writer, "[{crate_name}]")?;
                let proof_path = out_dir.join(format!("why3find.proofs/{crate_name}.json"));
                let proof_string = fs::read_to_string(proof_path)?;
                let proof_file: ProofFile = serde_json::from_str(&proof_string)?;
                for (&goal, proof_cert) in &proof_file.proofs["Top"] {
                    writeln!(
                        writer,
                        "{goal}: {}",
                        if proof_cert.is_complete() {
                            "✅"
                        } else {
                            "❌"
                        },
                    )?;
                }
                writeln!(writer)?;
            }
        }

        Ok(())
    })?
}

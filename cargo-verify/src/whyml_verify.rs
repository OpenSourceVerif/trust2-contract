use anyhow::{Result, anyhow, bail};
use charon_lib::ast::TranslatedCrate;
use utils::{
    why3find_proof::{ProofCertificate, ProofFile},
    yansi::STDERR_IS_TTY_AND_COLOR,
};
use yansi::Paint;

use std::{
    collections::HashMap,
    ffi::OsString,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

pub fn verify_crates(
    why3_out_dir: &Path,
    crates: &HashMap<String, TranslatedCrate>,
    provers: &[&str],
    why3find_config_args: &[OsString],
    why3find_prove_args: &[OsString],
) -> Result<()> {
    let why3find_config_output = Command::new("why3find")
        .arg("config")
        .args([
            "--root".as_ref(),
            why3_out_dir.as_os_str(),
            "--prover".as_ref(),
            provers.join(",").as_ref(),
        ])
        .args(why3find_config_args)
        .output()?;
    if !why3find_config_output.status.success() {
        io::stdout().write_all(&why3find_config_output.stdout)?;
        io::stderr().write_all(&why3find_config_output.stderr)?;
        bail!("`why3find config` failed");
    }
    for crate_name in crates.keys() {
        verify_crate(why3_out_dir, crate_name, why3find_prove_args)?;
    }
    Ok(())
}

fn verify_crate(
    why3_out_dir: &Path,
    crate_name: &String,
    why3find_prove_args: &[OsString],
) -> Result<()> {
    let now = Instant::now();
    // 先记下证明记录文件此刻的修改时间：why3find 如果在「证明」开始前就退出
    // （例如生成的 WhyML 有类型错误），并不会更新该文件；此时读到的是上一轮
    // 跑出来的旧记录，直接打印会让人误以为这次的证明结果和耗时。
    let proof_mtime_before = proof_file_path(why3_out_dir, crate_name)
        .and_then(|path| fs::metadata(path).ok())
        .and_then(|metadata| metadata.modified().ok());
    let why3find_prove_exit_status = Command::new("why3find")
        .arg("prove")
        .args([
            "--root".as_ref(),
            why3_out_dir.as_os_str(),
            format!("{crate_name}.mlw").as_ref(),
        ])
        .args(why3find_prove_args)
        .stdout(io::stderr())
        .status()?;
    // 是否存在新记录由验证流程决定；打印函数只处理已经读取的证明数据。
    if let Some(proof_path) = proof_file_path(why3_out_dir, crate_name) {
        if fs::metadata(&proof_path)?.modified().ok() == proof_mtime_before {
            eprintln!(
                "{:>12} no updated proof record for `{crate_name}` (why3find exited before proving)",
                "Skipped".yellow().bold().whenever(STDERR_IS_TTY_AND_COLOR),
            );
        } else {
            let proof_string = fs::read_to_string(&proof_path)?;
            let proof_file: ProofFile = serde_json::from_str(&proof_string)?;
            print_proofs(crate_name, &proof_file);
        }
    }
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

/// 批量打印已读取的证明目标、证明器与耗时。
///
/// 即使本次验证失败（存在未能证明的目标）也会打印，便于定位是哪个目标失败。
/// 调用方负责确认记录来自本次验证。
fn print_proofs(crate_name: &str, proof_file: &ProofFile<'_>) {
    for (theory, goals) in &proof_file.proofs {
        for (goal, certificate) in goals {
            print_certificate(&format!("{crate_name}.{theory}.{goal}"), certificate);
        }
    }
}

/// why3find 写出的证明记录文件的位置。
///
/// why3find 默认把每个 crate 的证明记录写在 `<why3-out-dir>/<crate>/proof.json`；
/// 若传入了改变记录方式的参数，则可能汇总在 `<why3-out-dir>/why3find.proofs/<crate>.json`。
fn proof_file_path(why3_out_dir: &Path, crate_name: &str) -> Option<PathBuf> {
    [
        why3_out_dir.join(crate_name).join("proof.json"),
        why3_out_dir
            .join("why3find.proofs")
            .join(format!("{crate_name}.json")),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

/// 打印单个证明目标的结果：已证明的目标给出证明器与耗时。
fn print_certificate(goal: &str, certificate: &ProofCertificate) {
    match certificate {
        ProofCertificate::Proven { prover, time } => {
            eprintln!(
                "{:>12} {goal} with {prover} in {}",
                "Proved".green().bold().whenever(STDERR_IS_TTY_AND_COLOR),
                utils::elapsed(Duration::from_secs_f64(*time)),
            );
        }
        ProofCertificate::Stuck => {
            eprintln!(
                "{:>12} {goal}",
                "Stuck".red().bold().whenever(STDERR_IS_TTY_AND_COLOR),
            );
        }
        ProofCertificate::Tactic { tactic, children } => {
            // 战术（如 split_vc）会把目标拆成若干子目标，逐个打印
            eprintln!(
                "{:>12} {goal} ({tactic})",
                "Tactic".magenta().bold().whenever(STDERR_IS_TTY_AND_COLOR),
            );
            for child in children {
                print_certificate(goal, child);
            }
        }
    }
}

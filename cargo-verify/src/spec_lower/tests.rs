use crate::spec_ast::Spec as PSpec;

use super::{lower_crate_specs, name_to_string};
use anyhow::{Context, Result, anyhow, bail};
use charon_lib::ast::*;

use std::{collections::BTreeMap, path::PathBuf, process::Command, sync::LazyLock};

static SAMPLE_CRATE: LazyLock<Result<TranslatedCrate, String>> =
    LazyLock::new(|| generate_sample_crate().map_err(|err| format!("{err:#}")));

fn sample_crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../charon/charon/tests/cargo/trust2-contract-sample")
}

fn charon_manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../charon/charon/Cargo.toml")
}

fn ensure_charon_bins_built() -> Result<PathBuf> {
    let manifest_path = charon_manifest_path();
    let output = Command::new("cargo")
        .arg("build")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--bins")
        .output()
        .context("failed to spawn cargo build for charon binaries")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "failed to build charon binaries with `cargo build --quiet --manifest-path {} --bins`:\n{}",
            manifest_path.display(),
            stderr.trim()
        );
    }

    let charon_bin = manifest_path
        .parent()
        .expect("charon manifest path must have a parent")
        .join("target/debug/charon");
    if !charon_bin.is_file() {
        bail!("expected charon binary at {}", charon_bin.display());
    }
    Ok(charon_bin)
}

fn generate_sample_crate() -> Result<TranslatedCrate> {
    let temp_dir = tempfile::tempdir().context("failed to create temporary fixture directory")?;
    let llbc_path = temp_dir.path().join("trust2-contract-sample.llbc");
    let manifest_path = sample_crate_dir().join("Cargo.toml");
    let target_dir = temp_dir.path().join("target");
    let charon_bin = ensure_charon_bins_built()?;
    let output = Command::new(&charon_bin)
        .arg("cargo")
        .arg("--dest-file")
        .arg(&llbc_path)
        .arg("--")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--target-dir")
        .arg(&target_dir)
        .arg("--features=trust2-contract/verify")
        .output()
        .context("failed to spawn charon for fixture generation")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "failed to generate trust2-contract sample fixture with `{}`:\n{}",
            format!(
                "{} -- cargo --dest-file {} -- --manifest-path {} --target-dir {} --features=trust2-contract/verify",
                charon_bin.display(),
                llbc_path.display(),
                manifest_path.display(),
                target_dir.display()
            ),
            stderr.trim()
        );
    }
    charon_lib::deserialize_llbc(&llbc_path).with_context(|| {
        format!(
            "failed to deserialize generated fixture {}",
            llbc_path.display()
        )
    })
}

fn load_sample_crate() -> Result<&'static TranslatedCrate> {
    match &*SAMPLE_CRATE {
        Ok(krate) => Ok(krate),
        Err(err) => Err(anyhow!("{err}")),
    }
}

fn find_fun_id_by_name(krate: &TranslatedCrate, full_name: &str) -> FunDeclId {
    krate
        .fun_decls
        .iter_indexed_values()
        .find_map(|(fun_id, decl)| {
            (name_to_string(&decl.item_meta.name) == full_name).then_some(fun_id)
        })
        .unwrap_or_else(|| panic!("missing function fixture: {full_name}"))
}

fn lowered_spec<'a>(lowered: &'a BTreeMap<FunDeclId, PSpec>, fun_id: FunDeclId) -> &'a PSpec {
    lowered
        .get(&fun_id)
        .unwrap_or_else(|| panic!("missing lowered spec for function id {}", fun_id.index()))
}

#[test]
fn lowers_max_fixture_pre_and_multiple_posts() -> Result<()> {
    let krate = load_sample_crate()?;
    let lowered = lower_crate_specs(krate).map_err(anyhow::Error::new)?;
    let fun_id = find_fun_id_by_name(krate, "trust2_contract_sample::max");
    let spec = lowered_spec(&lowered, fun_id);

    assert_eq!(
        spec.to_string(),
        "\
pre:
  - true
post:
  - result => (result >= b)
  - result => (result >= a)
"
    );
    Ok(())
}

#[test]
fn lowers_min_fixture_if_expression() -> Result<()> {
    let krate = load_sample_crate()?;
    let lowered = lower_crate_specs(krate).map_err(anyhow::Error::new)?;
    let fun_id = find_fun_id_by_name(krate, "trust2_contract_sample::min");
    let spec = lowered_spec(&lowered, fun_id);

    assert_eq!(
        spec.to_string(),
        "\
pre: []
post:
  - result => (if (result <= a) then (result <= b) else false)
"
    );
    Ok(())
}

#[test]
fn lowers_quantified_postcondition_from_fixture() -> Result<()> {
    let krate = load_sample_crate()?;
    let lowered = lower_crate_specs(krate).map_err(anyhow::Error::new)?;
    let fun_id = find_fun_id_by_name(krate, "trust2_contract_sample::to_sorted");
    let spec = lowered_spec(&lowered, fun_id);

    assert_eq!(
        spec.to_string(),
        "\
pre: []
post:
  - result => forall i. (((i + Unsigned(Usize, 1)) < core.slice.impl.len(a)) -> (alloc.vec.impl.index(result, i) <= alloc.vec.impl.index(result, (i + Unsigned(Usize, 1)))))
"
    );
    Ok(())
}

#[test]
fn lowers_nested_precondition_from_fixture() -> Result<()> {
    let krate = load_sample_crate()?;
    let lowered = lower_crate_specs(krate).map_err(anyhow::Error::new)?;
    let fun_id = find_fun_id_by_name(krate, "trust2_contract_sample::use_assert::decuple");
    let spec = lowered_spec(&lowered, fun_id);

    assert_eq!(
        spec.to_string(),
        "\
pre:
  - (x <= Unsigned(U8, 25))
post: []
"
    );
    Ok(())
}

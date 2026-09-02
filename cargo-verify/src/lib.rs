use anyhow::{Context, Result};
use std::{fs, path::Path};

pub mod llbc_to_whyml;
pub mod rust_to_llbc;
pub mod whyml_verify;

pub fn run_with_dir<T>(f: impl FnOnce(&Path) -> T, dir: Option<&Path>) -> Result<T> {
    if let Some(dir) = dir {
        fs::create_dir_all(dir)?;
        Ok(f(dir))
    } else {
        run_with_temp_dir(f)
    }
}

pub fn run_with_temp_dir<T>(f: impl FnOnce(&Path) -> T) -> Result<T> {
    let temp_dir = tempfile::tempdir()?;
    let result = f(temp_dir.path());
    let err_msg = format!(
        "failed to delete temporary directory: {}",
        temp_dir.path().display(),
    );
    temp_dir.close().context(err_msg)?;
    Ok(result)
}

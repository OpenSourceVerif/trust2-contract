#![feature(rustc_private)]

use charon_driver::CharonFailure;
use charon_lib::{
    options::CliOpts,
    transform::{TransformCtx, finish_transformation_passes, run_ullbc_passes},
};
use std::{env, path::PathBuf};

fn transform(options: &CliOpts, ctx: &mut TransformCtx) -> Result<(), CharonFailure> {
    run_ullbc_passes(options, ctx);
    let errors = ctx.errors.borrow().error_count;
    if errors != 0 {
        return Err(CharonFailure::CharonError(errors));
    }

    let output = env::var_os("ULLBC_PCG_OUTPUT")
        .map(PathBuf::from)
        .ok_or_else(|| CharonFailure::Analysis("missing ULLBC_PCG_OUTPUT directory".into()))?;
    let name = env::var("ULLBC_PCG_FUNCTION")
        .ok()
        .filter(|n| !n.is_empty());
    let krate = &ctx.translated;
    let function = ullbc_pcg::select_function(krate, name.as_deref())
        .map_err(|e| CharonFailure::Analysis(e.to_string()))?;
    let result = ullbc_pcg::analyze_function(krate, function)
        .map_err(|e| CharonFailure::Analysis(e.to_string()))?;
    ullbc_pcg::export::write_analysis(krate, &result, &output)
        .map_err(|e| CharonFailure::Analysis(e.to_string()))?;
    finish_transformation_passes(options, ctx);
    Ok(())
}

fn main() {
    charon_driver::main_with_transform(transform)
}

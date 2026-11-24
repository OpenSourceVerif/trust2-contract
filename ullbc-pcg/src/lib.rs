//! Independent CFG PCG construction over Charon ULLBC and adapted rustc NLL facts.
//! No rustc dependency or MIR-PCG results are needed by this crate. The initial
//! supported semantics are scalar/unit locals and non-escaping direct references.
//! Branch joins, cyclic PCG abstraction, calls and reborrows are explicitly rejected.
mod analysis;
mod borrowck;
pub mod export;
mod graph;
mod input;
mod liveness;
mod pcg;
pub use pcg::EvalStmtPhase;
mod results;
mod transfer;

use charon_lib::ullbc_ast as u;
pub use graph::*;
pub use results::*;

/// Propagates a fresh PCG over the reachable ULLBC CFG using matching compiler facts.
/// Returns no partial Analysis when the body or metadata is unsupported/invalid.
pub fn analyze(
    body: &u::ExprBody,
    facts: &charon_lib::ast::BodyBorrowckInfo,
) -> Result<Analysis, AnalysisError> {
    analysis::run(body, facts)
}

use charon_lib::{
    ast::{
        Body, BodyBorrowckInfo, BorrowckData, FINAL_ULLBC_BORROWCK_STAGE, FunDecl,
        PRE_STRUCTURE_BORROWCK_STAGE, TranslatedCrate,
    },
    formatter::IntoFormatter,
    pretty::FmtWithCtx,
};

/// Owned PCG result, identified independently of the Charon context or any viewer.
#[derive(serde::Serialize)]
pub struct FunctionAnalysis {
    pub function_id: charon_lib::ast::FunDeclId,
    pub analysis_stage: &'static str,
    pub body_fingerprint: String,
    #[serde(flatten)]
    pub analysis: Analysis,
}

struct FunctionInput<'a> {
    body: &'a u::ExprBody,
    facts: &'a BodyBorrowckInfo,
    bundle: &'a BorrowckData,
}

impl FunctionInput<'_> {
    fn analysis_stage(&self) -> &'static str {
        if self.bundle.stage == PRE_STRUCTURE_BORROWCK_STAGE {
            "before-control-flow-reconstruction"
        } else {
            "final-ullbc"
        }
    }
}

pub fn select_function<'a>(
    krate: &'a TranslatedCrate,
    function: Option<&str>,
) -> Result<&'a FunDecl, Box<dyn std::error::Error>> {
    let fmt = krate.into_fmt();
    let matches: Vec<_> = krate
        .fun_decls
        .iter()
        .filter(|f| matches!(&f.body, Body::Unstructured(_)))
        .filter(|f| {
            let name = f.item_meta.name.to_string_with_ctx(&fmt);
            function.is_none_or(|wanted| name == wanted || name.rsplit("::").next() == Some(wanted))
        })
        .collect();
    if matches.len() != 1 {
        let names: Vec<_> = krate
            .fun_decls
            .iter()
            .filter(|f| matches!(&f.body, Body::Unstructured(_)))
            .map(|f| f.item_meta.name.to_string_with_ctx(&fmt))
            .collect();
        return Err(format!(
            "expected one function matching {function:?}, found {}. Use --function NAME. Available functions: {}",
            matches.len(), names.join(", ")
        )
        .into());
    }
    Ok(matches[0])
}

pub fn analyze_function(
    krate: &TranslatedCrate,
    function: &FunDecl,
) -> Result<FunctionAnalysis, Box<dyn std::error::Error>> {
    let input = function_input(krate, function)?;
    let analysis = analyze(input.body, input.facts)?;
    Ok(FunctionAnalysis {
        function_id: function.def_id,
        analysis_stage: input.analysis_stage(),
        body_fingerprint: input.facts.body_fingerprint.clone(),
        analysis,
    })
}

fn function_input<'a>(
    krate: &'a TranslatedCrate,
    function: &'a FunDecl,
) -> Result<FunctionInput<'a>, Box<dyn std::error::Error>> {
    let Body::Unstructured(body) = &function.body else {
        return Err("expected an ULLBC body".into());
    };
    let bundle = krate.borrowck.as_ref().ok_or_else(|| {
        AnalysisError::new(
            ErrorKind::InvalidInput,
            "missing rustc borrow information; re-extract using Charon --export-borrowck",
        )
    })?;
    if bundle.schema_version != 1
        || !matches!(
            bundle.stage.as_str(),
            FINAL_ULLBC_BORROWCK_STAGE | PRE_STRUCTURE_BORROWCK_STAGE
        )
        || bundle.producer != "rustc-nll"
    {
        return Err(AnalysisError::new(
            ErrorKind::InvalidInput,
            "unsupported borrowck metadata schema/stage/producer",
        )
        .into());
    }
    let mut matching = bundle
        .bodies
        .iter()
        .filter(|b| b.function_id == function.def_id);
    let facts = matching.next().ok_or_else(|| {
        AnalysisError::new(ErrorKind::InvalidInput, "missing function borrowck data")
    })?;
    if matching.next().is_some() {
        return Err(AnalysisError::new(
            ErrorKind::InvalidInput,
            "duplicate function borrowck data",
        )
        .into());
    }
    Ok(FunctionInput {
        body,
        facts,
        bundle,
    })
}

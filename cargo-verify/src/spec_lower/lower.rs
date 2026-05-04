//! Top-level orchestration for scanning a crate and lowering all attached specs.

use crate::spec_ast::{Post as PPost, Spec as PSpec};
use charon_lib::ast::*;

use std::collections::BTreeMap;

use super::{
    builder::TermBuilder,
    closure::{ClosureRole, binders_to_pattern},
    errors::{SpecLowerError, SpecLowerErrors},
    naming::name_to_string,
};

/// Lower every structured function body carrying legacy `FunSpecs`.
pub fn lower_crate_specs(
    krate: &TranslatedCrate,
) -> Result<BTreeMap<FunDeclId, PSpec>, SpecLowerErrors> {
    let mut lowered_specs = BTreeMap::new();
    let mut errors = Vec::new();

    for (fun_id, fun_decl) in krate.fun_decls.iter_indexed_values() {
        let Some(body) = fun_decl.body.as_structured() else {
            continue;
        };
        if body.specs.preconditions.is_empty() && body.specs.postconditions.is_empty() {
            continue;
        }

        match lower_specs(krate, &name_to_string(&fun_decl.item_meta.name), body) {
            Ok(spec) => {
                lowered_specs.insert(fun_id, spec);
            }
            Err(mut fun_errors) => errors.append(&mut fun_errors),
        }
    }

    if errors.is_empty() {
        Ok(lowered_specs)
    } else {
        Err(SpecLowerErrors { errors })
    }
}

/// Lower all specs attached to one function body.
///
/// Each top-level pre/post block is lowered independently so the caller gets a
/// complete error list instead of stopping at the first failure in a function.
fn lower_specs(
    krate: &TranslatedCrate,
    function_name: &str,
    body: &llbc_ast::ExprBody,
) -> Result<PSpec, Vec<SpecLowerError>> {
    let mut pre = Vec::new();
    let mut post = Vec::new();
    let mut errors = Vec::new();

    for spec_block in &body.specs.preconditions {
        let builder = TermBuilder::new_for_function(
            krate,
            &body.locals,
            function_name.to_owned().into_boxed_str(),
            "precondition".into(),
        );
        match lower_top_level_spec_block(builder, spec_block, ClosureRole::Pre) {
            Ok(lowered) => {
                if !lowered.binders.is_empty() {
                    errors.push(SpecLowerError {
                        function_name: function_name.to_owned().into_boxed_str(),
                        spec_kind: "precondition".into(),
                        span: spec_block.call.span,
                        reason: "precondition closure must not bind parameters".into(),
                    });
                } else {
                    pre.push(lowered.term);
                }
            }
            Err(err) => errors.push(err),
        }
    }

    for spec_block in &body.specs.postconditions {
        let builder = TermBuilder::new_for_function(
            krate,
            &body.locals,
            function_name.to_owned().into_boxed_str(),
            "postcondition".into(),
        );
        match lower_top_level_spec_block(builder, spec_block, ClosureRole::Post) {
            Ok(lowered) => {
                post.push(PPost {
                    span: spec_block.call.span,
                    pattern: binders_to_pattern(spec_block.call.span, &lowered.binders),
                    term: lowered.term,
                });
            }
            Err(err) => errors.push(err),
        }
    }

    if errors.is_empty() {
        Ok(PSpec { pre, post })
    } else {
        Err(errors)
    }
}

/// Lower one top-level `requires` or `ensures` closure invocation.
fn lower_top_level_spec_block(
    mut builder: TermBuilder<'_>,
    spec_block: &llbc_ast::FunSpecBlock,
    role: ClosureRole,
) -> Result<super::closure::LoweredClosure, SpecLowerError> {
    builder.eval_statements(&spec_block.statements)?;
    let Some(closure_operand) = spec_block.call.args.first() else {
        return builder.error(
            spec_block.call.span,
            "empty spec call argument list".to_owned(),
        );
    };
    let closure = builder.eval_operand_as_closure(closure_operand, spec_block.call.span)?;
    builder.lower_closure_value(closure, role, spec_block.call.span)
}

//! Helpers for interpreting closure-encoded specs in Charon LLBC.

use crate::spec_ast::{
    Binder as PBinder, Ident as PIdent, Pattern as PPattern, PatternDesc as PPatternDesc,
    Term as PTerm,
};
use charon_lib::ast::*;

use super::{
    errors::SpecLowerError,
    naming::{name_to_string, sanitize_local_name},
};

/// Role played by a lowered closure inside the surrounding specification.
#[derive(Debug, Clone, Copy)]
pub(super) enum ClosureRole {
    /// Top-level precondition closure.
    Pre,
    /// Top-level postcondition closure.
    Post,
    /// Quantified `forall` helper closure.
    Forall,
    /// Quantified `exists` helper closure.
    Exists,
    /// Nested assertion helper closure.
    Assert,
    /// Nested assumption helper closure.
    Assume,
}

impl ClosureRole {
    /// Return the role label used in error messages.
    pub(super) fn label(self) -> &'static str {
        match self {
            ClosureRole::Pre => "precondition",
            ClosureRole::Post => "postcondition",
            ClosureRole::Forall => "forall",
            ClosureRole::Exists => "exists",
            ClosureRole::Assert => "assert",
            ClosureRole::Assume => "assume",
        }
    }
}

/// Closure aggregate reconstructed from LLBC with its captured values lowered.
#[derive(Clone)]
pub(super) struct ClosureValue {
    /// Charon type id of the closure ADT.
    pub(super) type_id: TypeDeclId,
    /// Captured environment values in field order.
    pub(super) captures: Vec<PTerm>,
}

/// Result of lowering a callable closure body.
pub(super) struct LoweredClosure {
    /// Binders introduced by the closure argument tuple.
    pub(super) binders: Vec<PBinder>,
    /// Lowered body term returned by the closure.
    pub(super) term: PTerm,
}

/// Derive binders for the tuple argument passed into a closure call shim.
///
/// Charon lowers closure calls as methods that receive the environment and a
/// tuple of explicit parameters. This function rehydrates those tuple elements
/// into named binders for the spec AST.
pub(super) fn derive_closure_binders(
    body: &llbc_ast::ExprBody,
    signature: &FunSig,
    role: ClosureRole,
    span: Span,
) -> Vec<PBinder> {
    let Some(args_ty) = signature.inputs.get(1) else {
        return Vec::new();
    };
    let Some(args_tys) = args_ty.as_tuple() else {
        return Vec::new();
    };

    let binder_count = args_tys.iter().count();
    args_tys
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            let preferred_name = if matches!(role, ClosureRole::Post) && binder_count == 1 {
                "result".to_owned()
            } else {
                body.locals
                    .locals
                    .get(LocalId::new(index + 3))
                    .map(|local| sanitize_local_name(local.name.as_deref(), local.index))
                    .unwrap_or_else(|| format!("arg{index}"))
            };
            PBinder {
                span,
                id: Some(PIdent::new(preferred_name, span)),
                ghost: false,
                ty: Some(ty.clone()),
            }
        })
        .collect()
}

/// Resolve the concrete closure call shim that contains the executable body.
///
/// Charon may wire closures through `Fn`, `FnMut`, or `FnOnce`; this helper
/// searches the available trait impls in that order and returns the first
/// callable method body.
pub(super) fn resolve_closure_call_fun_id(
    krate: &TranslatedCrate,
    type_id: TypeDeclId,
    function_name: &str,
    spec_kind: &str,
    span: Span,
) -> Result<FunDeclId, SpecLowerError> {
    let Some(type_decl) = krate.type_decls.get(type_id) else {
        return Err(SpecLowerError {
            function_name: function_name.to_owned().into_boxed_str(),
            spec_kind: spec_kind.to_owned().into_boxed_str(),
            span,
            reason: format!("unknown closure type id: {}", type_id.index()).into_boxed_str(),
        });
    };
    let ItemSource::Closure { info } = &type_decl.src else {
        return Err(SpecLowerError {
            function_name: function_name.to_owned().into_boxed_str(),
            spec_kind: spec_kind.to_owned().into_boxed_str(),
            span,
            reason: "spec closure value did not resolve to a closure type".into(),
        });
    };

    let candidates = [
        ("call", info.fn_impl.as_ref().map(|r| r.skip_binder.id)),
        (
            "call_mut",
            info.fn_mut_impl.as_ref().map(|r| r.skip_binder.id),
        ),
        ("call_once", Some(info.fn_once_impl.skip_binder.id)),
    ];
    for (method_name, trait_impl_id) in candidates {
        let Some(trait_impl_id) = trait_impl_id else {
            continue;
        };
        let Some(trait_impl) = krate.trait_impls.get(trait_impl_id) else {
            continue;
        };
        if let Some((_, method_ref)) = trait_impl
            .methods
            .iter()
            .find(|(name, _)| name.0.as_str() == method_name)
        {
            return Ok(method_ref.skip_binder.id);
        }
    }
    Err(SpecLowerError {
        function_name: function_name.to_owned().into_boxed_str(),
        spec_kind: spec_kind.to_owned().into_boxed_str(),
        span,
        reason: format!(
            "failed to resolve callable closure body for type `{}`",
            name_to_string(&type_decl.item_meta.name)
        )
        .into_boxed_str(),
    })
}

/// Convert postcondition binders into the pattern shape expected by `PPost`.
pub(super) fn binders_to_pattern(span: Span, binders: &[PBinder]) -> PPattern {
    match binders {
        [] => PPattern {
            span,
            desc: PPatternDesc::Wild,
        },
        [binder] => {
            if let Some(id) = &binder.id {
                PPattern {
                    span,
                    desc: PPatternDesc::Var(id.clone()),
                }
            } else {
                PPattern {
                    span,
                    desc: PPatternDesc::Wild,
                }
            }
        }
        _ => PPattern {
            span,
            desc: PPatternDesc::Tuple(
                binders
                    .iter()
                    .map(|binder| PPattern {
                        span,
                        desc: binder
                            .id
                            .as_ref()
                            .map(|id| PPatternDesc::Var(id.clone()))
                            .unwrap_or(PPatternDesc::Wild),
                    })
                    .collect(),
            ),
        },
    }
}

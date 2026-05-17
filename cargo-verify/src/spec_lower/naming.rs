//! Shared translation helpers for names, locals, closure detection, and operators.

use crate::spec_ast::{
    BinOp as PBinOp, Ident as PIdent, Qualid as PQualid, Term as PTerm, TermDesc as PTermDesc,
};
use charon_lib::ast::*;

/// Render a Charon name as a `::`-separated path.
pub fn name_to_string(name: &Name) -> String {
    name_path_segments(name).join("::")
}

/// Convert a Charon path into the nested `Qualid` representation used by `spec_ast`.
pub(super) fn name_to_qualid(name: &Name, span: Span) -> PQualid {
    let mut segments = name_path_segments(name).into_iter();
    let Some(first) = segments.next() else {
        return PQualid::ident("<anon>", span);
    };
    let mut qid = PQualid::Ident(PIdent::new(first, span));
    for segment in segments {
        qid = PQualid::Dot(Box::new(qid), PIdent::new(segment, span));
    }
    qid
}

/// Extract normalized path segments from a Charon `Name`.
///
/// Synthetic path elements are rendered with stable placeholder segments so the
/// lowered output remains readable even when Charon does not expose a plain identifier.
fn name_path_segments(name: &Name) -> Vec<String> {
    name.name
        .iter()
        .map(|elem| match elem {
            PathElem::Ident(seg, _) => seg.clone(),
            PathElem::Impl(_) => "impl".to_owned(),
            PathElem::Instantiated(_) => "inst".to_owned(),
            PathElem::Target(target) => target.clone(),
        })
        .collect()
}

/// Check whether a type id refers to a closure ADT produced by Charon.
pub(super) fn is_closure_type(krate: &TranslatedCrate, type_id: TypeId) -> bool {
    let TypeId::Adt(type_id) = type_id else {
        return false;
    };
    krate
        .type_decls
        .get(type_id)
        .map(|decl| matches!(decl.src, ItemSource::Closure { .. }))
        .unwrap_or(false)
}

/// Build a plain identifier term used for locals and binder references.
pub(super) fn local_ident_term(span: Span, name: &str) -> PTerm {
    PTerm::new(
        span,
        PTermDesc::Ident(PQualid::Ident(PIdent::new(name, span))),
    )
}

/// Normalize Charon local names into stable user-facing identifiers.
///
/// Charon often suffixes locals with `_N`; stripping that suffix keeps the
/// rendered spec close to the original source names.
pub(super) fn sanitize_local_name(name: Option<&str>, local: LocalId) -> String {
    let base = name
        .map(str::to_owned)
        .unwrap_or_else(|| format!("_{}", local.index()));
    if let Some((prefix, suffix)) = base.rsplit_once('_')
        && !prefix.is_empty()
        && suffix.chars().all(|c| c.is_ascii_digit())
    {
        return prefix.to_owned();
    }
    base
}

/// Translate LLBC binary operators to the corresponding `spec_ast` operator.
///
/// Operators without a direct Why3-style encoding in the current lowering are
/// left as `None` so callers can emit a structured error.
pub(super) fn map_binop(binop: BinOp) -> Option<PBinOp> {
    match binop {
        BinOp::Eq => Some(PBinOp::Eq),
        BinOp::Ne => Some(PBinOp::Ne),
        BinOp::Lt => Some(PBinOp::Lt),
        BinOp::Le => Some(PBinOp::Le),
        BinOp::Gt => Some(PBinOp::Gt),
        BinOp::Ge => Some(PBinOp::Ge),
        BinOp::Add(_) => Some(PBinOp::Add),
        BinOp::Sub(_) => Some(PBinOp::Sub),
        BinOp::Mul(_) => Some(PBinOp::Mul),
        BinOp::Div(_) => Some(PBinOp::Div),
        BinOp::Rem(_) => Some(PBinOp::Rem),
        BinOp::BitAnd => Some(PBinOp::And),
        BinOp::BitOr => Some(PBinOp::Or),
        _ => None,
    }
}

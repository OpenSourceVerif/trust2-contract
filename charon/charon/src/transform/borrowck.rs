//! Borrow facts follow explicit transformations of final ULLBC. This sidecar
//! never changes a body or decides whether a normal Charon pass runs.
use crate::{ast::*, transform::TransformCtx, ullbc_ast as u};

pub(crate) fn take(ctx: &mut TransformCtx, body: &u::ExprBody) -> Option<BodyBorrowckInfo> {
    let ItemId::Fun(id) = ctx.errors.borrow().def_id? else {
        return None;
    };
    let bodies = &mut ctx.translated.borrowck.as_mut()?.bodies;
    let index = bodies.iter().position(|b| b.function_id == id)?;
    let mut info = bodies.remove(index);
    if info.body_fingerprint != borrowck_body_fingerprint(body) {
        info.unsupported
            .get_or_insert("borrowck: body changed without adapting its facts".into());
    }
    Some(info)
}

pub(crate) fn put(ctx: &mut TransformCtx, body: &u::ExprBody, info: Option<BodyBorrowckInfo>) {
    if let Some(mut info) = info {
        info.body_fingerprint = borrowck_body_fingerprint(body);
        info.points.sort_by_key(|p| p.location);
        ctx.translated.borrowck.as_mut().unwrap().bodies.push(info);
    }
}

/// Insert operations with no loan effect at a boundary of the current ULLBC.
/// Their scope is the existing next operation's before-state, not a guessed
/// empty set and not the previous operation's after-state (NLL may kill loans
/// between those two boundaries).
pub(crate) fn insert(info: &mut BodyBorrowckInfo, block: u::BlockId, index: usize, count: usize) {
    if count == 0 || info.unsupported.is_some() {
        return;
    }
    let Some(scope) = info
        .points
        .iter()
        .find(|p| p.location.block == block && p.location.statement_index == index)
        .map(|p| p.before.clone())
    else {
        info.unsupported = Some("borrowck: missing insertion boundary".into());
        return;
    };
    for point in &mut info.points {
        if point.location.block == block && point.location.statement_index >= index {
            point.location.statement_index += count;
        }
    }
    for loan in &mut info.loans {
        if loan.reserve.block == block && loan.reserve.statement_index >= index {
            loan.reserve.statement_index += count;
        }
    }
    for offset in 0..count {
        info.points.push(BorrowckPoint {
            location: BorrowckLocation {
                block,
                statement_index: index + offset,
            },
            synthetic: true,
            before: scope.clone(),
            after: scope.clone(),
        });
    }
}

/// Apply a transformation's exact old->new position map. Deleted locations
/// disappear; removing a loan creation is unsupported until loan elimination
/// has an explicit semantic adapter.
pub(crate) fn remap(
    info: &mut BodyBorrowckInfo,
    map: impl Fn(BorrowckLocation) -> Option<BorrowckLocation>,
) {
    if info.unsupported.is_some() {
        return;
    }
    info.points.retain_mut(|p| {
        if let Some(location) = map(p.location) {
            p.location = location;
            true
        } else {
            false
        }
    });
    for loan in &mut info.loans {
        if let Some(location) = map(loan.reserve) {
            loan.reserve = location;
        } else {
            info.unsupported = Some("borrowck: transformation removed a loan creation".into());
        }
    }
}

pub(crate) fn audit_function(ctx: &mut TransformCtx, decl: &FunDecl, pass: &str) {
    let Some(bundle) = ctx.translated.borrowck.as_mut() else {
        return;
    };
    let Some(info) = bundle
        .bodies
        .iter_mut()
        .find(|b| b.function_id == decl.def_id)
    else {
        return;
    };
    if let Body::Unstructured(body) = &decl.body {
        let fingerprint = borrowck_body_fingerprint(body);
        // These passes only change type region representatives / comments.
        // They do not move program points or renumber local identities.
        let preserves_points = pass.ends_with("typecheck_and_unify::Check")
            || pass.ends_with("add_missing_info::recover_body_comments::Transform");
        if fingerprint != info.body_fingerprint && !preserves_points {
            info.unsupported
                .get_or_insert_with(|| format!("borrowck: unsupported adaptation through {pass}"));
        }
        info.body_fingerprint = fingerprint;
    } else {
        info.unsupported = Some(
            "borrowck: LLBC facts are not yet adapted; normal LLBC reconstruction was performed"
                .into(),
        );
        info.points.clear();
        info.loans.clear();
        info.reference_regions.clear();
        info.outlives.clear();
        info.body_fingerprint.clear();
    }
}

pub(crate) fn audit(ctx: &mut TransformCtx, pass: &str) {
    if ctx.translated.borrowck.is_some() {
        ctx.for_each_fun_decl(|ctx, decl| audit_function(ctx, decl, pass));
    }
}

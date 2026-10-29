use charon_lib::{
    ast::*, export::CrateData, formatter::IntoFormatter, pretty::FmtWithCtx, ullbc_ast as u,
};
use ullbc_pcg::{Action, BorrowckQuery, Edge, ErrorKind, EvalStmtPhase, Place, analyze};

fn fixture(name: &str) -> (u::ExprBody, BodyBorrowckInfo) {
    let data: CrateData = serde_json::from_str(include_str!("fixtures/cfg_borrows.ullbc")).unwrap();
    assert!(!data.has_errors);
    let fmt = (&data.translated).into_fmt();
    let fun = data
        .translated
        .fun_decls
        .iter()
        .find(|f| {
            f.item_meta
                .name
                .to_string_with_ctx(&fmt)
                .ends_with(&format!("::{name}"))
        })
        .unwrap();
    let Body::Unstructured(body) = &fun.body else {
        panic!("unstructured fixture")
    };
    let facts = data
        .translated
        .borrowck
        .as_ref()
        .unwrap()
        .bodies
        .iter()
        .find(|b| b.function_id == fun.def_id)
        .unwrap();
    (body.clone(), facts.clone())
}

#[test]
fn cfg_worklist_preserves_live_borrow_across_an_explicit_goto() {
    let (mut body, mut facts) = fixture("goto_borrow");
    // Normal Charon merges this straight-line source into one final block.
    // For this solver-level test only, split it at a known boundary without
    // changing operations. This is a constructed CFG, not an extracted three-
    // block fixture. Facts move with operations; the inserted goto is neutral.
    assert_eq!(body.body.len(), 1);
    let cut = facts.loans[0].reserve.statement_index + 1;
    let scope = facts
        .points
        .iter()
        .find(|p| p.location.statement_index == cut)
        .unwrap()
        .before
        .clone();
    let tail = body.body[u::BlockId::ZERO].statements.split_off(cut);
    let terminator = body.body[u::BlockId::ZERO].terminator.clone();
    let target = body.body.push(u::BlockData {
        statements: tail,
        terminator,
    });
    body.body[u::BlockId::ZERO].terminator.kind = u::TerminatorKind::Goto { target };
    for p in &mut facts.points {
        if p.location.statement_index >= cut {
            p.location.block = target;
            p.location.statement_index -= cut;
        }
    }
    facts.points.push(BorrowckPoint {
        location: BorrowckLocation {
            block: u::BlockId::ZERO,
            statement_index: cut,
        },
        synthetic: true,
        before: scope.clone(),
        after: scope,
    });
    facts.body_fingerprint = borrowck_body_fingerprint(&body);
    let result = analyze(&body, &facts).unwrap();
    assert_eq!(result.blocks.len(), 2);
    let loan = &facts.loans[0];
    let r = loan.assigned;
    let mut crossed_live_edge = false;
    for block in &result.blocks {
        for target in &block.successors {
            let next = result.blocks.iter().find(|b| b.block == *target).unwrap();
            assert_eq!(block.exit, next.entry);
            let terminator = result
                .points
                .iter()
                .rfind(|p| p.location.block == block.block)
                .unwrap();
            if terminator.live_references_after.contains(&r) {
                crossed_live_edge = true;
                assert!(terminator.loans_before.contains(&loan.id));
                assert_eq!(block.exit.borrows().count(), 1);
                assert_eq!(next.entry.borrows().count(), 1);
                assert_eq!(next.entry.capability(Place::Local(loan.borrowed)), None);
            }
        }
    }
    assert!(
        crossed_live_edge,
        "constructed CFG must exercise a loan across its goto edge"
    );
}

#[test]
fn self_constructed_positive_cases_have_complete_phase_traces() {
    for name in [
        "repeated_write",
        "sequential_borrows",
        "goto_borrow",
        "two_shared",
        "unit_result",
        "removed_unit_before_borrow",
    ] {
        let (body, facts) = fixture(name);
        let analysis = analyze(&body, &facts).unwrap();
        assert_eq!(
            analysis.points.len(),
            body.body
                .iter()
                .map(|b| b.statements.len() + 1)
                .sum::<usize>()
        );
        for point in &analysis.points {
            assert_eq!(point.phases.len(), 4);
            let mut previous = &point.initial;
            for phase in &point.phases {
                assert_eq!(previous, &phase.before);
                previous = &phase.after;
            }
        }
    }
}

#[test]
fn compiler_scope_actually_guards_loan_release() {
    let (body, mut facts) = fixture("sequential_borrows");
    let analysis = analyze(&body, &facts).unwrap();
    let released = analysis.points.iter().find(|p| p.actions().any(|a|
        matches!(&a.action, Action::RemoveEdge {edge: Edge::Borrow {location, ..}} if *location == facts.loans[0].reserve.into()))).unwrap();
    assert!(
        released
            .phase(EvalStmtPhase::PreOperands)
            .borrowck_queries
            .iter()
            .any(|q| matches!(
                q,
                BorrowckQuery::LoanInScope {
                    loan: 0,
                    result: false,
                    ..
                }
            ))
    );
    // Keep the first loan in compiler scope through the second borrow. ULLBC
    // liveness alone would release it; the new query must prevent obtaining E.
    let start = released.location;
    let second = facts.loans[1].reserve;
    for point in &mut facts.points {
        if point.location.block == start.block
            && point.location.statement_index >= start.statement_index
            && point.location.statement_index <= second.statement_index
            && !point.before.contains(&0)
        {
            point.before.push(0);
        }
    }
    let error = analyze(&body, &facts).unwrap_err();
    assert_eq!(error.kind, ErrorKind::CapabilityUnavailable);
}

#[test]
fn missing_and_duplicate_points_are_rejected() {
    let (body, facts) = fixture("repeated_write");
    let mut missing = facts.clone();
    missing.points.pop();
    assert_eq!(
        analyze(&body, &missing).unwrap_err().kind,
        ErrorKind::InvalidInput
    );
    let mut duplicate = facts.clone();
    duplicate.points.push(facts.points[0].clone());
    assert_eq!(
        analyze(&body, &duplicate).unwrap_err().kind,
        ErrorKind::InvalidInput
    );
}

#[test]
fn wrong_loan_anchor_and_unknown_scope_id_are_rejected() {
    let (body, facts) = fixture("repeated_write");
    let mut wrong = facts.clone();
    wrong.loans[0].reserve.statement_index = 0;
    assert_eq!(
        analyze(&body, &wrong).unwrap_err().kind,
        ErrorKind::InvalidInput
    );
    let mut unknown = facts;
    unknown.points[0].before.push(9999);
    assert_eq!(
        analyze(&body, &unknown).unwrap_err().kind,
        ErrorKind::InvalidInput
    );
}

#[test]
fn unit_initialization_has_explicit_synthetic_anchor() {
    let (body, facts) = fixture("unit_result");
    assert!(matches!(
        body.body[0].statements[0].kind,
        u::StatementKind::StorageLive(LocalId::ZERO)
    ));
    assert!(matches!(
        body.body[0].statements[1].kind,
        u::StatementKind::Assign(_, _)
    ));
    assert!(facts.points[0].synthetic);
    assert!(facts.points[1].synthetic);
    assert_eq!(facts.points[0].before, facts.points[0].after);
    assert_eq!(facts.points[0].location.statement_index, 0);
    assert_eq!(facts.body_fingerprint, borrowck_body_fingerprint(&body));
    analyze(&body, &facts).unwrap();
}

#[test]
fn unsupported_semantics_never_return_partial_analysis() {
    for name in ["branch", "cycle", "reborrow"] {
        let (body, facts) = fixture(name);
        assert_eq!(
            analyze(&body, &facts).unwrap_err().kind,
            ErrorKind::Unsupported
        );
    }
}

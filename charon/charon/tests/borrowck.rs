//! End-to-end tests of rustc capture and ULLBC point adaptation.
use charon_lib::{ast::*, ullbc_ast as u};
mod util;

#[test]
fn compiler_facts_follow_the_exported_cfg_and_survive_serialization() -> anyhow::Result<()> {
    let krate = util::translate_rust_text(
        "pub fn test() -> i32 { let mut x = loop { break 0; }; let r = &mut x; loop { *r = 1; break; } x }",
        &[
            "--ullbc",
            "--export-borrowck",
            "--mir",
            "promoted",
            "--sysroot",
            "default",
        ],
    )?;
    let bundle = krate.borrowck.as_ref().unwrap();
    assert_eq!(bundle.stage, "ullbc-final-borrowck-v2");
    let facts = &bundle.bodies[0];
    assert!(facts.unsupported.is_none(), "{:?}", facts.unsupported);
    let Body::Unstructured(body) = &krate.fun_decls[facts.function_id].body else {
        panic!("ULLBC")
    };
    assert_eq!(body.body.len(), 1);
    assert!(matches!(
        body.body[0].statements[0].kind,
        u::StatementKind::StorageLive(LocalId::ZERO)
    ));
    assert_eq!(facts.body_fingerprint, borrowck_body_fingerprint(body));
    assert_eq!(
        facts.points.len(),
        body.body
            .iter()
            .map(|b| b.statements.len() + 1)
            .sum::<usize>()
    );
    let loan = &facts.loans[0];
    assert!(matches!(
        &body.body[loan.reserve.block].statements[loan.reserve.statement_index].kind,
        u::StatementKind::Assign(_, Rvalue::Ref { .. })
    ));
    assert!(facts.points.iter().any(
        |p| p.location.statement_index > loan.reserve.statement_index
            && p.before.contains(&loan.id)
    ));
    Ok(())
}

#[test]
fn projected_loan_reservations_identify_the_field_and_reborrow_source() -> anyhow::Result<()> {
    let krate = util::translate_rust_text(
        "pub struct Pair { x: i32, y: i32 } pub fn test(mut p: Pair) -> Pair { let r = &mut p.y; let s = &mut *r; *s = p.x; p }",
        &[
            "--ullbc",
            "--mir",
            "promoted",
            "--export-borrowck",
            "--sysroot",
            "default",
        ],
    )?;
    let facts = krate
        .borrowck
        .as_ref()
        .unwrap()
        .bodies
        .iter()
        .find(|b| b.loans.len() == 2)
        .unwrap();
    assert!(facts.unsupported.is_none());
    let Body::Unstructured(body) = &krate.fun_decls[facts.function_id].body else {
        panic!("expected ULLBC")
    };
    for (i, loan) in facts.loans.iter().enumerate() {
        let u::StatementKind::Assign(dst, Rvalue::Ref { place, .. }) =
            &body.body[loan.reserve.block].statements[loan.reserve.statement_index].kind
        else {
            panic!("expected borrow assignment")
        };
        assert_eq!(dst.as_local(), Some(loan.assigned));
        let PlaceKind::Projection(base, elem) = &place.kind else {
            panic!("expected projection")
        };
        assert_eq!(base.as_local(), Some(loan.borrowed));
        if i == 0 {
            assert!(matches!(elem, ProjectionElem::Field(_, id) if id.index() == 1));
        } else {
            assert!(matches!(elem, ProjectionElem::Deref));
            assert_eq!(loan.borrowed, facts.loans[0].assigned);
        }
    }
    Ok(())
}

#[test]
fn unsupported_adapter_data_is_explicit() -> anyhow::Result<()> {
    let krate = util::translate_rust_text(
        "pub fn test(a: &mut [i32], i: usize) { let r = &mut a[i]; *r = 1; }",
        &["--ullbc", "--export-borrowck", "--sysroot", "default"],
    )?;
    assert!(
        krate.borrowck.as_ref().unwrap().bodies[0]
            .unsupported
            .as_ref()
            .unwrap()
            .contains("projected loan")
    );
    Ok(())
}

#[test]
fn borrowck_export_preserves_normal_final_ullbc() -> anyhow::Result<()> {
    for code in [
        "pub fn test() -> i32 { let mut x = 0; let r = &mut x; *r = 1; x }",
        "pub fn test() { let mut x = 0; let r = &mut x; *r = 1; }",
        "pub fn test() -> i32 { let mut x = loop { break 0; }; let r = &mut x; loop { *r = 1; break; } x }",
        "pub fn test() -> i32 { let unused = (); let mut x = 0; let r = &mut x; *r = 1; x }",
    ] {
        let ordinary = util::translate_rust_text(
            code,
            &["--ullbc", "--mir", "promoted", "--sysroot", "default"],
        )?;
        let annotated = util::translate_rust_text(
            code,
            &[
                "--ullbc",
                "--export-borrowck",
                "--mir",
                "promoted",
                "--sysroot",
                "default",
            ],
        )?;
        let facts = &annotated.borrowck.as_ref().unwrap().bodies[0];
        assert!(
            facts.unsupported.is_none(),
            "{code}: {:?}",
            facts.unsupported
        );
        let Body::Unstructured(actual) = &annotated.fun_decls[facts.function_id].body else {
            panic!("ULLBC")
        };
        let Body::Unstructured(expected) = &ordinary.fun_decls[facts.function_id].body else {
            panic!("ULLBC")
        };
        // Files are extracted in different temp directories; compare the complete
        // bodies after normalizing source spans only, including local/type/CFG data.
        assert_eq!(
            facts.points.len(),
            actual
                .body
                .iter()
                .map(|b| b.statements.len() + 1)
                .sum::<usize>()
        );
        for point in &facts.points {
            assert!(
                point.location.statement_index
                    <= actual.body[point.location.block].statements.len()
            );
            if point.synthetic {
                assert_eq!(point.before, point.after);
            }
        }
        for loan in &facts.loans {
            let u::StatementKind::Assign(dest, Rvalue::Ref { place, .. }) =
                &actual.body[loan.reserve.block].statements[loan.reserve.statement_index].kind
            else {
                panic!("loan must anchor its borrow")
            };
            assert_eq!(dest.as_local(), Some(loan.assigned));
            assert_eq!(place.as_local(), Some(loan.borrowed));
        }
        let mut actual = actual.clone();
        let mut expected = expected.clone();
        actual.dyn_visit_in_body_mut(|s: &mut Span| *s = Span::dummy());
        expected.dyn_visit_in_body_mut(|s: &mut Span| *s = Span::dummy());
        assert_eq!(
            borrowck_body_fingerprint(&actual),
            borrowck_body_fingerprint(&expected),
            "{code}"
        );
    }
    Ok(())
}

#[test]
fn requesting_borrowck_does_not_skip_llbc_reconstruction() -> anyhow::Result<()> {
    let krate = util::translate_rust_text(
        "pub fn test() -> i32 { 1 }",
        &["--export-borrowck", "--sysroot", "default"],
    )?;
    let facts = &krate.borrowck.as_ref().unwrap().bodies[0];
    assert!(matches!(
        krate.fun_decls[facts.function_id].body,
        Body::Structured(_)
    ));
    assert!(facts.unsupported.as_ref().unwrap().contains("LLBC"));
    assert!(facts.points.is_empty());
    Ok(())
}

#[test]
fn unsupported_transform_still_emits_the_normal_final_body() -> anyhow::Result<()> {
    let code = "pub fn test(x: i32) -> i32 { x + 1 }";
    let ordinary = util::translate_rust_text(
        code,
        &[
            "--ullbc",
            "--reconstruct-fallible-operations",
            "--rustc-arg=-Coverflow-checks=yes",
            "--sysroot",
            "default",
        ],
    )?;
    let annotated = util::translate_rust_text(
        code,
        &[
            "--ullbc",
            "--export-borrowck",
            "--reconstruct-fallible-operations",
            "--rustc-arg=-Coverflow-checks=yes",
            "--sysroot",
            "default",
        ],
    )?;
    let facts = &annotated.borrowck.as_ref().unwrap().bodies[0];
    assert!(
        facts
            .unsupported
            .as_ref()
            .unwrap()
            .contains("unsupported adaptation")
    );
    let Body::Unstructured(actual) = &annotated.fun_decls[facts.function_id].body else {
        panic!("ULLBC")
    };
    let Body::Unstructured(expected) = &ordinary.fun_decls[facts.function_id].body else {
        panic!("ULLBC")
    };
    let mut actual = actual.clone();
    let mut expected = expected.clone();
    actual.dyn_visit_in_body_mut(|s: &mut Span| *s = Span::dummy());
    expected.dyn_visit_in_body_mut(|s: &mut Span| *s = Span::dummy());
    assert_eq!(
        borrowck_body_fingerprint(&actual),
        borrowck_body_fingerprint(&expected)
    );
    Ok(())
}

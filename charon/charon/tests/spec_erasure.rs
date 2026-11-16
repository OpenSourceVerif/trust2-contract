//! Integration tests for pre-borrowck specification extraction, host erasure, and anchor legality.
use assert_cmd::prelude::CommandCargoExt;
use charon_lib::{ast::*, export::CrateData, options::SerializationFormat, ullbc_ast as u};
use std::{fs, process::Command};

mod util;

/// Self-contained stub matching `trust2_contract::internal` so single-file `charon rustc` tests
/// exercise the exact driver hooks without spawning full Cargo builds per case.
const INTERNAL_PRELUDE: &str = r#"
pub mod internal {
    pub fn entry(_: u32) {}
    pub fn precondition<T: FnOnce() -> bool>(_: T) {}
    pub fn postcondition<T, U: FnOnce(T) -> bool>(_: U) {}
    pub trait TypeInvariant {
        fn invariant(&self) -> bool;
    }
    pub fn contract_assert<T: FnOnce() -> bool>(_: T) {}
    pub fn contract_assume<T: FnOnce() -> bool>(_: T) {}
    pub fn forall<T, U: FnOnce(T) -> bool>(_: U) -> bool { true }
    pub fn exists<T, U: FnOnce(T) -> bool>(_: U) -> bool { true }
    pub fn implies(_: bool, _: bool) -> bool { true }
}
"#;

fn run_charon_as_trust2_contract(
    body_code: &str,
    charon_flags: &[&str],
) -> anyhow::Result<Result<TranslatedCrate, String>> {
    let tmp_dir = tempfile::TempDir::new()?;
    let input_path = tmp_dir.path().join("lib.rs");
    let output_path = tmp_dir.path().join("out.llbc");
    fs::write(&input_path, format!("{INTERNAL_PRELUDE}\n{body_code}"))?;

    let output = Command::cargo_bin("charon")?
        .arg("rustc")
        .arg("--abort-on-error")
        .arg("--dest-file")
        .arg(&output_path)
        .args(charon_flags)
        .arg("--")
        .arg("--edition=2024")
        .arg("--crate-type=rlib")
        .arg("--crate-name=trust2_contract")
        .arg(&input_path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Ok(Err(stderr));
    }

    let data = CrateData::deserialize_from_file(&output_path, SerializationFormat::Json)?;
    Ok(Ok(data.translated))
}

fn expect_ok(code: &str) -> TranslatedCrate {
    match run_charon_as_trust2_contract(
        code,
        &[
            "--error-on-warnings",
            "--precise-drops",
            "--sysroot",
            "default",
        ],
    )
    .unwrap()
    {
        Ok(krate) => krate,
        Err(stderr) => panic!("expected success, but charon failed:\n{stderr}"),
    }
}

fn expect_err_containing(code: &str, expected_substring: &str) {
    match run_charon_as_trust2_contract(code, &["--precise-drops", "--sysroot", "default"]).unwrap()
    {
        Ok(_) => panic!(
            "expected charon failure containing `{expected_substring}`, but compilation succeeded"
        ),
        Err(stderr) => assert!(
            stderr.contains(expected_substring),
            "expected error containing `{expected_substring}`, got:\n{stderr}"
        ),
    }
}

#[test]
fn non_copy_and_drop_values_can_be_reused_in_specs_and_moved_by_host() {
    let krate = expect_ok(
        r#"
        pub struct NonCopy {
            pub v: i32,
        }
        impl Drop for NonCopy {
            fn drop(&mut self) {}
        }

        pub struct Pair {
            pub a: NonCopy,
            pub b: NonCopy,
        }

        pub fn consume(x: NonCopy) -> bool {
            x.v >= 0
        }

        pub fn normalize(x: NonCopy) -> NonCopy {
            x
        }

        impl internal::TypeInvariant for Pair {
            fn invariant(&self) -> bool {
                // Moving non-Copy `Drop` fields out of `&self` multiple times in a type invariant.
                consume(self.a) && consume(self.a) && consume(self.b)
            }
        }

        pub fn passthrough(x: NonCopy, mut p: Pair) -> NonCopy {
            {
                internal::entry(0);
                internal::precondition(|| {
                    let mut acc = 0;
                    if consume(x) && consume(normalize(x)) {
                        acc = 1;
                    }
                    acc == 1
                });
            }
            {
                internal::entry(0);
                internal::postcondition::<NonCopy, _>(|res| {
                    consume(x)
                        && consume(res)
                        && consume(res)
                        && {
                            internal::entry(0);
                            internal::forall(|q: NonCopy| internal::implies(consume(q), consume(q)))
                        }
                });
            }
            let r = &mut p.a;
            {
                internal::entry(0);
                internal::contract_assert(|| consume(p.b) && consume(p.b) && r.v == 0);
            }
            r.v += 1;
            x
        }
        "#,
    );

    let fun = krate
        .fun_decls
        .iter()
        .find(|f| {
            f.item_meta
                .name
                .name
                .last()
                .is_some_and(|e| e.as_ident().is_some_and(|(s, _)| s == "passthrough"))
        })
        .expect("passthrough function");
    assert_eq!(fun.specs.preconditions.len(), 1);
    assert_eq!(fun.specs.postconditions.len(), 1);
}

#[test]
fn active_mutable_borrow_blocks_direct_reads_but_allows_deref_and_disjoint_fields() {
    // 1. Disjoint field `p.b` and deref `*r` are allowed while `r = &mut p.a` is live;
    //    reading `p` again AFTER `r`'s NLL borrow ends is also allowed.
    expect_ok(
        r#"
        pub struct NonCopy(pub i32);
        pub struct Pair { pub a: NonCopy, pub b: NonCopy }
        pub fn consume(_: NonCopy) -> bool { true }
        pub fn consume_pair(_: Pair) -> bool { true }

        pub fn allowed(mut p: Pair) -> i32 {
            let r = &mut p.a;
            {
                internal::entry(0);
                internal::contract_assert(|| consume(p.b) && r.0 >= 0);
            }
            r.0 += 1;
            // `r` is dead here under NLL, so reading `p` as a whole is valid again.
            {
                internal::entry(0);
                internal::contract_assert(|| consume_pair(p));
            }
            p.a.0 + p.b.0
        }
        "#,
    );

    // 2. Directly reading `x` while `r = &mut x` is live must be rejected.
    expect_err_containing(
        r#"
        pub struct NonCopy(pub i32);
        pub fn consume(_: NonCopy) -> bool { true }

        pub fn reject_direct_read(mut x: NonCopy) -> i32 {
            let r = &mut x;
            {
                internal::entry(0);
                internal::contract_assert(|| consume(x));
            }
            r.0 += 1;
            x.0
        }
        "#,
        "cannot read mutably borrowed value in specification",
    );

    // 3. Reading whole `p` while `r = &mut p.a` is live must be rejected.
    expect_err_containing(
        r#"
        pub struct NonCopy(pub i32);
        pub struct Pair { pub a: NonCopy, pub b: NonCopy }
        pub fn consume_pair(_: Pair) -> bool { true }

        pub fn reject_overlapping_parent(mut p: Pair) -> i32 {
            let r = &mut p.a;
            {
                internal::entry(0);
                internal::contract_assert(|| consume_pair(p));
            }
            r.0 += 1;
            p.a.0
        }
        "#,
        "cannot read mutably borrowed value in specification",
    );

    // 4. Reading `*r` while a nested mutable reborrow `r2 = &mut *r` is live must be rejected.
    expect_err_containing(
        r#"
        pub fn reject_reborrowed_parent(mut x: i32) -> i32 {
            let r = &mut x;
            let r2 = &mut *r;
            {
                internal::entry(0);
                internal::contract_assert(|| *r == 0);
            }
            *r2 += 1;
            x
        }
        "#,
        "cannot read mutably borrowed value in specification",
    );
}

#[test]
fn moved_and_uninitialized_places_are_rejected_at_spec_anchors() {
    // Reading unmoved sibling `p.b` after `drop(p.a)` is valid.
    expect_ok(
        r#"
        pub struct NonCopy(pub i32);
        pub struct Pair { pub a: NonCopy, pub b: NonCopy }
        pub fn consume(_: NonCopy) -> bool { true }

        pub fn ok_unmoved_sibling(p: Pair) {
            drop(p.a);
            {
                internal::entry(0);
                internal::contract_assert(|| consume(p.b));
            }
        }
        "#,
    );

    // Reading moved variable `x` is rejected.
    expect_err_containing(
        r#"
        pub struct NonCopy(pub i32);
        pub fn consume(_: NonCopy) -> bool { true }

        pub fn err_use_after_move(x: NonCopy) {
            drop(x);
            {
                internal::entry(0);
                internal::contract_assert(|| consume(x));
            }
        }
        "#,
        "cannot read moved or uninitialized value in specification",
    );

    // Reading partially moved `p` as a whole is rejected.
    expect_err_containing(
        r#"
        pub struct NonCopy(pub i32);
        pub struct Pair { pub a: NonCopy, pub b: NonCopy }
        pub fn consume_pair(_: Pair) -> bool { true }

        pub fn err_partial_move(p: Pair) {
            drop(p.a);
            {
                internal::entry(0);
                internal::contract_assert(|| consume_pair(p));
            }
        }
        "#,
        "cannot read moved or uninitialized value in specification",
    );

    // Reading uninitialized binding `x` is rejected.
    expect_err_containing(
        r#"
        pub struct NonCopy(pub i32);
        pub fn consume(_: NonCopy) -> bool { true }

        pub fn err_uninitialized() -> NonCopy {
            let x: NonCopy;
            {
                internal::entry(0);
                internal::contract_assert(|| consume(x));
            }
            x = NonCopy(1);
            x
        }
        "#,
        "cannot read moved or uninitialized value in specification",
    );
}

#[test]
fn impure_constructs_inside_specs_and_runtime_quantifiers_are_rejected() {
    // 1. Mutating a host variable inside a spec.
    expect_err_containing(
        r#"
        pub fn err_mutate_host(mut x: i32) -> i32 {
            {
                internal::entry(0);
                internal::contract_assert(|| { x = 1; true });
            }
            x
        }
        "#,
        "specifications cannot mutably borrow or mutate program variables",
    );

    // 2. Taking `&mut` of a spec-local variable.
    expect_err_containing(
        r#"
        pub fn err_local_mut_ref() {
            {
                internal::entry(0);
                internal::contract_assert(|| {
                    let mut s = 0;
                    let r = &mut s;
                    *r == 0
                });
            }
        }
        "#,
        "mutable borrows (`&mut`) are not allowed in specifications",
    );

    // 3. Projected field write inside a spec.
    expect_err_containing(
        r#"
        pub struct Pair { pub a: i32, pub b: i32 }
        pub fn err_field_write() {
            {
                internal::entry(0);
                internal::contract_assert(|| {
                    let mut p = Pair { a: 0, b: 1 };
                    p.a = 2;
                    p.a == 2
                });
            }
        }
        "#,
        "mutating fields or dereferenced references is not allowed in specifications",
    );

    // 4. Loops inside a spec.
    expect_err_containing(
        r#"
        pub fn err_loop_in_spec() {
            {
                internal::entry(0);
                internal::contract_assert(|| {
                    let mut i = 0;
                    while i < 3 {
                        i += 1;
                    }
                    i == 3
                });
            }
        }
        "#,
        "loops are not allowed in specifications",
    );

    // 5. Quantifier directly in runtime host code.
    expect_err_containing(
        r#"
        pub fn err_runtime_quantifier() -> bool {
            internal::entry(0);
            internal::forall(|x: i32| x == x)
        }
        "#,
        "quantifiers (`forall!` / `exists!`) can only be used inside specifications",
    );

    // 6. Nested contract assertion inside a specification body.
    expect_err_containing(
        r#"
        pub fn err_nested_assert_in_spec(x: i32) {
            {
                internal::entry(0);
                internal::precondition(|| {
                    {
                        internal::entry(0);
                        internal::contract_assert(|| x > 0);
                    }
                    x >= 0
                });
            }
        }
        "#,
        "nested contracts or assertions are not allowed inside specifications",
    );
}

#[test]
fn function_contracts_leave_zero_borrowck_pollution_for_ullbc_pcg() -> anyhow::Result<()> {
    let bare = run_charon_as_trust2_contract(
        "pub fn target(y: i32) -> i32 { let mut x = y; let r = &mut x; *r = 1; x }",
        &[
            "--ullbc",
            "--export-borrowck",
            "--mir",
            "promoted",
            "--sysroot",
            "default",
        ],
    )?
    .expect("bare borrowck export should succeed");

    let contracted = run_charon_as_trust2_contract(
        r#"
        pub fn target(y: i32) -> i32 {
            {
                internal::entry(0);
                internal::precondition(|| y >= 0);
            }
            {
                internal::entry(0);
                internal::postcondition::<i32, _>(|res| res == 1 && y >= 0);
            }
            let mut x = y;
            let r = &mut x;
            *r = 1;
            x
        }
        "#,
        &[
            "--ullbc",
            "--export-borrowck",
            "--mir",
            "promoted",
            "--sysroot",
            "default",
        ],
    )?
    .expect("contracted borrowck export should succeed");

    let bare_fun = bare
        .fun_decls
        .iter()
        .find(|f| {
            f.item_meta
                .name
                .name
                .last()
                .is_some_and(|e| e.as_ident().is_some_and(|(s, _)| s == "target"))
        })
        .expect("bare target function");
    let bare_facts = bare
        .borrowck
        .as_ref()
        .unwrap()
        .bodies
        .iter()
        .find(|b| b.function_id == bare_fun.def_id)
        .expect("bare borrowck info");
    assert!(bare_facts.unsupported.is_none());

    let contracted_fun = contracted
        .fun_decls
        .iter()
        .find(|f| {
            f.item_meta
                .name
                .name
                .last()
                .is_some_and(|e| e.as_ident().is_some_and(|(s, _)| s == "target"))
        })
        .expect("target function");
    assert_eq!(contracted_fun.specs.preconditions.len(), 1);
    assert_eq!(contracted_fun.specs.postconditions.len(), 1);

    let contracted_facts = contracted
        .borrowck
        .as_ref()
        .unwrap()
        .bodies
        .iter()
        .find(|b| b.function_id == contracted_fun.def_id)
        .expect("borrowck info for target");
    assert!(
        contracted_facts.unsupported.is_none(),
        "{:?}",
        contracted_facts.unsupported
    );
    // Only the single real `&mut x` loan exists; zero fake loans from precondition/postcondition.
    assert_eq!(contracted_facts.loans.len(), 1);
    assert!(contracted_facts.loans[0].is_mutable);

    let Body::Unstructured(mut actual) = contracted_fun.body.clone() else {
        panic!("expected unstructured ULLBC");
    };
    let Body::Unstructured(mut expected) = bare.fun_decls[bare_facts.function_id].body.clone()
    else {
        panic!("expected unstructured ULLBC");
    };
    assert_eq!(
        contracted_facts.body_fingerprint,
        borrowck_body_fingerprint(&actual)
    );
    actual.dyn_visit_in_body_mut(|s: &mut Span| *s = Span::dummy());
    expected.dyn_visit_in_body_mut(|s: &mut Span| *s = Span::dummy());
    assert_eq!(actual, expected);
    assert_eq!(
        borrowck_body_fingerprint(&actual),
        borrowck_body_fingerprint(&expected)
    );
    assert!(matches!(
        actual.body[u::BlockId::ZERO].terminator.kind,
        u::TerminatorKind::Return
    ));

    Ok(())
}

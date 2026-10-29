use std::{fs, path::PathBuf, process::Command};

struct Output(PathBuf);
impl Output {
    fn new(name: &str) -> Self {
        Self(std::env::temp_dir().join(format!("ullbc-pcg-{}-{name}", std::process::id())))
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cli_exports_actual_statements_actions_and_graphs() {
    let output = Output::new("export");
    let status = Command::new(env!("CARGO_BIN_EXE_ullbc-pcg"))
        .args([
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/mutable_borrow.ullbc"
            ),
            "--function",
            "mutable_borrow",
            "--output",
        ])
        .arg(&output.0)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.0.join("analysis.json")).unwrap()).unwrap();
    assert_eq!(json["schema_version"], 3);
    assert!(json["source"].as_str().unwrap().contains("let r = &mut x;"));
    let points = json["points"].as_array().unwrap();
    assert_eq!(points.len(), 10);
    assert_eq!(points[0]["statement"], "storage_live(_0)");
    let borrow = points
        .iter()
        .find(|p| p["statement"] == "r = &mut x")
        .unwrap();
    let dot = fs::read_to_string(
        output.0.join(
            borrow["steps"].as_array().unwrap().last().unwrap()["dot"]
                .as_str()
                .unwrap(),
        ),
    )
    .unwrap();
    assert!(dot.contains("x: ∅"));
    assert!(dot.contains("_0: W"));
    assert!(!dot.contains("RETURN"));
    assert!(dot.contains("bw0: Default '?2"));
    assert!(dot.contains("ULLBC borrow at bb0:4"));
    assert!(!dot.contains("style=dashed"));
    assert!(dot.contains("r|'1"));
    assert!(!dot.contains("(*r)"));
    let read = points
        .iter()
        .find(|p| p["statement"] == "_0 = copy x")
        .unwrap();
    let dot = fs::read_to_string(
        output.0.join(
            read["steps"].as_array().unwrap().last().unwrap()["dot"]
                .as_str()
                .unwrap(),
        ),
    )
    .unwrap();
    assert!(dot.contains("x: E"));
    assert!(!dot.contains(" -> "));
    for point in points {
        let steps = point["steps"].as_array().unwrap();
        assert_eq!(steps[0]["kind"], "initial");
        let phases: Vec<_> = steps
            .iter()
            .filter(|s| s["kind"] == "phase")
            .map(|s| s["phase"].as_str().unwrap())
            .collect();
        assert_eq!(
            phases,
            ["pre_operands", "post_operands", "pre_main", "post_main"]
        );
        for step in steps {
            assert!(output.0.join(step["dot"].as_str().unwrap()).is_file());
        }
        assert!(steps.len() >= 5);
    }
    let write = points
        .iter()
        .find(|p| p["statement"] == "(*r) = const 1i32")
        .unwrap();
    let steps = write["steps"].as_array().unwrap();
    let pre_main = steps
        .iter()
        .find(|s| s["kind"] == "phase" && s["phase"] == "pre_main")
        .unwrap();
    let dot = fs::read_to_string(output.0.join(pre_main["dot"].as_str().unwrap())).unwrap();
    assert!(dot.contains("(*r): W"));
    assert!(dot.contains("r: e"));
    assert!(dot.contains("label=\"r|'1 before bb0:5\""));
    assert!(
        dot.contains("@ bb0:5 PreMain"),
        "phase remains available in the tooltip"
    );
    assert!(!dot.contains("shape=point"));
    assert_eq!(dot.matches("[color=green,").count(), 2);
    assert_eq!(dot.matches("[color=grey, style=dashed").count(), 1);
    assert_eq!(
        pre_main["graph"]["edges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "Deref")
            .count(),
        1
    );
}

#[test]
fn unsupported_function_produces_no_success_artifact() {
    let output = Output::new("unsupported");
    let status = Command::new(env!("CARGO_BIN_EXE_ullbc-pcg"))
        .args([
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/borrows.ullbc"),
            "--function",
            "overwrite_reference",
            "--output",
        ])
        .arg(&output.0)
        .output()
        .unwrap();
    assert!(!status.status.success());
    assert!(String::from_utf8_lossy(&status.stderr).contains("Unsupported"));
    assert!(!output.0.exists());
}

#[test]
fn cli_selects_single_function_but_requires_choice_for_multiple() {
    let output = Output::new("automatic-selection");
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let ambiguous = Command::new(env!("CARGO_BIN_EXE_ullbc-pcg"))
        .arg(fixture.join("borrows.ullbc"))
        .arg("--output")
        .arg(&output.0)
        .output()
        .unwrap();
    assert!(!ambiguous.status.success());
    let error = String::from_utf8_lossy(&ambiguous.stderr);
    assert!(error.contains("Use --function NAME"));
    assert!(error.contains("shared_borrow") && error.contains("moved_reference"));
    assert!(!output.0.exists());
    let single = Command::new(env!("CARGO_BIN_EXE_ullbc-pcg"))
        .arg(fixture.join("mutable_borrow.ullbc"))
        .arg("--output")
        .arg(&output.0)
        .output()
        .unwrap();
    assert!(
        single.status.success(),
        "{}",
        String::from_utf8_lossy(&single.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.0.join("analysis.json")).unwrap()).unwrap();
    assert!(json["name"].as_str().unwrap().ends_with("::mutable_borrow"));
}

#[test]
fn dot_labels_escape_local_names() {
    use charon_lib::ast::LocalId;
    use ullbc_pcg::{Capability, Graph, LocalInfo, Place, PlaceCapability, export::dot};
    let id = LocalId::from(0);
    let graph = Graph {
        capabilities: vec![PlaceCapability {
            place: Place::Local(id),
            capability: Capability::Exclusive,
        }],
        edges: vec![],
        owned: vec![],
    };
    let fixture: charon_lib::export::CrateData =
        serde_json::from_str(include_str!("fixtures/mutable_borrow.ullbc")).unwrap();
    let facts = &fixture.translated.borrowck.as_ref().unwrap().bodies[0];
    let result = dot(
        &graph,
        &[LocalInfo {
            id,
            name: Some("a\"\\\nb".into()),
        }],
        facts,
    );
    assert!(result.contains(r#"label="a\"\\\nb: E""#));
}

#[test]
fn missing_or_incompatible_borrowck_bundle_produces_no_success_artifact() {
    let scratch = Output::new("borrowck-validation");
    fs::create_dir_all(&scratch.0).unwrap();
    for case in ["missing", "schema", "stage", "function", "duplicate"] {
        let mut data: charon_lib::export::CrateData =
            serde_json::from_str(include_str!("fixtures/mutable_borrow.ullbc")).unwrap();
        match case {
            "missing" => data.translated.borrowck = None,
            "schema" => data.translated.borrowck.as_mut().unwrap().schema_version = 999,
            "stage" => {
                data.translated.borrowck.as_mut().unwrap().stage = "ullbc-borrowck-v1".into()
            }
            "function" => data.translated.borrowck.as_mut().unwrap().bodies.clear(),
            "duplicate" => {
                let bodies = &mut data.translated.borrowck.as_mut().unwrap().bodies;
                bodies.push(bodies[0].clone());
            }
            _ => unreachable!(),
        }
        let input = scratch.0.join(format!("{case}.ullbc"));
        let output = scratch.0.join(case);
        fs::write(&input, serde_json::to_vec(&data).unwrap()).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_ullbc-pcg"))
            .arg(&input)
            .arg("--output")
            .arg(&output)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("InvalidInput"),
            "{case}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!output.exists());
    }
}

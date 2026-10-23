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
    assert_eq!(json["schema_version"], 2);
    assert!(json["source"].as_str().unwrap().contains("let r = &mut x;"));
    let points = json["points"].as_array().unwrap();
    assert_eq!(points.len(), 10);
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
    assert!(
        dot.contains("shape=point"),
        "deref must be rendered as a hyperedge"
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
    let result = dot(
        &graph,
        &[LocalInfo {
            id,
            name: Some("a\"\\\nb".into()),
        }],
    );
    assert!(result.contains(r#"label="a\"\\\nb: E""#));
}

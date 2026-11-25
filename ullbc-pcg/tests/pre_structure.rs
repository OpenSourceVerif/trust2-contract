#![cfg(feature = "driver")]
use charon_lib::{ast::Body, export::CrateData, options::SerializationFormat};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::OnceLock,
};

fn run(fixture: &str, function: &str, output: &Path, llbc: &Path) -> Output {
    static CHARON: OnceLock<PathBuf> = OnceLock::new();
    let charon = CHARON.get_or_init(|| {
        std::env::var_os("CHARON_BIN")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/../charon/charon/Cargo.toml");
                let built = Command::new(env!("CARGO"))
                    .args([
                        "build",
                        "--locked",
                        "--manifest-path",
                        manifest,
                        "--bin",
                        "charon",
                        "--message-format=json",
                    ])
                    .output()
                    .unwrap();
                assert!(
                    built.status.success(),
                    "{}",
                    String::from_utf8_lossy(&built.stderr)
                );
                String::from_utf8(built.stdout)
                    .unwrap()
                    .lines()
                    .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                    .find_map(|record| record["executable"].as_str().map(std::path::PathBuf::from))
                    .expect("Cargo must report the Charon executable")
            })
    });
    Command::new(charon)
        .args([
            "rustc",
            "--export-borrowck",
            "--mir",
            "promoted",
            "--sysroot",
            "default",
            "--dest-file",
        ])
        .arg(llbc)
        .args([
            "--",
            "--crate-type",
            "lib",
            "--crate-name",
            "placement",
            fixture,
        ])
        .env("CHARON_DRIVER", env!("CARGO_BIN_EXE_ullbc-pcg-driver"))
        .env("ULLBC_PCG_OUTPUT", output)
        .env("ULLBC_PCG_FUNCTION", function)
        .output()
        .unwrap()
}

#[test]
fn live_analysis_precedes_nop_removal_and_llbc_reconstruction() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("viewer");
    let llbc = temp.path().join("out.llbc");
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cfg_borrows.rs");
    let result = run(fixture, "goto_borrow", &output, &llbc);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("analysis.json")).unwrap()).unwrap();
    assert_eq!(json["analysis_stage"], "before-control-flow-reconstruction");
    let points = json["points"].as_array().unwrap();
    let core: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("pcg.json")).unwrap()).unwrap();
    assert_eq!(core["schema_version"], 1);
    assert_eq!(core["function_id"], json["function_id"]);
    assert_eq!(core["body_fingerprint"], json["body_fingerprint"]);
    assert_eq!(core["blocks"], json["blocks"]);
    assert_eq!(core["points"].as_array().unwrap().len(), points.len());
    for (point, viewer) in core["points"].as_array().unwrap().iter().zip(points) {
        assert_eq!(point["location"], viewer["location"]);
        assert_eq!(point["phases"], viewer["phases"]);
    }
    assert_eq!(points.len(), 13);
    assert!(points.iter().any(|p| {
        p["statement"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("nop")
            && p["phases"][0]["actions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["action"].get("RemoveEdge").is_some())
    }));
    let data = CrateData::deserialize_from_file(&llbc, SerializationFormat::Json).unwrap();
    assert!(!data.has_errors);
    assert!(
        data.translated
            .fun_decls
            .iter()
            .any(|f| matches!(f.body, Body::Structured(_)))
    );
}

const PROJECTED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/projected_borrows.rs"
);

fn live_case(name: &str) -> (Value, Value) {
    let tmp = tempfile::tempdir().unwrap();
    let result = run(PROJECTED, name, tmp.path(), &tmp.path().join("case.llbc"));
    assert!(
        result.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let read = |file: &str| {
        serde_json::from_slice(&std::fs::read(tmp.path().join(file)).unwrap()).unwrap()
    };
    (read("pcg.json"), read("analysis.json"))
}

#[test]
fn pair_returns_loan_and_collapses_before_evaluating_whole_value_move() {
    let (core, viewer) = live_case("change");
    let points = viewer["points"].as_array().unwrap();
    let point = |text: &str| {
        points
            .iter()
            .position(|p| p["statement"].as_str().unwrap().contains(text))
            .unwrap()
    };
    let borrow = point(" = &mut");
    let write = point("(*r) =");
    let moved = point("= move p");
    assert!(borrow < write && write < moved && moved < point("storage_dead(r)"));
    let fields = &core["locals"][1]["fields"];
    assert_eq!(fields[0]["name"], "x");
    assert_eq!(fields[1]["name"], "y");
    let loan = &core["loans"][0];
    assert_eq!(loan["borrowed"]["Field"]["projection"], 0);
    assert_eq!(loan["reserve"], points[borrow]["location"]);
    assert_eq!(
        points[borrow]["phases"][0]["actions"][0]["action"]["Expand"]["fields"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let after = &points[borrow]["phases"][3]["after"];
    assert!(
        after["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["place"]["Field"]["projection"] == 1 && c["capability"] == "Exclusive")
    );
    assert!(
        after["edges"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["kind"] == "Borrow" && e["borrowed"] == loan["borrowed"])
    );
    let pm = &points[write]["phases"][2]["actions"];
    assert!(
        pm.as_array()
            .unwrap()
            .iter()
            .any(|a| a["action"]["AddEdge"]["edge"]["kind"] == "Deref")
    );
    let actions = points[moved]["phases"][0]["actions"].as_array().unwrap();
    let pos = |test: &dyn Fn(&Value) -> bool| {
        actions.iter().map(|a| &a["action"]).position(test).unwrap()
    };
    let deref = pos(&|a| a["RemoveEdge"]["edge"]["kind"] == "Deref");
    let release = pos(&|a| a["RemoveEdge"]["edge"]["kind"] == "Borrow");
    let regain = pos(&|a| {
        a["SetCapability"]["place"] == loan["borrowed"] && a["SetCapability"]["to"] == "Exclusive"
    });
    let collapse = pos(&|a| a.get("Collapse").is_some());
    assert!(deref < release && release < regain && regain < collapse);
    assert_eq!(core["points"][moved]["phases"], points[moved]["phases"]);
    let ret = &points[point("return")];
    let return_cap = |graph: &Value| {
        graph["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["place"] == core["return_place"])
            .unwrap()["capability"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(return_cap(&ret["phases"][2]["after"]), "Exclusive");
    assert_eq!(return_cap(&ret["phases"][3]["after"]), "Write");
}

#[test]
fn mutually_exclusive_field_loans_retain_guards_through_their_release() {
    let (core, viewer) = live_case("borrow_choice");
    let join = core["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["incoming"].as_array().unwrap().len() == 2)
        .unwrap();
    let guarded = join["entry"]["edge_conditions"].as_array().unwrap();
    let mut predecessors = std::collections::BTreeSet::new();
    for p in guarded
        .iter()
        .filter(|c| c["edge"]["kind"] == "Borrow" && c["edge"]["borrowed"].get("Field").is_some())
    {
        let paths = p["alternatives"].as_array().unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0][0]["to"], join["block"]);
        predecessors.insert(paths[0][0]["from"].as_u64().unwrap());
    }
    assert_eq!(predecessors.len(), 2);
    let moved = viewer["points"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["statement"] == "_0 = move p")
        .unwrap();
    let releases: Vec<_> = moved["phases"][0]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| {
            a["action"]["RemoveEdge"]["edge"]["borrowed"]
                .get("Field")
                .is_some()
        })
        .collect();
    assert_eq!(releases.len(), 2);
    assert_ne!(releases[0]["condition"], releases[1]["condition"]);
}

#[test]
fn independent_fields_reborrows_input_lifetimes_and_loop_backedge() {
    let (disjoint, _) = live_case("disjoint");
    assert!(disjoint["points"].as_array().unwrap().iter().any(|p| {
        p["phases"][3]["after"]["edges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "Borrow")
            .count()
            == 2
    }));
    let (nested, _) = live_case("nested");
    assert!(
        nested["loans"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["borrowed"].get("Deref").is_some())
    );
    let (shared, _) = live_case("shared_field");
    assert!(
        shared["points"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["phases"][0]["actions"].as_array().unwrap())
            .any(|a| a["action"]["Expand"]["capability"] == "Read")
    );
    let (returned, _) = live_case("return_arg");
    let exit = returned["blocks"][0]["exit"]["edges"].as_array().unwrap();
    assert!(
        exit.iter()
            .any(|e| e["kind"] == "InitialBorrowFlow" && e["argument"] == 1)
    );
    assert!(
        exit.iter()
            .any(|e| e["kind"] == "Borrow" && e["assigned"]["reference"] == 0)
    );
    let (cycled, _) = live_case("loop_fields");
    let header = cycled["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| {
            b["incoming"].as_array().unwrap().iter().any(|pred| {
                pred["predecessor"]
                    .as_u64()
                    .is_some_and(|p| p > b["block"].as_u64().unwrap())
            })
        })
        .unwrap();
    assert!(header["entry"]["edges"].as_array().unwrap().is_empty());
    assert!(header["incoming"].as_array().unwrap().iter().any(|pred| {
        pred["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["action"].get("Expand").is_some())
    }));
    let tmp = tempfile::tempdir().unwrap();
    let unsupported = run(
        PROJECTED,
        "loop_borrow",
        tmp.path(),
        &tmp.path().join("case.llbc"),
    );
    assert!(!unsupported.status.success());
    assert!(String::from_utf8_lossy(&unsupported.stderr).contains("loop abstraction"));
    assert!(!tmp.path().join("pcg.json").exists());
}

#[test]
fn direct_call_lifetime_dependency_is_anchored_to_its_normal_return() {
    let (result, _) = live_case("call_ref");
    let p = result["points"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p.get("callee").is_some())
        .unwrap();
    let block = result["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["block"] == p["location"]["block"])
        .unwrap();
    assert!(!block["unwind"].is_null());
    assert!(
        !block["successors"]
            .as_array()
            .unwrap()
            .contains(&block["unwind"])
    );
    let edge = p["phases"][3]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| &a["action"]["AddEdge"]["edge"])
        .find(|e| e["kind"] == "CallAbstraction")
        .unwrap();
    assert_eq!(edge["callee"], p["callee"]);
    assert_eq!(edge["location"], p["location"]);
    assert!(
        p["phases"][3]["borrowck_queries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|q| q["Outlives"]["result"] == true)
    );
    let skipped = result["unindexed_shared_reborrows"].as_array().unwrap();
    assert_eq!(skipped.len(), 1);
    assert!(
        result["loans"]
            .as_array()
            .unwrap()
            .iter()
            .all(|loan| loan["reserve"] != skipped[0])
    );
    let (value, _) = live_case("call_value");
    assert!(
        value["points"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p.get("callee").is_some())
            .all(|p| p["phases"][3]["after"]["edges"]
                .as_array()
                .unwrap()
                .iter()
                .all(|e| e["kind"] != "CallAbstraction"))
    );
}

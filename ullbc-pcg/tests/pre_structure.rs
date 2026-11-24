#![cfg(feature = "driver")]
use charon_lib::{ast::Body, export::CrateData, options::SerializationFormat};
use std::process::Command;

#[test]
fn live_analysis_precedes_nop_removal_and_llbc_reconstruction() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("viewer");
    let llbc = temp.path().join("out.llbc");
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cfg_borrows.rs");
    let charon = std::env::var_os("CHARON_BIN")
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
        });
    let result = Command::new(charon)
        .args([
            "rustc",
            "--export-borrowck",
            "--mir",
            "promoted",
            "--sysroot",
            "default",
            "--dest-file",
        ])
        .arg(&llbc)
        .args([
            "--",
            "--crate-type",
            "lib",
            "--crate-name",
            "placement",
            fixture,
        ])
        .env("CHARON_DRIVER", env!("CARGO_BIN_EXE_ullbc-pcg-driver"))
        .env("ULLBC_PCG_OUTPUT", &output)
        .env("ULLBC_PCG_FUNCTION", "goto_borrow")
        .output()
        .unwrap();
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

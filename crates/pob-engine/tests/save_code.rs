use std::path::PathBuf;

use pob_engine::{Engine, EngineConfig};
use serde_json::{Value, json};

fn pob_root(env: &str, resource: &str) -> Option<PathBuf> {
    if let Ok(r) = std::env::var(env) {
        return Some(PathBuf::from(r));
    }
    // pob-sync output, checked out beside this crate.
    let local = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources").join(resource);

    (local.join("Launch.lua").is_file()).then_some(local)
}

#[test]
fn save_after_code_import() {
    check_save("POB_ROOT", "pob");
}

#[test]
fn save_after_code_import_poe1() {
    check_save("POB_ROOT_POE1", "pob1");
}

fn check_save(env: &str, resource: &str) {
    let Some(root) = pob_root(env, resource) else {
        eprintln!("skipping: no PoB data (run pob-sync or set {env})");
        return;
    };
    let tmp = std::env::temp_dir().join(format!("pob-engine-test-{resource}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let engine = Engine::boot(EngineConfig { pob_root: root, user_dir: tmp.clone(), cache_dir: None, data_dir: None }).expect("boot");

    // The contract is `file: string | null`; an unsaved build reports null.
    let info: Value = engine.call("get_build", &json!(null)).expect("get_build");
    
    assert_eq!(info["file"], json!(null), "unsaved build must report file: null, got {}", info["file"]);

    // Round-trip through PoB code, the way ImportView does.
    let code = engine
        .call("save_build_code", &json!(null))
        .expect("save_build_code")["code"]
        .as_str()
        .expect("code")
        .to_string();

    engine.call("load_build_code", &json!({ "code": code })).expect("load_build_code");

    let info: Value = engine.call("get_build", &json!(null)).expect("get_build");

    assert_eq!(info["file"], json!(null), "a code import leaves the build unsaved, got {}", info["file"]);

    // The Save button's target: write to a path.
    let out = tmp.join("saved.xml");
    let r: Value = engine
        .call("save_build_file", &json!({ "path": out.to_string_lossy() }))
        .expect("save_build_file");
    assert_eq!(r["ok"], json!(true), "save_build_file failed: {r}");
    let written = std::fs::read_to_string(&out).expect("saved build file on disk");

    assert!(written.contains("<PathOfBuilding"), "not a PoB document: {written}");

    let before = engine.call("get_build", &Value::Null).unwrap();
    let failed_path = tmp.join("missing").join("failed.xml");
    assert!(engine.call("save_build_file", &json!({ "path": failed_path })).is_err());
    let after = engine.call("get_build", &Value::Null).unwrap();
    assert_eq!(after["file"], before["file"]);
    assert_eq!(after["name"], before["name"]);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), written);

    engine.call("set_level", &json!({ "level": 42 })).unwrap();
    let expected = engine.call("get_build", &Value::Null).unwrap();
    assert_ne!(expected["level"], before["level"]);
    engine.call("save_build_file", &Value::Null).unwrap();
    let recovered = engine.call("load_build_xml", &json!({
        "xml": written, "path": out, "recoverAfter": 0
    })).unwrap();
    assert_eq!(recovered["level"], expected["level"]);
    let recovered = engine.call("load_build_xml", &json!({
        "xml": written, "path": out, "recoverAfter": 4_102_444_800_000_i64
    })).unwrap();
    assert_eq!(recovered["level"], before["level"]);

    let _ = std::fs::remove_dir_all(&tmp);
}

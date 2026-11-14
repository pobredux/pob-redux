use std::path::PathBuf;

use pob_engine::{Engine, EngineConfig};
use serde_json::{Value, json};

fn pob_root() -> Option<PathBuf> {
    if let Ok(r) = std::env::var("POB_ROOT") {
        return Some(PathBuf::from(r));
    }
    // pob-sync output, checked out beside this crate.
    let local = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/pob");
    
    (local.join("Launch.lua").is_file()).then_some(local)
}

#[test]
fn save_after_code_import() {
    let Some(root) = pob_root() else {
        eprintln!("skipping: no PoB data (run pob-sync or set POB_ROOT)");
        return;
    };
    let tmp = std::env::temp_dir().join(format!("pob-engine-test-{}", std::process::id()));
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

    let _ = std::fs::remove_dir_all(&tmp);
}

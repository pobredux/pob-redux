use std::path::PathBuf;

use pob_engine::{Engine, EngineConfig};
use serde_json::{Value, json};

fn pob_root() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("POB_ROOT") {
        return Some(PathBuf::from(root));
    }
    let local = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/pob");
    local.join("Launch.lua").is_file().then_some(local)
}

#[test]
fn items_report_only_the_slots_they_fit() {
    let Some(root) = pob_root() else {
        eprintln!("skipping: no PoB data (run pob-sync or set POB_ROOT)");
        return;
    };
    let user_dir = std::env::temp_dir().join(format!("pob-engine-slots-test-{}", std::process::id()));
    let engine = Engine::boot(EngineConfig { pob_root: root, user_dir: user_dir.clone(), cache_dir: None, data_dir: None }).expect("boot");

    engine
        .call("equip_item_raw", &json!({ "text": "Rarity: NORMAL\nHunter Hood" }))
        .expect("add helmet");
    engine
        .call("equip_item_raw", &json!({ "text": "Rarity: NORMAL\nGlass Shank" }))
        .expect("add dagger");

    let result: Value = engine.call("get_items", &Value::Null).expect("get_items");
    let items = result["items"].as_array().expect("items array");
    let helmet = items.iter().find(|item| item["baseName"] == "Hunter Hood").expect("helmet");
    let dagger = items.iter().find(|item| item["baseName"] == "Glass Shank").expect("dagger");
    let helmet_slots = helmet["compatibleSlots"].as_array().expect("helmet slots");
    let dagger_slots = dagger["compatibleSlots"].as_array().expect("dagger slots");

    assert!(helmet_slots.contains(&json!("Helmet")));
    assert!(!helmet_slots.contains(&json!("Weapon 1")));
    assert!(dagger_slots.contains(&json!("Weapon 1")));
    assert!(!dagger_slots.contains(&json!("Helmet")));

    let _ = std::fs::remove_dir_all(user_dir);
}

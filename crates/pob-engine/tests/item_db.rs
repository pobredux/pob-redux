use std::path::PathBuf;
use std::time::{Duration, Instant};

use pob_engine::{Engine, EngineConfig, EngineHandle};
use serde_json::{json, Value};

const COUNT: &str = "local n = 0 for _ in pairs(launch.main.uniqueDB.list) do n = n + 1 end return n";
const LOADING: &str = "return (launch.main.uniqueDB.loading or launch.main.rareDB.loading) and true or false";

fn config(name: &str) -> Option<EngineConfig> {
    let root = match std::env::var("POB_ROOT") {
        Ok(root) => PathBuf::from(root),
        Err(_) => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/pob"),
    };
    if !root.join("Launch.lua").is_file() {
        eprintln!("skipping: no PoB data (run pob-sync or set POB_ROOT)");
        return None;
    }
    let user_dir = std::env::temp_dir().join(format!("pob-engine-item-db-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&user_dir);
    Some(EngineConfig { pob_root: root, user_dir, cache_dir: None, data_dir: None })
}

fn uniques(engine: &Engine) -> u64 {
    engine.eval(COUNT).expect("count").as_u64().expect("number")
}

fn loading(engine: &Engine) -> bool {
    engine.eval(LOADING).expect("loading") == json!(true)
}

#[test]
fn boot_parses_no_items_and_frames_finish_the_database() {
    let Some(cfg) = config("frames") else { return };
    let engine = Engine::boot(cfg).expect("boot");
    assert_eq!(uniques(&engine), 0);
    assert!(loading(&engine));

    engine.call("new_build", &json!({ "name": "frames" })).expect("new_build");
    let mut frames = 0;
    while loading(&engine) && frames < 500 {
        engine.call("refresh", &Value::Null).expect("refresh");
        frames += 1;
    }
    assert!(!loading(&engine), "frames must still load the database when nothing holds it");
    assert!(uniques(&engine) > 100);
}

#[test]
fn a_step_holds_the_database_until_a_first_use_finishes_it() {
    let Some(cfg) = config("held") else { return };
    let engine = Engine::boot(cfg).expect("boot");
    assert!(engine.hold_idle_work());
    engine.call("new_build", &json!({ "name": "held" })).expect("new_build");
    for _ in 0..5 {
        engine.call("refresh", &Value::Null).expect("refresh");
    }
    assert_eq!(uniques(&engine), 0, "frames must not parse items while the loader is held");

    assert!(engine.idle_step(), "one step must leave more to parse");
    let after_step = uniques(&engine);
    assert!(after_step > 0);
    for _ in 0..5 {
        engine.call("refresh", &Value::Null).expect("refresh");
    }
    assert_eq!(uniques(&engine), after_step);

    let rows = engine.call("item_db_list", &json!({ "db": "unique" })).expect("item_db_list");
    assert!(!loading(&engine));
    assert!(uniques(&engine) > after_step);
    assert!(rows.to_string().len() > 1000);
    assert!(!engine.idle_step());
}

#[test]
fn idle_work_loads_the_database_between_calls() {
    let Some(cfg) = config("idle") else { return };
    let engine = EngineHandle::spawn_with_idle_work(cfg);
    engine.wait_ready().expect("boot");

    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        std::thread::sleep(Duration::from_millis(600));
        if engine.eval(LOADING).expect("loading") == json!(false) {
            break;
        }
        assert!(Instant::now() < deadline, "idle work did not finish the item database");
    }
    assert!(engine.eval(COUNT).expect("count").as_u64().expect("number") > 100);
}

#[test]
fn a_plain_handle_does_no_idle_work() {
    let Some(cfg) = config("plain") else { return };
    let engine = EngineHandle::spawn(cfg);
    engine.wait_ready().expect("boot");
    engine.call("get_build", Value::Null).expect("get_build");
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(engine.eval(COUNT).expect("count"), json!(0));
}

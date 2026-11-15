use std::path::PathBuf;

use pob_engine::{Engine, EngineConfig};
use reqwest::Url;
use serde_json::{Value, json};

const BOOTS: &str = "Rarity: Rare\nTrade Reference\nRawhide Boots\nImplicits: 0\n+55 to maximum Life\n20% increased Movement Speed";

// Use PoB's real calculator and generator with a small offline trade-stat cache.
const QUERY_MODS: &str = r##"return {
    Explicit = {
        life = {
            Boots = { min = 20, max = 60 },
            specialCaseData = {},
            tradeMod = { id = "explicit.stat_3299347043", text = "+# to maximum Life" },
        },
        speed = {
            Boots = { min = 10, max = 30 },
            specialCaseData = {},
            tradeMod = { id = "explicit.stat_2250533757", text = "#% increased Movement Speed" },
        },
    },
    Implicit = {},
}"##;

fn snapshot(engine: &Engine) -> Value {
    json!({
        "items": engine.call("get_items", &Value::Null).unwrap(),
        "slots": engine.call("list_slots", &Value::Null).unwrap(),
        "weights": engine.call("trade_weights_get", &Value::Null).unwrap(),
        "undo": engine.eval("local t = launch.main.modes.BUILD.itemsTab; return { undo = #t.undo, redo = #t.redo }").unwrap(),
        "popups": engine.eval("return #launch.main.popups").unwrap(),
    })
}

fn parse_link(result: Value, expected_path: &str) -> Value {
    let url = Url::parse(result["tradeUrl"].as_str().unwrap()).unwrap();
    assert_eq!(url.host_str(), Some("www.pathofexile.com"));
    assert_eq!(url.path(), expected_path);
    let query = url.query_pairs().find(|(key, _)| key == "q").unwrap().1;
    serde_json::from_str(&query).unwrap()
}

fn check_trade(game: &str, root_env: &str, resource: &str, expected_path: &str) {
    let root = std::env::var_os(root_env).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../src-tauri/resources")
            .join(resource)
    });
    if !root.join("SYNC.json").is_file() {
        eprintln!("skipping {game}: run pob-sync");
        return;
    }
    let user_dir =
        std::env::temp_dir().join(format!("pob-weighted-trade-{game}-{}", std::process::id()));
    let cache = user_dir.join("pob-redux-cache/Data");
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(cache.join("QueryMods.lua"), QUERY_MODS).unwrap();
    let engine = Engine::boot(EngineConfig {
        pob_root: root,
        user_dir,
        cache_dir: None,
        data_dir: None,
    })
    .unwrap();
    engine
        .call("new_build", &json!({ "name": "Weighted trade test" }))
        .unwrap();
    engine.call("set_level", &json!({ "level": 75 })).unwrap();
    engine
        .call(
            "equip_item_raw",
            &json!({
                "slot": "Boots", "text": "Rarity: Normal\nRawhide Boots"
            }),
        )
        .unwrap();
    let before = snapshot(&engine);

    let mut params = json!({
        "item_text": BOOTS,
        "slot": "Boots",
        "league": "Test League",
        "max_price": 2.5,
        "currency": "divine",
        "stat_weights": [{ "stat": "Life", "weightMult": 1 }]
    });
    engine
        .call(
            "trade_search_start",
            &json!({
                "slotName": "Boots", "statWeights": params["stat_weights"], "maxPrice": 10
            }),
        )
        .unwrap();
    let life = parse_link(
        engine.call("weighted_trade_link", &params).unwrap(),
        expected_path,
    );
    assert_eq!(life["sort"]["statgroup.0"], "desc");
    assert_eq!(
        life["query"]["filters"]["type_filters"]["filters"]["category"]["option"],
        "armour.boots"
    );
    assert_eq!(
        life["query"]["filters"]["trade_filters"]["filters"]["price"],
        json!({ "max": 2.5, "option": "divine" })
    );
    let weighted = &life["query"]["stats"][0];
    assert_eq!(weighted["type"], "weight");
    assert!(weighted["value"]["min"].as_f64().unwrap() > 0.0);
    assert!(weighted["filters"].as_array().unwrap().iter().any(|entry| {
        entry["id"] == "explicit.stat_3299347043"
            && entry["value"]["weight"].as_f64().unwrap() > 0.0
    }));
    assert_eq!(
        engine
            .call("trade_search_step", &json!({ "steps": 150 }))
            .unwrap()["done"],
        true
    );
    let manual = engine.call("trade_search_result", &Value::Null).unwrap();
    let manual: Value = serde_json::from_str(manual["query"].as_str().unwrap()).unwrap();
    assert_eq!(
        manual["query"]["filters"]["trade_filters"]["filters"]["price"]["max"],
        10
    );

    params["stat_weights"][0]["stat"] = json!("EffectiveMovementSpeedMod");
    params["max_price"] = json!(30);
    params["currency"] = json!("equivalent");
    let speed = parse_link(
        engine.call("weighted_trade_link", &params).unwrap(),
        expected_path,
    );
    assert_ne!(weighted["filters"], speed["query"]["stats"][0]["filters"]);
    assert_eq!(
        speed["query"]["filters"]["trade_filters"]["filters"]["price"],
        json!({ "max": 30 })
    );

    for (key, value) in [
        ("slot", json!("Ring 1")),
        ("currency", json!("bad-currency")),
        ("item_text", json!("Rarity: Normal\nRawhide Boots")),
        (
            "stat_weights",
            json!([{ "stat": "NotAStat", "weightMult": 1 }]),
        ),
    ] {
        let mut invalid = params.clone();
        invalid[key] = value;
        assert!(
            engine.call("weighted_trade_link", &invalid).is_err(),
            "{key}"
        );
    }
    assert_eq!(snapshot(&engine), before);
}

#[test]
fn weighted_trade_poe1() {
    check_trade("poe1", "POB_REDUX_POB1_ROOT", "pob1", "/trade/search/Test%20League");
}

#[test]
fn weighted_trade_poe2() {
    check_trade("poe2", "POB_REDUX_POB_ROOT", "pob", "/trade2/search/poe2/Test%20League");
}

use std::path::PathBuf;

use pob_engine::{Engine, EngineConfig};
use serde_json::{Value, json};

const RING: &str = "Rarity: Rare\nPreview Ring\nIron Ring\nItem Level: 80\nImplicits: 0\n+70 to maximum Life\n20% increased Cast Speed";

fn snapshot(engine: &Engine) -> Value {
    // Save first: canonicalization must not count as a preview mutation.
    let xml = engine.call("save_build_xml", &Value::Null).unwrap();
    json!({
        "items": engine.call("get_items", &Value::Null).unwrap(),
        "slots": engine.call("list_slots", &Value::Null).unwrap(),
        "xml": xml,
        "build": engine.call("get_build", &Value::Null).unwrap(),
        "undo": engine.eval("local t = launch.main.modes.BUILD.itemsTab; return { undo = #t.undo, redo = #t.redo }").unwrap(),
    })
}

fn lines(preview: &Value) -> String {
    preview["tooltip"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|l| l["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn preview_is_temporary_and_commit_matches_candidate() {
    let root = std::env::var_os("POB_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/pob")
        });
    if !root.join("Launch.lua").is_file() {
        eprintln!("skipping: run pob-sync or set POB_ROOT");
        return;
    }
    let user_dir = std::env::temp_dir().join(format!("pob-item-preview-{}", std::process::id()));
    let engine = Engine::boot(EngineConfig {
        pob_root: root,
        user_dir: user_dir.clone(),
    })
    .unwrap();
    engine
        .call("new_build", &json!({ "name": "Preview test" }))
        .unwrap();
    let group = engine.call("add_socket_group", &json!({})).unwrap();
    engine
        .call(
            "add_gem",
            &json!({ "groupIndex": group["groupIndex"], "nameSpec": "Fireball", "level": 1 }),
        )
        .unwrap();
    engine
        .call(
            "equip_item_raw",
            &json!({ "text": "Rarity: Normal\nIron Ring", "slot": "Ring 1" }),
        )
        .unwrap();
    let before = snapshot(&engine);
    let generation = before["build"]["generation"].clone();
    let first = engine
        .call(
            "item_preview",
            &json!({ "raw": RING, "generation": generation }),
        )
        .unwrap();
    assert!(lines(&first).contains("Equipping this item in Ring 1"));
    assert!(lines(&first).contains("Equipping this item in Ring 2"));
    assert!(lines(&first).contains("Life"));
    assert!(lines(&first).contains("DPS"));
    assert_eq!(first["slots"].as_array().unwrap().len(), 2);
    assert_eq!(
        snapshot(&engine),
        before,
        "preview must leave build and undo unchanged"
    );

    let legacy = engine.eval(&format!(r#"
        local tt = new("Tooltip"):Tooltip()
        launch.main.modes.BUILD.itemsTab:AddItemTooltip(tt, new("Item"):Item([=[{RING}]=]))
        local lines = {{}}
        for _, line in ipairs(tt.lines) do
            if line.text and not line.text:find("Tip: Hold Shift", 1, true) then lines[#lines + 1] = line.text end
        end
        return table.concat(lines, "\n")
    "#)).unwrap();
    let preview_text = first["tooltip"]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|l| l["text"].as_str())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(preview_text, legacy.as_str().unwrap());

    let edited = RING.replace("+70", "+120");
    let second = engine
        .call(
            "item_preview",
            &json!({ "raw": edited, "generation": generation }),
        )
        .unwrap();
    assert_ne!(lines(&first), lines(&second));
    for raw in ["", "not an item", "Rarity: Rare\nInvalid\nUnknown Base"] {
        assert!(
            engine
                .call(
                    "item_preview",
                    &json!({ "raw": raw, "generation": generation })
                )
                .is_err()
        );
    }
    assert_eq!(
        snapshot(&engine),
        before,
        "editing/invalid previews cannot change the build"
    );
    assert!(
        engine
            .call(
                "equip_item_raw",
                &json!({ "text": edited, "slot": "Helmet" })
            )
            .is_err()
    );
    assert_eq!(
        snapshot(&engine),
        before,
        "failed commit must not insert an item"
    );

    engine
        .call("stat_differences", &json!({ "show": false }))
        .unwrap();
    let hidden = engine
        .call("item_preview", &json!({ "raw": edited }))
        .unwrap();
    assert!(!lines(&hidden).contains("Equipping this item"));
    engine
        .call("stat_differences", &json!({ "show": true }))
        .unwrap();

    let added = engine
        .call(
            "item_edit",
            &json!({ "text": edited, "generation": generation }),
        )
        .unwrap();
    let after = snapshot(&engine);
    assert_eq!(after["items"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(
        after["slots"], before["slots"],
        "Add to build must not equip"
    );
    let raw = engine
        .call("item_raw", &json!({ "itemId": added["itemId"] }))
        .unwrap();
    assert!(
        raw["raw"]
            .as_str()
            .unwrap()
            .contains("+120 to maximum Life")
    );

    let equipped = engine
        .call(
            "equip_item_raw",
            &json!({ "text": edited, "slot": "Ring 2", "generation": generation }),
        )
        .unwrap();
    let slots = engine.call("list_slots", &Value::Null).unwrap();
    assert!(
        slots["slots"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["slot"] == "Ring 2" && s["itemId"] == equipped["itemId"])
    );

    engine
        .call("new_build", &json!({ "name": "Preview test" }))
        .unwrap();
    let replaced = snapshot(&engine);
    for method in ["item_preview", "item_edit", "equip_item_raw"] {
        assert!(engine.call(method, &json!({ "raw": edited, "text": edited, "slot": "Ring 1", "generation": generation })).is_err());
    }
    assert_eq!(snapshot(&engine), replaced);

    let version = engine.call("version", &Value::Null).unwrap();
    let (cases, weapon_base): (&[(&str, bool)], &str) = match version["game"].as_str() {
        Some("poe1") => (
            &[
                ("Rarity: Normal\nSmall Life Flask", false),
                ("Rarity: Normal\nCrimson Jewel", true),
            ],
            "Rusted Hatchet",
        ),
        Some("poe2") => (
            &[
                ("Rarity: Normal\nLesser Life Flask", false),
                ("Rarity: Normal\nThawing Charm", false),
                ("Rarity: Normal\nRuby", true),
            ],
            "Dull Hatchet",
        ),
        other => panic!("unsupported game: {other:?}"),
    };
    for &(raw, jewel) in cases {
        let before = snapshot(&engine);
        let preview = engine
            .call("item_preview", &json!({ "raw": raw }))
            .unwrap_or_else(|e| panic!("previewing {raw:?}: {e}"));
        assert!(!preview["tooltip"]["lines"].as_array().unwrap().is_empty());
        assert_eq!(snapshot(&engine), before);
        if jewel {
            assert!(preview["slots"].as_array().unwrap().is_empty());
            assert!(
                engine
                    .call("equip_item_raw", &json!({ "text": raw }))
                    .is_err()
            );
            assert_eq!(
                snapshot(&engine),
                before,
                "a jewel with no socket must not be inserted on failed equip"
            );
        }
    }
    for (set, suffix) in [(1, ""), (2, " Swap")] {
        engine
            .call("set_weapon_set", &json!({ "set": set }))
            .unwrap();
        let before = snapshot(&engine);
        let weapon = engine
            .call(
                "item_preview",
                &json!({ "raw": format!("Rarity: Normal\n{weapon_base}\nQuality: 0") }),
            )
            .unwrap();
        assert!(!weapon["slots"].as_array().unwrap().is_empty());
        for slot in weapon["slots"].as_array().unwrap() {
            assert_eq!(
                slot["slot"].as_str().unwrap().ends_with(" Swap"),
                suffix == " Swap"
            );
        }
        assert!(
            lines(&weapon).contains("0%"),
            "preview refresh must retain the candidate quality"
        );
        assert_eq!(snapshot(&engine), before);
    }
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn customization_edits_drafts_and_commits_saved_items() {
    let root = std::env::var_os("POB_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/pob")
        });
    if !root.join("Launch.lua").is_file() {
        return;
    }
    let user_dir = std::env::temp_dir().join(format!("pob-item-customize-{}", std::process::id()));
    let engine = Engine::boot(EngineConfig {
        pob_root: root,
        user_dir: user_dir.clone(),
    })
    .unwrap();
    engine
        .call("new_build", &json!({ "name": "Customize test" }))
        .unwrap();
    let before = snapshot(&engine);
    let generation = before["build"]["generation"].clone();
    let mut data = engine
        .call(
            "item_customization",
            &json!({ "raw": RING, "generation": generation }),
        )
        .unwrap();
    for edit in [
        json!({ "operation": "props", "itemLevel": 85, "corrupted": true }),
        json!({ "operation": "props", "corrupted": false }),
        json!({ "operation": "add_modifier", "text": "+(10-20)% to Fire Resistance" }),
    ] {
        let mut params = edit;
        params["raw"] = data["raw"].clone();
        params["generation"] = generation.clone();
        data = engine.call("item_customize", &params).unwrap();
        assert_eq!(snapshot(&engine), before);
    }
    assert_eq!(data["itemLevel"], 85);
    let modifier = data["modifiers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["text"].as_str().unwrap().contains("Fire Resistance"))
        .unwrap()
        .clone();
    assert!(modifier["range"].is_number());
    let rolled = engine
        .call("item_preview", &json!({"raw":data["raw"]}))
        .unwrap();
    assert!(
        lines(&rolled).contains("+15% to Fire Resistance"),
        "preview should show the selected roll, not a database range"
    );
    assert!(!lines(&rolled).contains("(10-20)"));
    let unknown = engine.call("item_customize", &json!({"raw":data["raw"],"operation":"add_modifier","text":"This is not a supported modifier"})).unwrap();
    let unknown_line = unknown["modifiers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["text"] == "This is not a supported modifier")
        .unwrap();
    assert_eq!(unknown_line["parsed"], false);
    let corrected = engine.call("item_customize", &json!({"raw":unknown["raw"],"operation":"modifier","section":unknown_line["section"],"index":unknown_line["index"],"text":"+20 to Strength"})).unwrap();
    assert!(
        corrected["modifiers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["text"] == "+20 to Strength" && m["parsed"] == true)
    );
    assert_eq!(snapshot(&engine), before);

    for patch in [
        json!({"range":1.0}),
        json!({"disabled":true}),
        json!({"disabled":false}),
        json!({"text":"+33% to Fire Resistance"}),
        json!({"remove":true}),
    ] {
        let mut params = json!({ "raw": data["raw"], "generation": generation, "operation":"modifier", "section":modifier["section"], "index":modifier["index"] });
        for (key, value) in patch.as_object().unwrap() {
            params[key] = value.clone();
        }
        data = engine.call("item_customize", &params).unwrap();
        let parsed = engine
            .call("item_customization", &json!({"raw":data["raw"]}))
            .unwrap();
        assert_eq!(
            parsed["modifiers"], data["modifiers"],
            "modifiers must survive text round-trip"
        );
        assert_eq!(snapshot(&engine), before);
    }
    assert!(!data["raw"].as_str().unwrap().contains("Fire Resistance"));
    let options = engine
        .call(
            "item_modifier_options",
            &json!({"raw":data["raw"],"source":"Prefix","query":"maximum Life"}),
        )
        .unwrap();
    let option = &options["options"][0]["id"];
    assert!(option.is_string(), "expected real prefix choices");
    data = engine
        .call(
            "item_customize",
            &json!({"raw":data["raw"],"operation":"add_modifier","modId":option}),
        )
        .unwrap();
    assert_eq!(snapshot(&engine), before);
    for invalid in [
        json!({"operation":"add_modifier","modId":"does-not-exist"}),
        json!({"operation":"modifier","section":"explicit","index":999,"remove":true}),
        json!({"operation":"add_modifier","text":"+10 to maximum Life\nRarity: Rare"}),
        json!({"operation":"rune","index":1,"name":"does-not-exist"}),
    ] {
        let mut p = invalid;
        p["raw"] = data["raw"].clone();
        assert!(engine.call("item_customize", &p).is_err());
        assert_eq!(snapshot(&engine), before);
    }
    let crafted =
        "Rarity: Rare\nCrafted candidate\nIron Ring\nCrafted: true\nItem Level: 80\nImplicits: 0";
    let crafted_data = engine
        .call("item_customization", &json!({"raw":crafted}))
        .unwrap();
    assert_eq!(crafted_data["affixes"]["crafted"], true);
    let poe1 = engine.call("version", &Value::Null).unwrap()["game"] == "poe1";
    let paired_base = if poe1 { "Iron Ring" } else { "Prismatic Ring" };
    let paired_raw = format!(
        "Rarity: Rare\nPaired roll\n{paired_base}\nCrafted: true\nItem Level: 80\nPrefix: {{range:0.1,0.9}}AddedPhysicalDamage2\nImplicits: 0"
    );
    let paired = engine
        .call("item_customization", &json!({"raw":paired_raw}))
        .unwrap();
    let paired_slot = &paired["affixes"]["prefixes"][0];
    assert_eq!(paired_slot["range"], 0.1);
    assert_eq!(paired_slot["rangeIsTable"], true);
    assert_eq!(
        paired_slot["value"],
        if poe1 { "Adds 2 to 5 Physical Damage to Attacks" } else { "Adds 2 to 6 Physical Damage to Attacks" }
    );
    assert!(paired["raw"].as_str().unwrap().contains("{range:0.1,0.9}"));
    let edited = engine
        .call("item_customize", &json!({
            "raw":paired["raw"], "operation":"affix", "table":"prefixes", "index":1,
            "modId":"AddedPhysicalDamage2", "range":0.4
        }))
        .unwrap();
    assert_eq!(edited["affixes"]["prefixes"][0]["range"], 0.4);
    assert_eq!(edited["affixes"]["prefixes"][0]["rangeIsTable"], false);
    assert!(edited["raw"].as_str().unwrap().contains("{range:0.4}AddedPhysicalDamage2"));
    assert_eq!(snapshot(&engine), before);
    let ranged_group = if poe1 { "IncreasedLife" } else { "IncreasedAccuracy" };
    let ranged_affix = crafted_data["affixes"]["prefixes"][0]["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|option| {
            option["modIds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id.as_str().unwrap().starts_with(ranged_group))
        })
        .unwrap();
    let rolls = engine.call("item_affix_rolls", &json!({
        "raw": crafted, "table": "prefixes", "index": 1, "seriesId": ranged_affix["id"]
    })).unwrap();
    let ranged_tiers = rolls["tiers"].as_array().unwrap();
    assert!(ranged_tiers.len() > 1);
    assert_eq!(
        ranged_tiers.iter().map(|tier| &tier["modId"]).collect::<Vec<_>>(),
        ranged_affix["modIds"].as_array().unwrap().iter().collect::<Vec<_>>()
    );
    let low_level = crafted.replace("Item Level: 80", "Item Level: 1");
    let low_level_rolls = engine.call("item_affix_rolls", &json!({
        "raw": low_level, "table": "prefixes", "index": 1, "seriesId": ranged_affix["id"]
    })).unwrap();
    assert_eq!(low_level_rolls["tiers"].as_array().unwrap().len(), ranged_tiers.len());
    assert_eq!(ranged_tiers[0]["tier"], ranged_tiers.len());
    assert_eq!(ranged_tiers.last().unwrap()["tier"], 1);
    assert!(ranged_tiers.iter().all(|tier| tier["steps"].as_array().unwrap().len() > 1));
    let value_fragment = if poe1 { "maximum Life" } else { "Accuracy Rating" };
    assert!(ranged_tiers[0]["steps"][0]["value"].as_str().unwrap().contains(value_fragment));
    let amulet = "Rarity: Rare\nAffix candidate\nJade Amulet\nCrafted: true\nItem Level: 80\nImplicits: 0";
    let amulet_data = engine.call("item_customization", &json!({"raw": amulet})).unwrap();
    let special = if poe1 {
        "Rarity: Rare\nAffix candidate\nJade Amulet\nElder Item\nCrafted: true\nItem Level: 80\nImplicits: 0"
    } else {
        "Rarity: Rare\nAffix candidate\nTime-Lost Ruby\nCrafted: true\nItem Level: 80\nImplicits: 0"
    };
    let special_data = engine.call("item_customization", &json!({"raw": special})).unwrap();
    let special_series = special_data["affixes"]["prefixes"][0]["options"].as_array().unwrap();
    let separate_mods = if poe1 {
        ["MaximumZombiesUber1", "MaximumSkeletonsUber1"]
    } else {
        ["JewelRadiusMediumSize", "JewelRadiusLargeSize"]
    };
    for mod_id in separate_mods {
        let series = special_series
            .iter()
            .find(|series| series["modIds"] == json!([mod_id]))
            .unwrap();
        let rolls = engine.call("item_affix_rolls", &json!({
            "raw": special, "table": "prefixes", "index": 1, "seriesId": series["id"]
        })).unwrap();
        assert_eq!(rolls["tiers"].as_array().unwrap().len(), 1);
        assert_eq!(rolls["tiers"][0]["modId"], mod_id);
    }
    let retained_mod = if poe1 { "MaximumZombiesUber1" } else { "JewelRadiusMediumSize" };
    let retained_base = if poe1 { "Jade Amulet" } else { "Ruby" };
    let retained = format!(
        "Rarity: Rare\nRetained affix\n{retained_base}\nCrafted: true\nItem Level: 80\nPrefix: {{range:0.5}}{retained_mod}\nImplicits: 0"
    );
    let retained_data = engine.call("item_customization", &json!({"raw": retained})).unwrap();
    let retained_slot = &retained_data["affixes"]["prefixes"][0];
    assert_eq!(retained_slot["modId"], retained_mod);
    assert_eq!(
        retained_slot["options"].as_array().unwrap().iter().any(|series| {
            series["modIds"].as_array().unwrap().contains(&json!(retained_mod))
        }),
        poe1
    );
    let removed = engine.call("item_customize", &json!({
        "raw": retained, "operation": "affix", "table": "prefixes", "index": 1, "modId": "None"
    })).unwrap();
    assert_eq!(removed["affixes"]["prefixes"][0]["modId"], "None");
    let discrete_group = if poe1 { "LifeGainPerTarget" } else { "GlobalSpellGemsLevel" };
    let discrete_affix = amulet_data["affixes"]["suffixes"][0]["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|option| {
            option["modIds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id.as_str().unwrap().starts_with(discrete_group))
        })
        .unwrap();
    let discrete = engine.call("item_affix_rolls", &json!({
        "raw": amulet, "table": "suffixes", "index": 1, "seriesId": discrete_affix["id"]
    })).unwrap();
    assert!(discrete["tiers"].as_array().unwrap().len() > 1);
    assert!(discrete["tiers"].as_array().unwrap().iter().all(|tier| tier["steps"].as_array().unwrap().len() == 1));
    let affix = &crafted_data["affixes"]["prefixes"][0]["options"][0]["modIds"][0];
    assert!(affix.is_string());
    let crafted_data=engine.call("item_customize",&json!({"raw":crafted,"operation":"affix","table":"prefixes","index":1,"modId":affix,"range":1.0})).unwrap();
    assert_eq!(crafted_data["affixes"]["prefixes"][0]["modId"], *affix);
    assert!(crafted_data["affixes"]["prefixes"][0]["value"].is_string());
    assert_eq!(snapshot(&engine), before);

    let variant = "Rarity: Rare\nVariant candidate\nIron Ring\nVariant: Small\nVariant: Large\nSelected Variant: 1\nImplicits: 0\n{variant:1}+10 to maximum Life\n{variant:2}+90 to maximum Life";
    let variants = engine
        .call(
            "item_customize",
            &json!({"raw":variant,"operation":"variant","picks":[2]}),
        )
        .unwrap();
    assert_eq!(variants["variants"]["picks"][0], 2);
    assert_eq!(snapshot(&engine), before);

    let weapon = "Rarity: Normal\nCrude Bow\nQuality: 0";
    let weapon = engine
        .call(
            "item_customize",
            &json!({"raw":weapon,"operation":"props","quality":7}),
        )
        .unwrap();
    assert_eq!(weapon["quality"], 7);
    let normalized = engine
        .call(
            "item_customize",
            &json!({"raw":weapon["raw"],"operation":"normalize"}),
        )
        .unwrap();
    assert!(normalized["quality"].as_i64().unwrap() > 7);
    if weapon["runeSocketLimit"].as_u64().unwrap() > 0 {
        let socketed = engine
            .call(
                "item_customize",
                &json!({"raw":weapon["raw"],"operation":"rune_sockets","count":1}),
            )
            .unwrap();
        assert_eq!(socketed["runes"]["socketCount"], 1);
        let rune = socketed["runes"]["options"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] != "None")
            .unwrap();
        let runed = engine
            .call(
                "item_customize",
                &json!({"raw":socketed["raw"],"operation":"rune","index":1,"name":rune["name"]}),
            )
            .unwrap();
        assert_eq!(runed["runes"]["runes"][0], rune["name"]);
    }
    assert_eq!(snapshot(&engine), before);

    let added = engine
        .call("item_edit", &json!({"text":data["raw"]}))
        .unwrap();
    let saved_before = snapshot(&engine);
    let saved=engine.call("item_customize",&json!({"itemId":added["itemId"],"generation":generation,"operation":"props","itemLevel":99})).unwrap();
    assert_eq!(saved["itemLevel"], 99);
    let saved_after = snapshot(&engine);
    assert_eq!(saved_after["items"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        saved_after["undo"]["undo"].as_u64().unwrap(),
        saved_before["undo"]["undo"].as_u64().unwrap() + 1
    );
    assert_ne!(saved_before["xml"], saved_after["xml"]);
    let amulet = "Rarity: Rare\nCandidate amulet\nJade Amulet\nImplicits: 0";
    let anoints = engine
        .call("item_anoints", &json!({"raw":amulet,"withNodes":true}))
        .unwrap();
    let node = anoints["nodes"][0]["name"].as_str().unwrap();
    let source = format!("{amulet}\n{{enchant}}Allocates {node}");
    engine
        .call("equip_item_raw", &json!({"text":source,"slot":"Amulet"}))
        .unwrap();
    let before_copy = snapshot(&engine);
    let copied = engine
        .call(
            "item_customize",
            &json!({"raw":amulet,"operation":"copy_anoints","sourceSlot":"Amulet"}),
        )
        .unwrap();
    assert!(
        copied["raw"]
            .as_str()
            .unwrap()
            .contains(&format!("Allocates {node}"))
    );
    assert_eq!(snapshot(&engine), before_copy);
    if weapon["runeSocketLimit"].as_u64().unwrap() > 0 {
        let socketed = engine
            .call(
                "item_customize",
                &json!({"raw":weapon["raw"],"operation":"rune_sockets","count":1}),
            )
            .unwrap();
        let rune = socketed["runes"]["options"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] != "None")
            .unwrap();
        let runed = engine
            .call(
                "item_customize",
                &json!({"raw":socketed["raw"],"operation":"rune","index":1,"name":rune["name"]}),
            )
            .unwrap();
        engine
            .call(
                "equip_item_raw",
                &json!({"text":runed["raw"],"slot":"Weapon 1"}),
            )
            .unwrap();
        let before_copy = snapshot(&engine);
        let copied = engine
            .call(
                "item_customize",
                &json!({"raw":weapon["raw"],"operation":"copy_augments","sourceSlot":"Weapon 1"}),
            )
            .unwrap();
        assert_eq!(copied["runes"]["runes"][0], rune["name"]);
        assert_eq!(snapshot(&engine), before_copy);
    }
    engine.call("new_build", &json!({})).unwrap();
    assert!(
        engine
            .call(
                "item_customize",
                &json!({"raw":data["raw"],"generation":generation,"operation":"props","quality":20})
            )
            .is_err()
    );
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn affix_family_edit_chooses_and_returns_rolls_in_one_command() {
    let root = std::env::var_os("POB_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/pob")
        });
    if !root.join("Launch.lua").is_file() {
        return;
    }
    let user_dir = std::env::temp_dir().join(format!("pob-affix-family-edit-{}", std::process::id()));
    let engine = Engine::boot(EngineConfig {
        pob_root: root,
        user_dir: user_dir.clone(),
    })
    .unwrap();
    engine.call("new_build", &json!({})).unwrap();
    let before = snapshot(&engine);
    let crafted = "Rarity: Rare\nFamily edit\nIron Ring\nCrafted: true\nItem Level: 80\nImplicits: 0";
    let data = engine
        .call("item_customization", &json!({"raw":crafted}))
        .unwrap();
    assert!(data["affixes"]["prefixes"][0]["rolls"].is_null());
    let poe1 = engine.call("version", &Value::Null).unwrap()["game"] == "poe1";
    let prefix = if poe1 {
        "IncreasedLife"
    } else {
        "IncreasedAccuracy"
    };
    let ranged_family = data["affixes"]["prefixes"][0]["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|option| {
            option["modIds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id.as_str().unwrap().starts_with(prefix))
        })
        .unwrap();
    let mut raw = json!(crafted);
    for position in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let edited = engine
            .call("item_customize", &json!({
                "raw":raw, "operation":"affix", "table":"prefixes", "index":1,
                "seriesId":ranged_family["id"], "relativePosition":position
            }))
            .unwrap();
        let tiers = edited["affixes"]["prefixes"][0]["rolls"]["tiers"].as_array().unwrap();
        let scaled = position * tiers.len() as f64;
        let tier_index = (scaled.ceil() as usize).max(1) - 1;
        let tier = &tiers[tier_index];
        let range = scaled - tier_index as f64;
        let expected_range = if tier["flipped"] == true { 1.0 - range } else { range };
        assert_eq!(edited["affixes"]["prefixes"][0]["rolls"]["seriesId"], ranged_family["id"]);
        assert_eq!(edited["affixes"]["prefixes"][0]["modId"], tier["modId"]);
        assert!((edited["affixes"]["prefixes"][0]["range"].as_f64().unwrap() - expected_range).abs() < 1e-9);
        let reloaded = engine.call("item_customization", &json!({"raw":edited["raw"]})).unwrap();
        assert!((reloaded["affixes"]["prefixes"][0]["range"].as_f64().unwrap() - expected_range).abs() < 1e-9);
        assert_eq!(
            reloaded["affixes"]["prefixes"][0]["rolls"]["tiers"],
            edited["affixes"]["prefixes"][0]["rolls"]["tiers"]
        );
        assert_eq!(snapshot(&engine), before);
        raw = edited["raw"].clone();
    }
    for (series, position) in [(ranged_family["id"].clone(), 1.1), (json!("missing"), 0.5)] {
        assert!(engine.call("item_customize", &json!({
            "raw":raw, "operation":"affix", "table":"prefixes", "index":1,
            "seriesId":series, "relativePosition":position
        })).is_err());
    }
    assert_eq!(snapshot(&engine), before);

    let amulet = "Rarity: Rare\nDiscrete roll\nJade Amulet\nCrafted: true\nItem Level: 80\nImplicits: 0";
    let amulet_data = engine
        .call("item_customization", &json!({"raw":amulet}))
        .unwrap();
    let discrete = if poe1 {
        "LifeGainPerTarget"
    } else {
        "GlobalSpellGemsLevel"
    };
    let discrete_family = amulet_data["affixes"]["suffixes"][0]["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|option| {
            option["modIds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id.as_str().unwrap().starts_with(discrete))
        })
        .unwrap();
    let discrete_edit = engine.call("item_customize", &json!({
        "raw":amulet, "operation":"affix", "table":"suffixes", "index":1,
        "seriesId":discrete_family["id"], "relativePosition":0.5
    })).unwrap();
    let tiers = discrete_edit["affixes"]["suffixes"][0]["rolls"]["tiers"].as_array().unwrap();
    assert!(tiers.iter().all(|tier| tier["steps"].as_array().unwrap().len() == 1));
    assert!(tiers
        .iter()
        .any(|tier| tier["modId"] == discrete_edit["affixes"]["suffixes"][0]["modId"]));
    assert_eq!(snapshot(&engine), before);

    let saved = engine.call("item_edit", &json!({"text":raw})).unwrap();
    let edited = engine
        .call("item_customize", &json!({
            "itemId":saved["itemId"], "operation":"affix", "table":"prefixes", "index":1,
            "seriesId":ranged_family["id"], "relativePosition":0.0
        }))
        .unwrap();
    assert_eq!(
        edited["affixes"]["prefixes"][0]["modId"],
        edited["affixes"]["prefixes"][0]["rolls"]["tiers"][0]["modId"]
    );
    let first_tier = &edited["affixes"]["prefixes"][0]["rolls"]["tiers"][0];
    let expected_range = if first_tier["flipped"] == true { 1.0 } else { 0.0 };
    assert_eq!(edited["affixes"]["prefixes"][0]["range"].as_f64().unwrap(), expected_range);
    assert_eq!(
        edited["raw"],
        engine.call("item_customization", &json!({"itemId":saved["itemId"]})).unwrap()["raw"]
    );
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn advanced_customization_has_saved_and_draft_parity() {
    let root = std::env::var_os("POB_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/pob")
        });
    if !root.join("Launch.lua").is_file() {
        return;
    }
    let user_dir = std::env::temp_dir().join(format!("pob-item-parity-{}", std::process::id()));
    let engine = Engine::boot(EngineConfig {
        pob_root: root,
        user_dir: user_dir.clone(),
    })
    .unwrap();
    engine
        .call("new_build", &json!({"name":"Advanced customization"}))
        .unwrap();
    let generation = engine.call("get_build", &Value::Null).unwrap()["generation"].clone();
    let amulet = "Rarity: Rare\nParity Amulet\nJade Amulet\nImplicits: 0\n+40 to maximum Life";
    let anoints = engine
        .call("item_anoints", &json!({"raw":amulet,"withNodes":true}))
        .unwrap();
    let node = &anoints["nodes"][0];
    let mut cases = vec![
        (
            amulet.to_owned(),
            json!({"operation":"anoint","nodeId":node["id"],"slot":1}),
        ),
        (
            format!(
                "{amulet}\n{{enchant}}Allocates {}",
                node["name"].as_str().unwrap()
            ),
            json!({"operation":"anoint","nodeId":null,"slot":1}),
        ),
    ];
    let corruption = engine
        .call("item_corruptions", &json!({"raw":amulet}))
        .unwrap();
    assert!(!corruption["mods"].as_array().unwrap().is_empty());
    cases.push((
        amulet.to_owned(),
        json!({"operation":"corruption","modIds":[corruption["mods"][0]["id"]]}),
    ));
    let unique = "Rarity: Unique\nParity Unique\nIron Ring\nImplicits: 0\n{range:0.5}+(10-20) to maximum Life";
    cases.push((
        unique.to_owned(),
        json!({"operation":"corruption","ranges":[{"index":1,"value":1.22}]}),
    ));
    let weapon =
        "Rarity: Rare\nParity Bow\nCrude Bow\nItem Level: 85\nImplicits: 0\n+20 to Dexterity";
    let detail = engine
        .call("item_customization", &json!({"raw":weapon}))
        .unwrap();
    let poe1 = detail["shape"]["socketLimit"].as_u64().unwrap() > 0;
    if poe1 {
        cases.push((weapon.to_owned(), json!({"operation":"shape","influences":["shaper","elder"],"sockets":[{"colour":"R","group":0},{"colour":"G","group":0},{"colour":"B","group":1}]})));
        let crucible = &detail["crucible"];
        assert_eq!(crucible["available"], true);
        let mut selected = vec![json!(""); 5];
        let (i, options) = crucible["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .find(|(_, o)| !o.as_array().unwrap().is_empty())
            .unwrap();
        selected[i] = options[0]["id"].clone();
        cases.push((
            weapon.to_owned(),
            json!({"operation":"crucible","selected":selected}),
        ));
        let cluster =
            "Rarity: Rare\nParity Cluster\nLarge Cluster Jewel\nItem Level: 85\nImplicits: 0";
        let shape = engine.call("item_shape", &json!({"raw":cluster})).unwrap();
        cases.push((cluster.to_owned(), json!({"operation":"shape","clusterSkill":shape["cluster"]["skills"][0]["id"],"clusterNodeCount":shape["cluster"]["minNodes"]})));
        let boots = "Rarity: Rare\nParity Boots\nRawhide Boots\nImplicits: 0\n+20 to maximum Life";
        let enchants = engine.call("item_enchants", &json!({"raw":boots})).unwrap();
        assert_eq!(enchants["available"], true);
        cases.push((boots.to_owned(), json!({"operation":"enchant","line":enchants["lines"][0],"slot":1,"source":enchants["source"]})));
        let enchanted = engine.call("item_customize", &json!({"raw":boots,"operation":"enchant","line":enchants["lines"][0],"slot":1,"source":enchants["source"]})).unwrap();
        cases.push((
            enchanted["raw"].as_str().unwrap().to_owned(),
            json!({"operation":"enchant","remove":true,"slot":1}),
        ));
    } else {
        assert_eq!(detail["shape"]["socketLimit"], 0);
        assert_eq!(detail["crucible"]["available"], false);
    }
    for (raw, edit) in cases {
        let added = engine.call("item_edit", &json!({"text":raw})).unwrap();
        let before = snapshot(&engine);
        let mut draft_params = edit.clone();
        draft_params["raw"] = json!(raw);
        draft_params["generation"] = generation.clone();
        let draft = engine
            .call("item_customize", &draft_params)
            .unwrap_or_else(|e| panic!("{edit}: {e}"));
        assert_eq!(
            snapshot(&engine),
            before,
            "draft operation {edit} changed build state"
        );
        let original = engine
            .call("item_customization", &json!({"raw":raw}))
            .unwrap();
        assert_ne!(
            draft["raw"], original["raw"],
            "{edit} did not edit the candidate"
        );
        match edit["operation"].as_str().unwrap() {
            "anoint" if edit["nodeId"].is_null() => {
                assert!(draft["anoints"]["current"].as_array().unwrap().is_empty())
            }
            "anoint" => assert_eq!(draft["anoints"]["current"][0], node["name"]),
            "corruption" => {
                assert_eq!(draft["corrupted"], true);
                if !edit["ranges"].is_null() {
                    assert_eq!(draft["corruptions"]["ranges"][0]["current"], 1.22);
                }
            }
            "shape" if !edit["sockets"].is_null() => {
                assert_eq!(draft["shape"]["sockets"], edit["sockets"]);
                assert_eq!(
                    draft["shape"]["influences"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|i| i["on"] == true)
                        .count(),
                    2
                );
            }
            "shape" => {
                assert_eq!(draft["shape"]["cluster"]["skill"], edit["clusterSkill"]);
                assert_eq!(
                    draft["shape"]["cluster"]["nodeCount"],
                    edit["clusterNodeCount"]
                );
            }
            "crucible" => assert_eq!(draft["crucible"]["selected"], edit["selected"]),
            "enchant" => {
                let info = engine
                    .call("item_enchants", &json!({"raw":draft["raw"]}))
                    .unwrap();
                assert_eq!(
                    info["current"].as_array().unwrap().is_empty(),
                    edit["remove"] == true
                );
            }
            _ => unreachable!(),
        }
        engine
            .call(
                "item_preview",
                &json!({"raw":draft["raw"],"generation":generation}),
            )
            .unwrap();
        assert_eq!(snapshot(&engine), before);
        let roundtrip = engine
            .call("item_customization", &json!({"raw":draft["raw"]}))
            .unwrap();
        assert_eq!(draft, roundtrip, "{edit} did not survive raw serialization");
        let mut saved_params = edit.clone();
        saved_params["itemId"] = added["itemId"].clone();
        saved_params["generation"] = generation.clone();
        let saved = engine.call("item_customize", &saved_params).unwrap();
        assert_eq!(
            saved["raw"], draft["raw"],
            "saved/draft mismatch for {edit}"
        );
        let after = snapshot(&engine);
        assert_eq!(
            after["items"]["items"].as_array().unwrap().len(),
            before["items"]["items"].as_array().unwrap().len()
        );
        assert_eq!(
            after["undo"]["undo"].as_u64().unwrap(),
            before["undo"]["undo"].as_u64().unwrap() + 1
        );
        let committed = engine
            .call(
                "item_edit",
                &json!({"text":draft["raw"],"generation":generation}),
            )
            .unwrap();
        let committed = engine
            .call("item_customization", &json!({"itemId":committed["itemId"]}))
            .unwrap();
        assert_eq!(committed["raw"], draft["raw"]);
    }
    let added = engine.call("item_edit", &json!({"text":amulet})).unwrap();
    for edit in [
        json!({"operation":"anoint","nodeId":-1,"slot":1}),
        json!({"operation":"anoint","nodeId":node["id"],"slot":99}),
        json!({"operation":"corruption","modIds":["not-a-mod"]}),
        json!({"operation":"corruption","ranges":[{"index":1,"value":99}]}),
        json!({"operation":"enchant","line":"not-an-enchant","slot":1}),
        json!({"operation":"crucible","selected":["not-a-node"]}),
    ] {
        for target in [json!({"raw":amulet}), json!({"itemId":added["itemId"]})] {
            let before = snapshot(&engine);
            let mut params = edit.clone();
            params
                .as_object_mut()
                .unwrap()
                .extend(target.as_object().unwrap().clone());
            assert!(engine.call("item_customize", &params).is_err(), "{params}");
            assert_eq!(
                snapshot(&engine),
                before,
                "invalid edit {params} changed build state"
            );
        }
    }
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn crafted_customization_preserves_custom_edits_across_affix_changes() {
    let root = std::env::var_os("POB_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/pob")
        });
    if !root.join("Launch.lua").is_file() {
        return;
    }
    let user_dir =
        std::env::temp_dir().join(format!("pob-crafted-modifiers-{}", std::process::id()));
    let engine = Engine::boot(EngineConfig {
        pob_root: root,
        user_dir: user_dir.clone(),
    })
    .unwrap();
    engine.call("new_build", &json!({})).unwrap();
    let raw =
        "Rarity: Rare\nCrafted candidate\nIron Ring\nCrafted: true\nItem Level: 80\nImplicits: 0";
    let initial = engine
        .call("item_customization", &json!({"raw":raw}))
        .unwrap();
    let prefix = &initial["affixes"]["prefixes"][0]["options"][0]["modIds"][0];
    let initial = engine
        .call(
            "item_customize",
            &json!({"raw":raw,"operation":"affix","table":"prefixes","index":1,"modId":prefix}),
        )
        .unwrap();
    assert!(
        initial["modifiers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["section"] != "explicit")
    );
    let suffix = initial["affixes"]["suffixes"][0]["options"][0]["modIds"][0].clone();
    for saved in [false, true] {
        let item_id = if saved {
            engine
                .call("item_edit", &json!({"text":initial["raw"]}))
                .unwrap()["itemId"]
                .clone()
        } else {
            Value::Null
        };
        let before = snapshot(&engine);
        for change in [
            json!({"remove":true}),
            json!({"disabled":true}),
            json!({"text":"+1 to maximum Life"}),
            json!({"range":0.1}),
        ] {
            let mut p = json!({"operation":"modifier","section":"explicit","index":1});
            if saved {
                p["itemId"] = item_id.clone();
            } else {
                p["raw"] = initial["raw"].clone();
            }
            p.as_object_mut()
                .unwrap()
                .extend(change.as_object().unwrap().clone());
            assert!(
                engine
                    .call("item_customize", &p)
                    .unwrap_err()
                    .to_string()
                    .contains("affix controls")
            );
            assert_eq!(snapshot(&engine), before);
        }
        let mut data = initial.clone();
        let edit = |data: &Value, mut p: Value| {
            if saved {
                p["itemId"] = item_id.clone();
            } else {
                p["raw"] = data["raw"].clone();
            }
            let result = engine.call("item_customize", &p).unwrap();
            if !saved {
                assert_eq!(snapshot(&engine), before);
            }
            result
        };
        data = edit(
            &data,
            json!({"operation":"add_modifier","text":"+10 to Strength"}),
        );
        let index = data["modifiers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["text"] == "+10 to Strength")
            .unwrap()["index"]
            .clone();
        data = edit(
            &data,
            json!({"operation":"modifier","section":"explicit","index":index,"text":"+27 to Dexterity","disabled":true}),
        );
        data = edit(
            &data,
            json!({"operation":"affix","table":"suffixes","index":1,"modId":suffix}),
        );
        let custom = data["modifiers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["text"] == "+27 to Dexterity")
            .unwrap();
        assert_eq!(custom["disabled"], true);
        assert_eq!(
            data["modifiers"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|m| m["section"] == "explicit")
                .count(),
            1
        );
        let index = custom["index"].clone();
        data = edit(
            &data,
            json!({"operation":"modifier","section":"explicit","index":index,"remove":true}),
        );
        data = edit(
            &data,
            json!({"operation":"affix","table":"suffixes","index":1,"modId":"None"}),
        );
        assert!(
            data["modifiers"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m["section"] != "explicit")
        );
        assert!(!data["raw"].as_str().unwrap().contains("+27 to Dexterity"));
        let roundtrip = engine
            .call("item_customization", &json!({"raw":data["raw"]}))
            .unwrap();
        assert_eq!(data["raw"], roundtrip["raw"]);
    }
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

fn prepared_like_legacy(engine: &Engine, raw: &str, normalise: bool) -> Value {
    let before = snapshot(engine);
    let prepared = engine
        .call(
            "item_prepare_preview",
            &json!({"raw":raw,"normalise":normalise}),
        )
        .unwrap();
    let preview = engine.call("item_preview", &prepared).unwrap();
    assert_eq!(
        snapshot(engine),
        before,
        "paste preparation changed the build"
    );
    let legacy = engine.eval(&format!(r#"
        local tab = launch.main.modes.BUILD.itemsTab
        local previous = tab.displayItem
        tab:CreateDisplayItemFromRaw([=[{raw}]=], {normalise})
        local raw = tab.displayItem:BuildRaw()
        local lines = {{}}
        for _, line in ipairs(tab.displayItemTooltip.lines) do
            if line.text and not line.text:find("Tip: Hold Shift", 1, true) then lines[#lines + 1] = line.text end
        end
        tab:SetDisplayItem(previous)
        return {{ raw = raw, tooltip = table.concat(lines, "\n") }}
    "#)).unwrap();
    assert_eq!(prepared["raw"], legacy["raw"]);
    assert_eq!(
        lines(&preview)
            .lines()
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        legacy["tooltip"].as_str().unwrap()
    );
    engine.call("item_customization", &prepared).unwrap()
}

#[test]
fn paste_defaults_match_legacy() {
    let root = std::env::var_os("POB_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/pob")
        });
    if !root.join("Launch.lua").is_file() {
        return;
    }
    let user_dir = std::env::temp_dir().join(format!("pob-paste-defaults-{}", std::process::id()));
    let engine = Engine::boot(EngineConfig {
        pob_root: root,
        user_dir: user_dir.clone(),
    })
    .unwrap();
    engine.call("new_build", &json!({})).unwrap();
    let poe2 = engine.call("version", &Value::Null).unwrap()["game"] == "poe2";
    let base = if poe2 { "Full Plate" } else { "Plate Vest" };
    let armour = format!(
        "Rarity: Rare\nPaste Armour\n{base}\nQuality: +7%\nItem Level: 80\nImplicits: 0\n+70 to maximum Life"
    );
    engine.eval("launch.main.defaultItemQuality = 15").unwrap();
    let prepared = prepared_like_legacy(&engine, &armour, true);
    assert_eq!(prepared["quality"], if poe2 { 15 } else { 20 });
    assert_eq!(prepared_like_legacy(&engine, &armour, false)["quality"], 7);
    for suffix in ["\nCorrupted", "\nMirrored"] {
        assert_eq!(
            prepared_like_legacy(&engine, &format!("{armour}{suffix}"), true)["quality"],
            7
        );
    }
    prepared_like_legacy(&engine, &armour.replace("+7%", "+30%"), true);

    let source = if poe2 {
        let socketed = engine
            .call(
                "item_customize",
                &json!({"raw":armour,"operation":"rune_sockets","count":1}),
            )
            .unwrap();
        let rune = socketed["runes"]["options"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] != "None")
            .unwrap();
        engine
            .call(
                "item_customize",
                &json!({"raw":socketed["raw"],"operation":"rune","index":1,"name":rune["name"]}),
            )
            .unwrap()["raw"]
            .as_str()
            .unwrap()
            .to_owned()
    } else {
        format!(
            "Rarity: Rare\nSource Armour\n{base}\nSearing Exarch Item\nImplicits: 1\n+12% to Fire Resistance\n+90 to maximum Life"
        )
    };
    engine
        .call(
            "equip_item_raw",
            &json!({"text":source,"slot":"Body Armour"}),
        )
        .unwrap();
    let migration = if poe2 {
        "migrateAugments"
    } else {
        "migrateEldritchImplicits"
    };
    engine
        .eval(&format!("launch.main.{migration} = true"))
        .unwrap();
    let migrated = prepared_like_legacy(&engine, &armour, true);
    if poe2 {
        assert_ne!(migrated["runes"]["runes"][0], "None");
        assert_eq!(migrated["runes"]["runes"].as_array().unwrap().len(), 1);
    } else {
        assert!(
            migrated["raw"]
                .as_str()
                .unwrap()
                .contains("Searing Exarch Item")
        );
        assert!(
            migrated["raw"]
                .as_str()
                .unwrap()
                .contains("+12% to Fire Resistance")
        );
    }
    prepared_like_legacy(&engine, &source, true);
    prepared_like_legacy(&engine, &armour, false);
    prepared_like_legacy(&engine, &format!("{armour}\nCorrupted"), true);
    engine
        .eval(&format!("launch.main.{migration} = false"))
        .unwrap();
    let unmigrated = prepared_like_legacy(&engine, &armour, true);
    assert_ne!(migrated["raw"], unmigrated["raw"]);

    let amulet = "Rarity: Rare\nPaste Amulet\nJade Amulet\nImplicits: 0\n+40 to maximum Life";
    let anoints = engine
        .call("item_anoints", &json!({"raw":amulet,"withNodes":true}))
        .unwrap();
    let node = anoints["nodes"][0]["name"].as_str().unwrap();
    engine
        .call(
            "equip_item_raw",
            &json!({"text":format!("{amulet}\n{{enchant}}Allocates {node}"),"slot":"Amulet"}),
        )
        .unwrap();
    let anointed = prepared_like_legacy(&engine, amulet, true);
    assert!(
        anointed["raw"]
            .as_str()
            .unwrap()
            .contains(&format!("Allocates {node}"))
    );
    let other_node = anoints["nodes"][1]["name"].as_str().unwrap();
    let preserved = prepared_like_legacy(
        &engine,
        &format!("{amulet}\n{{enchant}}Allocates {other_node}"),
        true,
    );
    assert!(
        preserved["raw"]
            .as_str()
            .unwrap()
            .contains(&format!("Allocates {other_node}"))
    );
    prepared_like_legacy(&engine, amulet, false);

    let edited = engine
        .call(
            "item_customize",
            &json!({"raw":migrated["raw"],"operation":"props","quality":3}),
        )
        .unwrap();
    engine
        .call("item_preview", &json!({"raw":edited["raw"]}))
        .unwrap();
    assert_eq!(
        engine
            .call("item_customization", &json!({"raw":edited["raw"]}))
            .unwrap()["quality"],
        3
    );
    let added = engine
        .call(
            "equip_item_raw",
            &json!({"text":edited["raw"],"slot":"Body Armour"}),
        )
        .unwrap();
    assert_eq!(
        engine
            .call("item_customization", &json!({"itemId":added["itemId"]}))
            .unwrap()["raw"],
        edited["raw"]
    );
    let before_invalid = snapshot(&engine);
    for raw in [
        "",
        "   ",
        "not an item",
        "Rarity: Rare\nInvalid\nUnknown Base",
    ] {
        assert_eq!(
            engine
                .call("item_prepare_preview", &json!({"raw":raw}))
                .unwrap(),
            json!({})
        );
        assert_eq!(snapshot(&engine), before_invalid);
    }
    assert!(
        engine
            .call("item_prepare_preview", &json!({"raw":"Item Class: Rings"}))
            .is_err()
    );
    assert!(
        engine
            .call("item_prepare_preview", &json!({"raw":42}))
            .is_err()
    );
    assert_eq!(snapshot(&engine), before_invalid);
    let generation = engine.call("get_build", &Value::Null).unwrap()["generation"].clone();
    engine.call("new_build", &json!({})).unwrap();
    assert!(
        engine
            .call(
                "item_prepare_preview",
                &json!({"raw":armour,"generation":generation})
            )
            .is_err()
    );
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

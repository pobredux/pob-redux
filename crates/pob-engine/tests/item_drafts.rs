use std::path::PathBuf;

use pob_engine::{Engine, EngineConfig};
use serde_json::{json, Value};

const RING: &str =
    "Rarity: Rare\nDraft Ring\nIron Ring\nItem Level: 80\nImplicits: 0\n+70 to maximum Life";

fn boot(name: &str) -> Option<(Engine, PathBuf)> {
    let root = std::env::var_os("POB_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/resources/pob")
        });
    if !root.join("Launch.lua").is_file() {
        eprintln!("skipping: run pob-sync or set POB_ROOT");
        return None;
    }
    let user_dir =
        std::env::temp_dir().join(format!("pob-item-drafts-{name}-{}", std::process::id()));
    let engine = Engine::boot(EngineConfig {
        pob_root: root,
        user_dir: user_dir.clone(),
    })
    .unwrap();
    engine
        .call("new_build", &json!({"name":"Draft test"}))
        .unwrap();
    Some((engine, user_dir))
}

fn create(engine: &Engine, raw: &str) -> Value {
    engine
        .call("item_draft_create", &json!({"raw":raw,"normalise":false}))
        .unwrap()
}

fn target(draft: &Value) -> Value {
    json!({"draftId":draft["draftId"],"draftRevision":draft["draftRevision"],"generation":draft["generation"]})
}

fn edit(engine: &Engine, draft: &Value, fields: Value) -> Value {
    let mut params = target(draft);
    params
        .as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    engine.call("item_draft_customize", &params).unwrap()
}

fn build_state(engine: &Engine) -> Value {
    json!({
        "xml":engine.call("save_build_xml", &Value::Null).unwrap(),
        "items":engine.call("get_items", &Value::Null).unwrap(),
        "slots":engine.call("list_slots", &Value::Null).unwrap(),
        "build":engine.call("get_build", &Value::Null).unwrap(),
        "undo":engine.eval("local t=launch.main.modes.BUILD.itemsTab; return {undo=#t.undo,redo=#t.redo}").unwrap(),
    })
}

#[test]
fn single_item_insertions_update_equipment_undo_and_calculations_consistently() {
    let Some((engine, user_dir)) = boot("item-insertion") else {
        return;
    };
    let bases = engine.call("craft_bases", &Value::Null).unwrap();
    let ring_type = bases["bases"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, entries)| {
            entries
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["name"] == "Iron Ring")
        })
        .unwrap()
        .0;
    let mut first_id = Value::Null;
    let mut first_xml = Value::Null;
    let mut first_raw = Value::Null;
    for (method, mut params, slot, replaces) in [
        (
            "equip_item_raw",
            json!({"text":RING,"slot":"ring1"}),
            Some("Ring 1"),
            false,
        ),
        (
            "item_edit",
            json!({"text":RING.replace("+70", "+90")}),
            Some("Ring 1"),
            true,
        ),
        ("item_edit", json!({"text":RING}), None, false),
        (
            "craft_item",
            json!({"type":ring_type,"baseName":"Iron Ring","equip":true}),
            Some("Ring 1"),
            false,
        ),
        (
            "craft_rare",
            json!({"type":ring_type,"baseName":"Iron Ring","slot":"ring2"}),
            Some("Ring 2"),
            false,
        ),
        (
            "compare_copy_item",
            json!({"slot":"Ring 1"}),
            Some("Ring 1"),
            false,
        ),
    ] {
        if replaces {
            params["itemId"] = first_id.clone();
        }
        if method == "compare_copy_item" {
            engine
                .call("compare_add", &json!({"xml":first_xml["xml"]}))
                .unwrap();
        }
        let before = build_state(&engine);
        let result = engine.call(method, &params).unwrap();
        let after = build_state(&engine);
        assert_eq!(result["ok"], true, "{method}");
        assert_eq!(
            after["undo"]["undo"].as_u64().unwrap(),
            before["undo"]["undo"].as_u64().unwrap() + 1,
            "{method}"
        );
        assert!(
            after["build"]["rev"].as_u64().unwrap() > before["build"]["rev"].as_u64().unwrap(),
            "{method}"
        );
        assert_eq!(
            after["items"]["items"].as_array().unwrap().len(),
            before["items"]["items"].as_array().unwrap().len() + usize::from(!replaces),
            "{method}"
        );
        if let Some(slot) = slot {
            let equipped = after["slots"]["slots"]
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry["slot"] == slot)
                .unwrap();
            if method == "compare_copy_item" {
                assert_eq!(
                    engine
                        .call("item_raw", &json!({"itemId":equipped["itemId"]}))
                        .unwrap(),
                    first_raw
                );
            } else {
                assert_eq!(equipped["itemId"], result["itemId"], "{method}");
            }
        }
        if replaces {
            assert_eq!(result["itemId"], first_id);
            assert!(engine
                .call("item_raw", &json!({"itemId":first_id}))
                .unwrap()["raw"]
                .as_str()
                .unwrap()
                .contains("+90 to maximum Life"));
        }
        if method == "equip_item_raw" {
            first_id = result["itemId"].clone();
            first_xml = engine.call("save_build_xml", &Value::Null).unwrap();
            first_raw = engine
                .call("item_raw", &json!({"itemId":first_id}))
                .unwrap();
        }
        if method == "item_edit" && !replaces {
            assert!(after["slots"]["slots"]
                .as_array()
                .unwrap()
                .iter()
                .all(|entry| entry["itemId"] != result["itemId"]));
        }
    }
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn failed_draft_commits_restore_build_ownership_and_allow_retry() {
    let Some((engine, user_dir)) = boot("commit-rollback") else {
        return;
    };
    for (owner, method, equip) in [
        ("draft", "BuildModList", true),
        ("items", "AddItem", true),
        ("slot", "SetSelItemId", true),
        ("items", "PopulateSlots", true),
        ("items", "AddUndoState", true),
        ("build", "RefreshStatList", true),
        ("build", "RefreshStatList", false),
    ] {
        engine
            .call("new_build", &json!({"name":"Commit rollback"}))
            .unwrap();
        let equipped = engine
            .call("equip_item_raw", &json!({"text":RING,"slot":"Ring 1"}))
            .unwrap();
        engine
            .call(
                "set_item_props",
                &json!({"itemId":equipped["itemId"],"itemLevel":81}),
            )
            .unwrap();
        assert_eq!(engine.eval("local b=launch.main.modes.BUILD; b.itemsTab:Undo(); b.buildFlag=true; runCallback('OnFrame'); return #b.itemsTab.redo").unwrap(), 1);
        let draft = create(&engine, &RING.replace("+70", "+90"));
        let before = build_state(&engine);
        engine
            .eval(&format!(
                r#"
            local b = launch.main.modes.BUILD
            local tab = b.itemsTab
            __commitItem = __bridge._draft.get({{draftId="{}",generation={}}}).item
            __commitSaved = tab.items[{}]
            __commitOutput = b.calcsTab.mainOutput
            __commitUndo, __commitRedo = tab.undo, tab.redo
            __commitUndoCount = #tab.undo
            __commitCount = __bridge._draft.count
            local owners = {{draft=__commitItem,items=tab,slot=tab.slots["Ring 1"],build=b}}
            __commitFaultOwner = owners["{owner}"]
            __commitFaultOriginal = rawget(__commitFaultOwner, "{method}")
            local original = __commitFaultOwner["{method}"]
            __commitFaultOwner["{method}"] = function(self, ...)
                original(self, ...)
                error("commit fault after {method}")
            end
        "#,
                draft["draftId"].as_str().unwrap(),
                draft["generation"],
                equipped["itemId"]
            ))
            .unwrap();
        let mut commit = target(&draft);
        commit["buildRevision"] = draft["rev"].clone();
        commit["equip"] = json!(equip);
        commit["slot"] = json!("Ring 1");
        let error = engine
            .call("item_draft_commit", &commit)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("commit fault after {method}")),
            "{owner}.{method}: {error}"
        );
        engine
            .eval(&format!(
                "rawset(__commitFaultOwner, '{method}', __commitFaultOriginal); return true"
            ))
            .unwrap();
        assert_eq!(
            engine
                .eval(&format!(
                    r#"
            local b = launch.main.modes.BUILD
            local entry = __bridge._draft.get({{draftId="{}",generation={}}})
            return entry.item == __commitItem and __commitItem.id == nil
                and __bridge._draft.byItem[__commitItem] == entry
                and __bridge._draft.count == __commitCount
                and b.itemsTab.items[{}] == __commitSaved
                and b.calcsTab.mainOutput == __commitOutput
                and b.itemsTab.undo == __commitUndo and b.itemsTab.redo == __commitRedo
        "#,
                    draft["draftId"].as_str().unwrap(),
                    draft["generation"],
                    equipped["itemId"]
                ))
                .unwrap(),
            true,
            "{owner}.{method}"
        );
        assert_eq!(build_state(&engine), before, "{owner}.{method}");
        assert_eq!(
            engine.call("item_draft_get", &target(&draft)).unwrap(),
            draft,
            "{owner}.{method}"
        );
        let saved = engine.call("item_draft_commit", &commit).unwrap();
        assert_eq!(engine.eval(&format!("return launch.main.modes.BUILD.itemsTab.items[{}] == __commitItem and __bridge._draft.byItem[__commitItem] == nil", saved["itemId"])).unwrap(), true);
        assert_eq!(
            engine.call("get_items", &Value::Null).unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(engine.eval(&format!("local tab=launch.main.modes.BUILD.itemsTab; return tab.slots['Ring 1'].selItemId == {} and #tab.undo == __commitUndoCount + 1 and #tab.redo == 0", if equip { saved["itemId"].clone() } else { equipped["itemId"].clone() })).unwrap(), true);
        assert!(engine.call("item_draft_get", &target(&draft)).is_err());
    }
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn failed_cluster_commits_restore_passive_graphs_and_socket_ownership() {
    let Some((engine, user_dir)) = boot("cluster-commit-rollback") else {
        return;
    };
    if engine.call("version", &Value::Null).unwrap()["game"] != "poe1" {
        drop(engine);
        std::fs::remove_dir_all(user_dir).unwrap();
        return;
    }
    let socket = engine.eval(r#"
        local candidates = {}
        for id, node in pairs(launch.main.modes.BUILD.spec.nodes) do
            if node.type == "Socket" and node.path and node.expansionJewel and node.expansionJewel.size == 2 then
                candidates[#candidates + 1] = id
            end
        end
        table.sort(candidates)
        return assert(candidates[1])
    "#).unwrap();
    engine.call("plan_alloc", &json!({"id":socket})).unwrap();
    let slot = format!("Jewel {socket}");
    let raw = "Rarity: Rare\nRollback Cluster\nLarge Cluster Jewel\nItem Level: 85\nImplicits: 3\n{enchant}Adds 8 Passive Skills\n{enchant}2 Added Passive Skills are Jewel Sockets\n{enchant}Added Small Passive Skills grant: 12% increased Physical Damage\n+17 to maximum Mana";
    let equipped = engine
        .call("equip_item_raw", &json!({"text":raw,"slot":slot}))
        .unwrap();
    let node = engine
        .eval(&format!(
            r#"
        local spec = launch.main.modes.BUILD.spec
        for _, graph in pairs(spec.subGraphs) do
            if graph.parentSocket.id == {socket} then
                __commitGraph = graph
                for _, node in pairs(graph.nodes) do
                    if node.path and not node.alloc then return node.id end
                end
            end
        end
        error("missing reachable cluster node")
    "#
        ))
        .unwrap();
    engine.call("plan_alloc", &json!({"id":node})).unwrap();
    let draft = create(&engine, &raw.replace("Adds 8", "Adds 10"));
    let before = build_state(&engine);
    engine.eval(&format!(r#"
        local spec = launch.main.modes.BUILD.spec
        __commitNode = spec.nodes[{node}]
        __commitGraphKey = nil
        for key, graph in pairs(spec.subGraphs) do
            if graph.parentSocket.id == {socket} then __commitGraph, __commitGraphKey = graph, key end
        end
        __commitGraphMethod = rawget(spec, "BuildClusterJewelGraphs")
        local original = spec.BuildClusterJewelGraphs
        spec.BuildClusterJewelGraphs = function(self, ...)
            original(self, ...)
            error("cluster commit fault")
        end
    "#)).unwrap();
    let mut commit = target(&draft);
    commit["buildRevision"] = draft["rev"].clone();
    commit["equip"] = json!(true);
    commit["slot"] = json!(slot);
    assert!(engine
        .call("item_draft_commit", &commit)
        .unwrap_err()
        .to_string()
        .contains("cluster commit fault"));
    engine.eval("rawset(launch.main.modes.BUILD.spec, 'BuildClusterJewelGraphs', __commitGraphMethod); return true").unwrap();
    assert_eq!(
        engine
            .eval(&format!(
                r#"
        local b = launch.main.modes.BUILD
        return b.spec.subGraphs[__commitGraphKey] == __commitGraph
            and b.spec.nodes[{node}] == __commitNode and __commitNode.alloc
            and b.spec.jewels[{socket}] == {}
            and b.itemsTab.slots["{slot}"].selItemId == {}
    "#,
                equipped["itemId"], equipped["itemId"]
            ))
            .unwrap(),
        true
    );
    assert_eq!(build_state(&engine), before);
    assert_eq!(
        engine.call("item_draft_get", &target(&draft)).unwrap(),
        draft
    );
    let saved = engine.call("item_draft_commit", &commit).unwrap();
    assert_eq!(
        engine
            .eval(&format!(
                "return launch.main.modes.BUILD.spec.jewels[{socket}] == {}",
                saved["itemId"]
            ))
            .unwrap(),
        true
    );
    assert!(engine.call("item_draft_get", &target(&draft)).is_err());
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn poe1_pasted_influenced_amulet_anoints_are_independent_of_equipped_items() {
    let Some((engine, user_dir)) = boot("anoint-isolation") else {
        return;
    };
    if engine.call("version", &Value::Null).unwrap()["game"] != "poe1" {
        drop(engine);
        std::fs::remove_dir_all(user_dir).unwrap();
        return;
    }
    let amulet = "Rarity: Rare\nAnoint Candidate\nJade Amulet\nShaper Item\nItem Level: 85\nImplicits: 0\n+40 to maximum Life";
    let seed = create(&engine, amulet);
    let mut params = target(&seed);
    params["withNodes"] = json!(true);
    let anoints = engine.call("item_anoints", &params).unwrap();
    let original_node = &anoints["nodes"][0];
    let replacement_node = &anoints["nodes"][1];
    engine
        .call("item_draft_dispose", &json!({"draftId":seed["draftId"]}))
        .unwrap();
    for change_anoint in [true, false] {
        engine
            .call("new_build", &json!({"name":"Anoint isolation"}))
            .unwrap();
        let equipped = engine.call(
            "equip_item_raw",
            &json!({"text":format!("Rarity: Rare\nEquipped Amulet\nJade Amulet\nItem Level: 85\nImplicits: 0\n{{enchant}}Allocates {}\n+70 to maximum Life", original_node["name"].as_str().unwrap()),"slot":"Amulet"}),
        ).unwrap();
        let equipped_target = json!({"itemId":equipped["itemId"]});
        let original_raw = engine.call("item_raw", &equipped_target).unwrap();
        let before = build_state(&engine);
        let mut draft = engine
            .call("item_draft_create", &json!({"raw":amulet,"normalise":true}))
            .unwrap();
        let original_anoint = format!("Allocates {}", original_node["name"].as_str().unwrap());
        assert!(draft["raw"].as_str().unwrap().contains(&original_anoint));
        assert_eq!(engine.eval(&format!(
            "local draft = __bridge._draft.get({{draftId=\"{}\",generation={}}}).item; local equipped = launch.main.modes.BUILD.itemsTab.items[{}]; return draft.shaper and draft.enchantModLines ~= equipped.enchantModLines and draft.enchantModLines[1] ~= equipped.enchantModLines[1] and draft.enchantModLines[1].modList ~= equipped.enchantModLines[1].modList",
            draft["draftId"].as_str().unwrap(), draft["generation"], equipped["itemId"],
        )).unwrap(), true, "pasting must detach the copied anoint and its parsed modifiers");
        if change_anoint {
            draft = edit(
                &engine,
                &draft,
                json!({"operation":"anoint","nodeId":replacement_node["id"],"slot":1}),
            );
            assert!(draft["raw"].as_str().unwrap().contains(&format!(
                "Allocates {}",
                replacement_node["name"].as_str().unwrap()
            )));
            assert_eq!(
                engine.call("item_raw", &equipped_target).unwrap(),
                original_raw,
                "changing the draft anoint must not change the equipped amulet",
            );
            assert_eq!(build_state(&engine), before);
        }
        let mut commit = target(&draft);
        commit["buildRevision"] = draft["rev"].clone();
        commit["equip"] = json!(false);
        let saved = engine.call("item_draft_commit", &commit).unwrap();
        assert_eq!(
            engine.call("item_raw", &equipped_target).unwrap(),
            original_raw
        );
        assert_eq!(
            engine
                .call("item_raw", &json!({"itemId":saved["itemId"]}))
                .unwrap()["raw"],
            draft["raw"]
        );
        assert_eq!(engine.eval(&format!(
            "local items = launch.main.modes.BUILD.itemsTab.items; return items[{}].enchantModLines ~= items[{}].enchantModLines",
            saved["itemId"], equipped["itemId"],
        )).unwrap(), true, "committed and equipped amulets must not share anoint tables");
    }
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn special_jewel_drafts_preserve_native_data_through_edits_rollback_and_commit() {
    let Some((engine, user_dir)) = boot("special-jewels") else {
        return;
    };
    let poe1 = engine.call("version", &Value::Null).unwrap()["game"] == "poe1";
    let socket = engine.eval(r#"
        local candidates = {}
        for id, node in pairs(launch.main.modes.BUILD.spec.nodes) do
            if node.type == "Socket" and node.path and
                (not data.clusterJewels or node.expansionJewel and node.expansionJewel.size == 2) then
                candidates[#candidates + 1] = id
            end
        end
        table.sort(candidates)
        return assert(candidates[1], "missing reachable jewel socket")
    "#).unwrap();
    engine.call("plan_alloc", &json!({"id":socket})).unwrap();
    engine.eval(r#"
        function __edgeState(item)
            local scalars = {}
            for k, v in pairs(item.jewelData or {}) do
                if type(v) == "number" or type(v) == "string" or type(v) == "boolean" then
                    scalars[k] = v
                end
            end
            return {
                base = item.baseName, radius = item.jewelRadiusIndex or false,
                skill = item.clusterJewelSkill or false, nodes = item.clusterJewelNodeCount or false,
                variant = item.variant or false, jewelData = scalars,
                conqueredBy = item.jewelData and item.jewelData.conqueredBy or false,
                notables = item.jewelData and item.jewelData.clusterJewelNotables or false,
                addedMods = item.jewelData and item.jewelData.clusterJewelAddedMods or false,
            }
        end
    "#).unwrap();
    let mut fixtures = Vec::new();
    if poe1 {
        for size in ["Small", "Medium", "Large"] {
            fixtures.push((format!("{size} Cluster Jewel"), format!("Rarity: Rare\nEdge Cluster\n{size} Cluster Jewel\nCrafted: true\nItem Level: 85\nPrefix: {{range:0.371}}AfflictionJewelSmallPassivesHaveIncreasedEffect2\nImplicits: 0\n{{custom}}+17 to maximum Mana")));
        }
        for name in [
            "Brutal Restraint",
            "Elegant Hubris",
            "Glorious Vanity",
            "Lethal Pride",
            "Militant Faith",
        ] {
            let raw = engine
                .eval(&format!(
                    r#"
                for _, raw in ipairs(LoadModule("Data/Uniques/jewel")) do
                    if raw:match("^%s*([^\n]+)") == "{name}" then
                        return "Rarity: Unique\n" .. raw:gsub("^%s+", "")
                    end
                end
                error("missing timeless fixture")
            "#
                ))
                .unwrap();
            fixtures.push((name.to_owned(), raw.as_str().unwrap().to_owned()));
        }
    } else {
        for colour in ["Ruby", "Emerald", "Sapphire", "Diamond"] {
            fixtures.push((format!("Time-Lost {colour}"), format!("Rarity: Rare\nEdge Radius\nTime-Lost {colour}\nCrafted: true\nItem Level: 85\nPrefix: {{range:0.371}}JewelRadiusMediumSize\nImplicits: 0\nUpgrades Radius to Medium\n{{custom}}+17 to maximum Mana")));
        }
    }
    for (name, raw) in fixtures {
        let raw = if name.contains("Cluster") {
            engine.eval(&format!(
                r#"local item = new("Item"):Item({})
                item.clusterJewelSkill = item.clusterJewel.size == "Medium" and "affliction_area_damage" or next(item.clusterJewel.skills)
                item.clusterJewelNodeCount = item.clusterJewel.maxNodes
                if item.clusterJewel.size == "Medium" then
                    item.suffixes[1] = {{modId = "AfflictionJewelSmallPassivesGrantAreaOfEffect2", range = 0.613}}
                end
                item.enchantModLines = {{
                    {{line = "Adds " .. item.clusterJewelNodeCount .. " Passive Skills", crafted = true}},
                    {{line = table.concat(item.clusterJewel.skills[item.clusterJewelSkill].enchant, "\n"), crafted = true}},
                }}
                if item.clusterJewel.size ~= "Small" then
                    table.insert(item.enchantModLines, {{line = (item.clusterJewel.size == "Large" and "2" or "1") .. " Added Passive Skills are Jewel Sockets", crafted = true}})
                end
                item:Craft()
                return item:BuildRaw()"#,
                serde_json::to_string(&raw).unwrap()
            )).unwrap().as_str().unwrap().to_owned()
        } else {
            raw
        };
        let before = build_state(&engine);
        let mut draft = create(&engine, &raw);
        assert!(draft["draftId"].is_string(), "{name}: {draft}");
        engine
            .eval(&format!(
                r#"
            __edgeItem = __bridge._draft.get({{draftId="{}",generation={}}}).item
            __edgeBase = __edgeItem.base
            __edgeCluster = __edgeItem.clusterJewel
            __edgeInitialData = __edgeItem.jewelData
        "#,
                draft["draftId"].as_str().unwrap(),
                draft["generation"]
            ))
            .unwrap();
        let initial = engine.eval("return __edgeState(__edgeItem)").unwrap();
        assert_eq!(
            engine.call("item_draft_get", &target(&draft)).unwrap(),
            draft,
            "{name}"
        );
        assert_eq!(
            engine
                .eval("return __edgeItem.jewelData == __edgeInitialData")
                .unwrap(),
            true,
            "{name}"
        );
        let operation = if name.contains("Cluster") {
            let shape = engine.call("item_shape", &target(&draft)).unwrap();
            let skill = shape["cluster"]["skills"]
                .as_array()
                .unwrap()
                .iter()
                .find(|skill| skill["id"] != shape["cluster"]["skill"])
                .unwrap()["id"]
                .clone();
            json!({"operation":"shape","clusterSkill":skill,"clusterNodeCount":shape["cluster"]["minNodes"]})
        } else if name.starts_with("Time-Lost") {
            assert!(initial["radius"].is_number(), "{name}: {initial}");
            json!({"operation":"affix","table":"prefixes","index":1,"modId":"JewelRadiusLargeSize","range":0.613})
        } else {
            assert!(
                initial["conqueredBy"]["id"].is_number(),
                "{name}: {initial}"
            );
            json!({"operation":"variant","picks":[2]})
        };
        let requested = operation.clone();
        engine
            .eval(
                r#"
            local preview = __bridge.item_preview
            __bridge.item_preview = function(p)
                __bridge.item_preview = preview
                preview(p)
                error("edge snapshot failure")
            end
        "#,
            )
            .unwrap();
        let mut failed = target(&draft);
        failed
            .as_object_mut()
            .unwrap()
            .extend(operation.as_object().unwrap().clone());
        assert!(
            engine
                .call("item_draft_customize", &failed)
                .unwrap_err()
                .to_string()
                .contains("edge snapshot failure"),
            "{name}"
        );
        assert_eq!(
            engine.call("item_draft_get", &target(&draft)).unwrap(),
            draft,
            "{name}"
        );
        assert_eq!(
            engine.eval("return __edgeState(__edgeItem)").unwrap(),
            initial,
            "{name}"
        );
        assert_eq!(engine.eval("return __edgeItem.jewelData == __edgeInitialData and __edgeItem.base == __edgeBase and __edgeItem.clusterJewel == __edgeCluster").unwrap(), true, "{name}");
        draft = edit(&engine, &draft, operation);
        if !name.contains("Cluster") && !name.starts_with("Time-Lost") {
            let seed_line = engine
                .eval(
                    r#"
                for index, line in ipairs(__edgeItem.explicitModLines) do
                    if __edgeItem:CheckModLineVariant(line) and line.line:find("%(%d+%-%d+%)") then
                        return index
                    end
                end
                error("missing active timeless seed")
            "#,
                )
                .unwrap();
            let old_seed = engine
                .eval("return __edgeItem.jewelData.conqueredBy.id")
                .unwrap();
            draft = edit(
                &engine,
                &draft,
                json!({"operation":"modifier","section":"explicit","index":seed_line,"range":0.137}),
            );
            assert_ne!(
                engine
                    .eval("return __edgeItem.jewelData.conqueredBy.id")
                    .unwrap(),
                old_seed,
                "{name}"
            );
        }
        let changed = engine.eval("return __edgeState(__edgeItem)").unwrap();
        assert_ne!(changed, initial, "{name}");
        if name.contains("Cluster") {
            assert_eq!(changed["skill"], requested["clusterSkill"], "{name}");
            assert_eq!(changed["nodes"], requested["clusterNodeCount"], "{name}");
        } else if name.starts_with("Time-Lost") {
            assert_eq!(initial["radius"], 2, "{name}");
            assert_eq!(changed["radius"], 3, "{name}");
        }
        assert_eq!(
            engine
                .eval("return __edgeState(new(\"Item\"):Item(__edgeItem:BuildRaw()))")
                .unwrap(),
            changed,
            "{name}"
        );
        if name.contains("Cluster") {
            assert!(
                draft["raw"]
                    .as_str()
                    .unwrap()
                    .contains("{range:0.371}AfflictionJewelSmallPassivesHaveIncreasedEffect2"),
                "{name}"
            );
            if name == "Medium Cluster Jewel" {
                assert!(
                    draft["raw"]
                        .as_str()
                        .unwrap()
                        .contains("AfflictionJewelSmallPassivesGrantAreaOfEffect2"),
                    "{}",
                    draft["raw"]
                );
                assert!(draft["customization"]["affixes"]["suffixes"][0]["options"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|option| {
                        option["modIds"]
                            .as_array()
                            .unwrap()
                            .contains(&json!("AfflictionJewelSmallPassivesGrantAreaOfEffect2"))
                            && option["label"].as_str().unwrap().contains("Retained")
                    }));
            }
            assert!(
                draft["raw"]
                    .as_str()
                    .unwrap()
                    .contains("{custom}+17 to maximum Mana"),
                "{name}"
            );
        }
        assert_eq!(build_state(&engine), before, "{name}");
        let mut commit = target(&draft);
        commit["buildRevision"] = draft["rev"].clone();
        let saved_slots = engine.call("list_slots", &Value::Null).unwrap();
        let slot = draft["slots"]
            .as_array()
            .unwrap()
            .iter()
            .find(|slot| {
                saved_slots["slots"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|saved| saved["slot"] == slot["slot"] && saved["nodeId"] == socket)
            })
            .expect("allocated jewel socket missing from compatible slots")["slot"]
            .clone();
        commit["equip"] = json!(true);
        commit["slot"] = slot.clone();
        let saved = engine.call("item_draft_commit", &commit).unwrap();
        assert_eq!(
            engine
                .eval(&format!(
                    "return launch.main.modes.BUILD.itemsTab.items[{}] == __edgeItem",
                    saved["itemId"]
                ))
                .unwrap(),
            true,
            "{name}"
        );
        assert_eq!(
            engine.eval("return __edgeState(__edgeItem)").unwrap(),
            changed,
            "{name}"
        );
        assert_eq!(
            engine
                .call("item_raw", &json!({"itemId":saved["itemId"]}))
                .unwrap()["raw"],
            draft["raw"],
            "{name}"
        );
        assert!(
            engine.call("list_slots", &Value::Null).unwrap()["slots"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["slot"] == slot && entry["itemId"] == saved["itemId"]),
            "{name}"
        );
        if name.contains("Cluster") {
            assert!(
                engine
                    .eval("return __edgeItem.jewelData.clusterJewelValid")
                    .unwrap()
                    .is_number(),
                "{name}"
            );
            assert_eq!(
                engine
                    .eval(&format!(
                        "for _, graph in pairs(launch.main.modes.BUILD.spec.subGraphs) do if graph.parentSocket.id == {} then return true end end return false",
                        socket
                    ))
                    .unwrap(),
                true,
                "{name}"
            );
        }
    }
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn draft_reads_reuse_the_native_item_and_edits_only_parse_natively() {
    let Some((engine, user_dir)) = boot("identity") else {
        return;
    };
    let before = build_state(&engine);
    let draft = create(&engine, RING);
    assert_eq!(draft["raw"], draft["customization"]["raw"]);
    assert_eq!(draft["draftRevision"], 0);
    engine
        .eval(&format!(
            r#"
        __draftOriginal = __bridge._draft.get({{draftId="{}",generation={}}}).item
        __draftParseCount = 0
        local class = getmetatable(new("Item"):Item("Rarity: Normal\nIron Ring"))
        local parse = class.ParseRaw
        class.ParseRaw = function(self, ...)
            __draftParseCount = __draftParseCount + 1
            return parse(self, ...)
        end
    "#,
            draft["draftId"].as_str().unwrap(),
            draft["generation"]
        ))
        .unwrap();
    for _ in 0..3 {
        assert_eq!(
            engine.call("item_draft_get", &target(&draft)).unwrap(),
            draft
        );
        assert_eq!(
            engine.call("item_customization", &target(&draft)).unwrap(),
            draft["customization"]
        );
        assert_eq!(
            engine.call("item_preview", &target(&draft)).unwrap()["tooltip"],
            draft["tooltip"]
        );
        engine
            .call("item_modifier_options", &target(&draft))
            .unwrap();
    }
    assert_eq!(engine.eval("return __draftParseCount").unwrap(), 0);
    assert_eq!(build_state(&engine), before);

    let updated = edit(&engine, &draft, json!({"operation":"props","itemLevel":84}));
    assert_eq!(updated["draftId"], draft["draftId"]);
    assert_eq!(updated["draftRevision"], 1);
    assert_eq!(updated["customization"]["itemLevel"], 84);
    assert_eq!(engine.eval("return __draftParseCount").unwrap(), 1);
    assert_eq!(engine.eval(&format!("return __draftOriginal == __bridge._draft.get({{draftId=\"{}\",generation={}}}).item", updated["draftId"].as_str().unwrap(), updated["generation"])).unwrap(), true);
    let unchanged = edit(
        &engine,
        &updated,
        json!({"operation":"props","itemLevel":84}),
    );
    let mut expected = updated.clone();
    expected["draftRevision"] = json!(2);
    assert_eq!(unchanged, expected);
    let mut stale = target(&updated);
    stale["operation"] = json!("props");
    stale["itemLevel"] = json!(90);
    assert!(engine.call("item_draft_customize", &stale).is_err());
    stale["buildRevision"] = updated["rev"].clone();
    stale["equip"] = json!(false);
    assert!(engine.call("item_draft_commit", &stale).is_err());
    assert_eq!(build_state(&engine), before);
    engine
        .call(
            "set_config",
            &json!({"var":"conditionEnemyMoving","value":true}),
        )
        .unwrap();
    let changed_build = build_state(&engine);
    let refreshed = engine.call("item_draft_get", &target(&updated)).unwrap();
    assert_eq!(refreshed["raw"], updated["raw"]);
    assert_eq!(refreshed["draftRevision"], unchanged["draftRevision"]);
    assert_ne!(refreshed["rev"], updated["rev"]);
    assert_eq!(build_state(&engine), changed_build);
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn draft_crafting_preserves_independent_rolls_custom_mods_and_commit_identity() {
    let Some((engine, user_dir)) = boot("craft") else {
        return;
    };
    let raw = engine.eval(r#"
        local item = new("Item"):Item("Rarity: Rare\nDraft Bow\nCrude Bow\nCrafted: true\nQuality: 23\nItem Level: 85\nPrefix: {range:0.137,0.863}AddedPhysicalDamage2\nImplicits: 0\n{custom}+17 to maximum Mana")
        item:Craft()
        return item:BuildRaw()
    "#).unwrap();
    let before = build_state(&engine);
    let mut draft = create(&engine, raw.as_str().unwrap());
    for fraction in [0.1, 0.8] {
        let series = draft["customization"]["affixes"]["suffixes"][0]["options"][0]["id"].clone();
        draft = edit(
            &engine,
            &draft,
            json!({"operation":"affix","table":"suffixes","index":1,"seriesId":series,"relativePosition":fraction}),
        );
        assert!(draft["raw"]
            .as_str()
            .unwrap()
            .contains("{range:0.137,0.863}AddedPhysicalDamage2"));
        assert!(draft["raw"]
            .as_str()
            .unwrap()
            .contains("{custom}+17 to maximum Mana"));
        assert_eq!(draft["customization"]["quality"], 23);
        assert_eq!(draft["customization"]["itemLevel"], 85);
        assert!(
            draft["customization"]["affixes"]["prefixes"][0]["rangeIsTable"]
                .as_bool()
                .unwrap()
        );
        assert_eq!(
            engine.call("item_draft_get", &target(&draft)).unwrap(),
            draft
        );
        assert_eq!(build_state(&engine), before);
    }
    engine
        .eval(&format!(
            "__committedCandidate = __bridge._draft.get({{draftId=\"{}\",generation={}}}).item",
            draft["draftId"].as_str().unwrap(),
            draft["generation"]
        ))
        .unwrap();
    let mut params = target(&draft);
    params["buildRevision"] = draft["rev"].clone();
    params["equip"] = json!(true);
    params["slot"] = json!("Weapon 1");
    let committed = engine.call("item_draft_commit", &params).unwrap();
    assert!(engine.call("item_draft_get", &target(&draft)).is_err());
    assert!(engine.call("item_draft_commit", &params).is_err());
    assert_eq!(
        engine
            .call("item_raw", &json!({"itemId":committed["itemId"]}))
            .unwrap()["raw"],
        draft["raw"]
    );
    assert_eq!(
        engine
            .eval(&format!(
                "return __committedCandidate == launch.main.modes.BUILD.itemsTab.items[{}]",
                committed["itemId"]
            ))
            .unwrap(),
        true
    );
    let after = build_state(&engine);
    assert_eq!(
        after["undo"]["undo"].as_u64().unwrap(),
        before["undo"]["undo"].as_u64().unwrap() + 1
    );
    let saved = engine
        .call(
            "set_item_props",
            &json!({"itemId":committed["itemId"],"quality":27}),
        )
        .unwrap();
    assert_eq!(saved["ok"], true);
    assert_eq!(
        engine.call("item_customization", &json!({"itemId":committed["itemId"]})).unwrap()["quality"],
        27
    );
    let saved_after = build_state(&engine);
    assert!(
        saved_after["build"]["rev"].as_u64().unwrap() > after["build"]["rev"].as_u64().unwrap()
    );
    assert_eq!(
        saved_after["undo"]["undo"].as_u64().unwrap(),
        after["undo"]["undo"].as_u64().unwrap() + 1
    );
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn stale_draft_reads_return_current_state_but_writes_remain_revision_checked() {
    let Some((engine, user_dir)) = boot("stale-reads") else {
        return;
    };
    let raw = engine.eval(r#"
        local item = new("Item"):Item("Rarity: Rare\nRead Bow\nCrude Bow\nCrafted: true\nItem Level: 85\nImplicits: 0")
        item:Craft()
        return item:BuildRaw()
    "#).unwrap();
    let original = create(&engine, raw.as_str().unwrap());
    let updated = edit(
        &engine,
        &original,
        json!({"operation":"props","itemLevel":84}),
    );
    assert_ne!(original["draftRevision"], updated["draftRevision"]);
    let before = build_state(&engine);
    let series = updated["customization"]["affixes"]["suffixes"][0]["options"][0]["id"].clone();
    assert!(series.is_string());
    for (method, extra) in [
        ("item_draft_get", json!({})),
        ("item_preview", json!({})),
        ("item_tooltip", json!({})),
        ("item_customization", json!({})),
        ("item_modifier_options", json!({"source":"Suffix"})),
        (
            "item_affix_rolls",
            json!({"table":"suffixes","index":1,"seriesId":series}),
        ),
        ("item_enchants", json!({})),
        ("item_anoints", json!({"withNodes":true})),
    ] {
        let mut stale = target(&original);
        let mut current = target(&updated);
        stale
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        current
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert_eq!(
            engine.call(method, &stale).unwrap(),
            engine.call(method, &current).unwrap(),
            "{method}"
        );
        stale["generation"] = json!(updated["generation"].as_u64().unwrap() + 1);
        assert!(engine.call(method, &stale).is_err(), "{method}");
    }
    assert_eq!(
        engine.call("item_draft_get", &target(&original)).unwrap(),
        updated
    );
    for method in ["item_draft_customize", "item_draft_commit"] {
        for missing in [false, true] {
            let mut stale = target(&original);
            if missing {
                stale.as_object_mut().unwrap().remove("draftRevision");
            }
            stale["operation"] = json!("props");
            stale["itemLevel"] = json!(90);
            stale["buildRevision"] = updated["rev"].clone();
            stale["equip"] = json!(false);
            assert!(
                engine
                    .call(method, &stale)
                    .unwrap_err()
                    .to_string()
                    .contains("item preview changed"),
                "{method}"
            );
        }
    }
    assert_eq!(
        engine.call("item_draft_get", &target(&updated)).unwrap(),
        updated
    );
    assert_eq!(build_state(&engine), before);
    engine
        .call("item_draft_dispose", &json!({"draftId":updated["draftId"]}))
        .unwrap();
    assert!(engine
        .call("item_draft_get", &target(&original))
        .unwrap_err()
        .to_string()
        .contains("item preview expired"));
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn failed_draft_edits_roll_back_and_stale_or_ambiguous_targets_are_rejected() {
    let Some((engine, user_dir)) = boot("rollback") else {
        return;
    };
    let before = build_state(&engine);
    let original = create(&engine, RING);
    let updated = edit(
        &engine,
        &original,
        json!({"operation":"props","itemLevel":83}),
    );
    let mut stale = target(&original);
    stale["operation"] = json!("props");
    stale["itemLevel"] = json!(90);
    assert!(engine.call("item_draft_customize", &stale).is_err());
    let mut missing = target(&updated);
    missing.as_object_mut().unwrap().remove("draftRevision");
    missing["operation"] = json!("props");
    assert!(engine.call("item_draft_customize", &missing).is_err());
    for conflicting in ["raw", "itemId", "db"] {
        let mut params = target(&updated);
        params[conflicting] = json!(RING);
        assert!(engine.call("item_draft_get", &params).is_err());
    }
    for method in ["item_preview", "item_tooltip", "item_customization", "item_customize", "set_item_props"] {
        assert!(engine.call(method, &json!({"raw":RING,"operation":"props","itemLevel":90})).is_err());
    }
    for method in [
        "item_customize", "set_item_props", "set_item_rune", "set_item_variant",
        "set_item_shape", "set_item_crucible", "set_item_enchant", "set_item_anoint", "corrupt_item",
    ] {
        let mut params = target(&updated);
        params["operation"] = json!("props");
        params["itemLevel"] = json!(90);
        assert!(engine.call(method, &params).unwrap_err().to_string()
            .contains("use item_draft_customize"));
    }
    assert_eq!(engine.call("item_draft_get", &target(&updated)).unwrap(), updated);
    assert_eq!(build_state(&engine), before);
    engine
        .eval(
            r#"
        local class = getmetatable(new("Item"):Item("Rarity: Normal\nIron Ring"))
        local rebuild = class.BuildAndParseRaw
        class.BuildAndParseRaw = function(self)
            rebuild(self)
            class.BuildAndParseRaw = rebuild
            error("simulated failure after native item mutation")
        end
    "#,
        )
        .unwrap();
    let mut params = target(&updated);
    params["operation"] = json!("props");
    params["itemLevel"] = json!(92);
    assert!(engine.call("item_draft_customize", &params).is_err());
    assert_eq!(
        engine.call("item_draft_get", &target(&updated)).unwrap(),
        updated
    );
    assert_eq!(build_state(&engine), before);
    let mut invalid_commit = target(&updated);
    invalid_commit["buildRevision"] = updated["rev"].clone();
    invalid_commit["equip"] = json!(true);
    invalid_commit["slot"] = json!("Helmet");
    assert!(engine.call("item_draft_commit", &invalid_commit).is_err());
    invalid_commit["equip"] = json!(false);
    invalid_commit["buildRevision"] = json!(-1);
    assert!(engine.call("item_draft_commit", &invalid_commit).is_err());
    assert_eq!(
        engine.call("item_draft_get", &target(&updated)).unwrap(),
        updated
    );
    assert_eq!(build_state(&engine), before);
    let repaired = edit(
        &engine,
        &updated,
        json!({"operation":"props","itemLevel":90}),
    );
    assert_eq!(repaired["draftRevision"], 2);
    assert_eq!(repaired["customization"]["itemLevel"], 90);
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn failed_draft_edits_restore_exact_native_state_without_reparsing() {
    let Some((engine, user_dir)) = boot("exact-rollback") else {
        return;
    };
    let before = build_state(&engine);
    let raw = "Rarity: Rare\nPrecise Draft\nCrude Bow\nCrafted: true\nQuality: 23\nItem Level: 85\nPrefix: {range:0.12349}AddedPhysicalDamage2\nPrefix: {range:0.23456,0.87654}AddedPhysicalDamage3\nImplicits: 0\n{custom}+17 to maximum Mana";
    let draft = create(&engine, raw);
    engine.eval(&format!(r#"
        __rollbackItem = __bridge._draft.get({{draftId="{}",generation={}}}).item
        __rollbackPrefixes = __rollbackItem.prefixes
        __rollbackPrefix = __rollbackPrefixes[1]
        __rollbackPaired = __rollbackPrefixes[2]
        __rollbackRanges = __rollbackPaired.range
        __rollbackBase = __rollbackItem.base
        __rollbackAffixes = __rollbackItem.affixes
        __rollbackMods = __rollbackItem.baseModList
        __rollbackRaw = __rollbackItem.raw
        __rollbackClass = getmetatable(__rollbackItem)
        __rollbackModsClass = getmetatable(__rollbackMods)
        __rollbackProbeMeta = {{}}
        __rollbackProbe = setmetatable({{value=0.12349, alias=__rollbackPrefix, item=__rollbackItem}}, __rollbackProbeMeta)
        __rollbackProbe.self = __rollbackProbe
        __rollbackItem.__rollbackProbe = __rollbackProbe
        __rollbackParseCount = 0
        local parse = __rollbackClass.ParseRaw
        __rollbackClass.ParseRaw = function(self, ...)
            __rollbackParseCount = __rollbackParseCount + 1
            return parse(self, ...)
        end
        function __assertRollbackState()
            assert(__rollbackItem.prefixes == __rollbackPrefixes)
            assert(__rollbackItem.prefixes[1] == __rollbackPrefix)
            assert(__rollbackItem.prefixes[2] == __rollbackPaired)
            assert(__rollbackPrefix.range == 0.12349)
            assert(__rollbackPaired.range == __rollbackRanges)
            assert(__rollbackRanges[1] == 0.23456 and __rollbackRanges[2] == 0.87654)
            assert(__rollbackItem.base == __rollbackBase)
            assert(__rollbackItem.affixes == __rollbackAffixes)
            assert(__rollbackItem.baseModList == __rollbackMods)
            assert(__rollbackItem.raw == __rollbackRaw)
            assert(getmetatable(__rollbackItem) == __rollbackClass)
            assert(getmetatable(__rollbackMods) == __rollbackModsClass)
            assert(__rollbackItem.__rollbackProbe == __rollbackProbe)
            assert(__rollbackProbe.value == 0.12349)
            assert(__rollbackProbe.self == __rollbackProbe)
            assert(__rollbackProbe.alias == __rollbackPrefix)
            assert(__rollbackProbe.item == __rollbackItem)
            assert(getmetatable(__rollbackProbe) == __rollbackProbeMeta)
            assert(__rollbackItem.__introducedByFailure == nil)
            assert(__rollbackItem.itemLevel == 85 and __rollbackItem.quality == 23)
            return true
        end
    "#, draft["draftId"].as_str().unwrap(), draft["generation"])).unwrap();
    assert_eq!(engine.eval("return __assertRollbackState()").unwrap(), true);

    let mut invalid = target(&draft);
    invalid["operation"] = json!("rune");
    invalid["index"] = json!(999);
    invalid["name"] = json!("None");
    assert!(
        engine
            .call("item_draft_customize", &invalid)
            .unwrap_err()
            .to_string()
            .contains("rune index out of range")
    );
    assert_eq!(
        engine.eval("return __rollbackItem.prefixes[1].range").unwrap(),
        json!(0.12349)
    );
    assert_eq!(engine.eval("return __rollbackParseCount").unwrap(), 0);
    assert_eq!(engine.eval("return __assertRollbackState()").unwrap(), true);
    assert_eq!(
        engine.call("item_draft_get", &target(&draft)).unwrap(),
        draft
    );

    engine
        .eval(
            r#"
        local rebuild = __rollbackClass.BuildAndParseRaw
        __rollbackClass.BuildAndParseRaw = function(self)
            __rollbackClass.BuildAndParseRaw = rebuild
            __rollbackPrefix.range = 0.99
            __rollbackRanges[1] = 0.01
            __rollbackProbe.value = 0.99
            __rollbackProbe.alias = nil
            setmetatable(__rollbackProbe, {})
            self.__introducedByFailure = true
            rebuild(self)
            error("failure after native reparse")
        end
    "#,
        )
        .unwrap();
    let mut mutation = target(&draft);
    mutation["operation"] = json!("props");
    mutation["itemLevel"] = json!(91);
    assert!(
        engine
            .call("item_draft_customize", &mutation)
            .unwrap_err()
            .to_string()
            .contains("failure after native reparse")
    );
    assert_eq!(engine.eval("return __rollbackParseCount").unwrap(), 1);
    assert_eq!(engine.eval("return __assertRollbackState()").unwrap(), true);
    assert_eq!(
        engine.call("item_draft_get", &target(&draft)).unwrap(),
        draft
    );
    assert_eq!(build_state(&engine), before);

    engine
        .eval(
            r#"
        local preview = __bridge.item_preview
        __bridge.item_preview = function(p)
            __bridge.item_preview = preview
            preview(p)
            error("failure calculating draft snapshot")
        end
    "#,
        )
        .unwrap();
    assert!(
        engine
            .call("item_draft_customize", &mutation)
            .unwrap_err()
            .to_string()
            .contains("failure calculating draft snapshot")
    );
    assert_eq!(engine.eval("return __rollbackParseCount").unwrap(), 2);
    assert_eq!(engine.eval("return __assertRollbackState()").unwrap(), true);
    assert_eq!(
        engine.call("item_draft_get", &target(&draft)).unwrap(),
        draft
    );
    assert_eq!(build_state(&engine), before);

    let repaired = edit(&engine, &draft, json!({"operation":"props","itemLevel":91}));
    assert_eq!(repaired["draftId"], draft["draftId"]);
    assert_eq!(repaired["draftRevision"], 1);
    assert_eq!(repaired["customization"]["itemLevel"], 91);
    assert_eq!(engine.eval("return __rollbackParseCount").unwrap(), 3);
    assert_eq!(engine.eval(&format!("return __rollbackItem == __bridge._draft.get({{draftId=\"{}\",generation={}}}).item", repaired["draftId"].as_str().unwrap(), repaired["generation"])).unwrap(), true);
    assert_eq!(build_state(&engine), before);
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn draft_capacity_evicts_least_recently_used_previews_without_touching_saved_items() {
    let Some((engine, user_dir)) = boot("lru") else {
        return;
    };
    let saved = engine
        .call("equip_item_raw", &json!({"text":RING,"slot":"Ring 1"}))
        .unwrap();
    let saved_raw = engine
        .call("item_raw", &json!({"itemId":saved["itemId"]}))
        .unwrap();
    let before = build_state(&engine);
    let drafts: Vec<_> = (0..64).map(|_| create(&engine, RING)).collect();
    engine
        .eval(&format!(
            "__evictedDraftItem = __bridge._draft.entries[\"{}\"].item",
            drafts[1]["draftId"].as_str().unwrap()
        ))
        .unwrap();
    assert_eq!(engine.eval("return __bridge._draft.count").unwrap(), 64);
    assert_eq!(create(&engine, "this is not an item"), Value::Null);
    engine
        .eval(
            r#"
        local snapshot = __bridge._draft.snapshot
        __bridge._draft.snapshot = function(entry)
            __bridge._draft.snapshot = snapshot
            snapshot(entry)
            error("failed new preview")
        end
    "#,
        )
        .unwrap();
    assert!(engine
        .call("item_draft_create", &json!({"raw":RING,"normalise":false}))
        .unwrap_err()
        .to_string()
        .contains("failed new preview"));
    assert_eq!(engine.eval("return __bridge._draft.count").unwrap(), 64);
    let active = edit(
        &engine,
        &drafts[0],
        json!({"operation":"props","itemLevel":81}),
    );
    engine
        .call("item_modifier_options", &target(&active))
        .unwrap();
    engine.call("item_tooltip", &target(&drafts[2])).unwrap();
    let replacement = create(&engine, RING);
    assert_eq!(engine.eval("return __bridge._draft.count").unwrap(), 64);
    assert_eq!(
        engine
            .eval("return __bridge._draft.byItem[__evictedDraftItem] == nil")
            .unwrap(),
        true
    );
    engine
        .eval(
            r#"
        __evictedDraftWeak = setmetatable({ __evictedDraftItem }, { __mode = "v" })
        __evictedDraftItem = nil
        return true
    "#,
        )
        .unwrap();
    assert_eq!(
        engine
            .eval("jit.flush(); collectgarbage('collect'); return __evictedDraftWeak[1] == nil")
            .unwrap(),
        true
    );
    assert!(engine
        .call("item_draft_get", &target(&drafts[1]))
        .unwrap_err()
        .to_string()
        .contains("item preview expired"));
    engine
        .call(
            "item_draft_dispose",
            &json!({"draftId":drafts[1]["draftId"]}),
        )
        .unwrap();
    assert_eq!(engine.eval("return __bridge._draft.count").unwrap(), 64);
    assert_eq!(
        engine.call("item_draft_get", &target(&active)).unwrap(),
        active
    );
    assert_eq!(
        engine.call("item_draft_get", &target(&drafts[2])).unwrap(),
        drafts[2]
    );
    assert_eq!(
        engine
            .call("item_draft_get", &target(&replacement))
            .unwrap(),
        replacement
    );
    create(&engine, RING);
    assert_eq!(engine.eval("return __bridge._draft.count").unwrap(), 64);
    assert!(engine.call("item_draft_get", &target(&drafts[3])).is_err());
    assert_eq!(
        engine.call("item_draft_get", &target(&active)).unwrap(),
        active
    );
    assert_eq!(build_state(&engine), before);
    assert_eq!(
        engine
            .call("item_raw", &json!({"itemId":saved["itemId"]}))
            .unwrap(),
        saved_raw
    );
    let mut commit = target(&active);
    commit["buildRevision"] = active["rev"].clone();
    commit["equip"] = json!(false);
    engine.call("item_draft_commit", &commit).unwrap();
    assert_eq!(engine.eval("return __bridge._draft.count").unwrap(), 63);
    engine
        .call(
            "item_draft_dispose",
            &json!({"draftId":replacement["draftId"]}),
        )
        .unwrap();
    assert_eq!(engine.eval("return __bridge._draft.count").unwrap(), 62);
    engine
        .call("new_build", &json!({"name":"After eviction"}))
        .unwrap();
    assert_eq!(engine.eval("return __bridge._draft.count").unwrap(), 0);
    assert!(engine
        .call("item_draft_get", &target(&replacement))
        .is_err());
    create(&engine, RING);
    assert_eq!(engine.eval("return __bridge._draft.count").unwrap(), 1);
    drop(engine);
    std::fs::remove_dir_all(user_dir).unwrap();
}

#[test]
fn draft_handles_are_disposed_and_scoped_to_build_and_engine_lifetimes() {
    let Some((engine, user_dir)) = boot("lifetime") else {
        return;
    };
    let Some((peer, peer_dir)) = boot("peer") else {
        return;
    };
    let before = build_state(&engine);
    assert_eq!(create(&engine, "this is not an item"), Value::Null);
    let draft = create(&engine, RING);
    let peer_draft = create(&peer, RING);
    assert_eq!(draft["generation"], peer_draft["generation"]);
    assert_ne!(draft["draftId"], peer_draft["draftId"]);
    assert!(peer.call("item_draft_get", &target(&draft)).is_err());
    for _ in 0..2 {
        engine
            .call("item_draft_dispose", &json!({"draftId":draft["draftId"]}))
            .unwrap();
    }
    assert!(engine.call("item_draft_get", &target(&draft)).is_err());
    assert_eq!(build_state(&engine), before);
    let first = create(&engine, RING);
    let second = create(&engine, "Rarity: Normal\nIron Ring");
    engine
        .call("item_draft_dispose", &json!({"draftId":first["draftId"]}))
        .unwrap();
    assert_eq!(
        engine.call("item_draft_get", &target(&second)).unwrap(),
        second
    );
    let mut commit = target(&second);
    commit["buildRevision"] = second["rev"].clone();
    commit["equip"] = json!(false);
    let saved = engine.call("item_draft_commit", &commit).unwrap();
    assert_eq!(
        engine
            .call("item_raw", &json!({"itemId":saved["itemId"]}))
            .unwrap()["raw"],
        second["raw"]
    );
    let abandoned = create(&engine, RING);
    engine
        .call("new_build", &json!({"name":"Replacement"}))
        .unwrap();
    assert!(engine.call("item_draft_get", &target(&abandoned)).is_err());
    let replacement = create(&engine, RING);
    let mut forged_generation = target(&abandoned);
    forged_generation["generation"] = replacement["generation"].clone();
    assert!(engine.call("item_draft_get", &forged_generation).is_err());
    engine
        .call(
            "item_draft_dispose",
            &json!({"draftId":abandoned["draftId"]}),
        )
        .unwrap();
    assert_eq!(
        engine
            .call("item_draft_get", &target(&replacement))
            .unwrap(),
        replacement
    );
    drop(peer);
    drop(engine);
    std::fs::remove_dir_all(peer_dir).unwrap();
    std::fs::remove_dir_all(user_dir).unwrap();
}

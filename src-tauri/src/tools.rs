//! The MCP tool registry: tool schemas and their bodies. Two front doors drive
//! it — the rmcp transport in `mcp.rs`, and the in-app chat panel through the
//! `ai_tools` / `ai_call_tool` commands in `lib.rs`.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use pob_engine::{EngineHandle, EnginePool};
use serde::Serialize;
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Emitter};

use crate::game::Game;

/// Same shape as `rmcp::model::JsonObject`, without depending on rmcp here.
pub(crate) type JsonObject = Map<String, Value>;

const INSTRUCTIONS: &str = "This server drives the build that is open in PoB Redux (Path of Building for \
Path of Exile 2). Every number comes from Path of Building's own calculation engine, and every change is \
shown in the app immediately. Start with get_character and get_stats to see what is loaded, or load_build \
to open a share code, a pobb.in / Maxroll / poe.ninja / poe2db.tw / Pastebin / Rentry link, a local .xml \
or .build file, or raw PoB XML. For advice or a fix, start with build_summary — one call gives the level, \
main skill and its support count, every skill group with whether it needs a keypress, the passive point \
budget for that level, spirit and its reservation, charm slots and resistances — then sanity_check for \
ranked findings with fixes. The library tool holds game knowledge PoB does not carry (point budgets, gem \
levels, what published builds do, what belongs in each gear slot); read advising-builds before a \
recommendation and the topic that matches the question. \
Set the level with set_level before anything else if the user names one: it changes every number, and gates \
which gems exist. Inspect further with get_stats / list_stat_keys / get_sidebar / get_tree_state / \
get_items / get_skills / get_config. Explore the passive tree with search_tree, node_info and \
node_path_cost; tree_suggest scores every reachable node for one stat and returns the best unallocated \
ones per point plus the weakest allocated ones. To reach a notable, find it with search_tree, then path_plan \
for a route — it takes an objective (defence, damage, speed, attributes, or a stat substring) and a max_extra \
point budget, so \"path to X optimising for defence\" is one call — and alloc_path to take it. Most of this \
tree is attribute nodes, so call set_attribute_choice (1 Str, 2 Dex, 3 Int) before pathing when the user \
says which they want. Change the build with alloc_node / dealloc_node / select_class / set_level, \
equip_item_raw / unequip_item, add_gem / set_gem / remove_gem / set_main_skill, and set_config. For better \
gear, optimise_gear searches the real mod pool for every slot and scores each candidate with PoB, keeping \
resistances capped; apply its proposals with `apply` or equip_item_raw. For unique jewels, suggest_unique_jewels \
scores every one PoB knows in every allocated socket, variants included, and ranks them; equip a pick with \
equip_from_item_db and its `variants`. For one specific item, list_bases \
then list_affixes for the mod pool, then craft_rare, which builds it from PoB's own affix tables. Before a \
run of changes call checkpoint; every write returns `stats` and a `delta` \
against the previous state, and rollback restores a checkpoint if the result is worse. Manage alternate \
trees and gear sets with the list/select/create/copy/rename/delete _spec and _item_set tools. Use \
save_build or export_build to persist the result. The user is watching the app while you work.";

const INSTRUCTIONS_POE1: &str = "This server drives the build that is open in PoB Redux (Path of Building for \
Path of Exile 1). Every number comes from Path of Building's own calculation engine, and every change is \
shown in the app immediately. Start with get_character and get_stats to see what is loaded, or load_build \
to open a share code, a pobb.in / Maxroll / Mobalytics / poe.ninja / Pastebin / Rentry link, a local .xml \
file, or raw PoB XML. For advice or a fix, start with build_summary — one call gives the level, main skill \
and its support count, every skill with whether it needs a keypress (auras, heralds and other skills that \
reserve count as persistent), the passive point budget for that level, mana and life reservation, flasks, \
bandit and pantheon, and resistances — then sanity_check for ranked findings with fixes. \
Set the level with set_level before anything else if the user names one: it changes every number, and gates \
which gem levels the character can use. Inspect further with get_stats / list_stat_keys / get_sidebar / \
get_tree_state / get_items / get_skills / get_config. Explore the passive tree with search_tree, node_info and \
node_path_cost; tree_suggest scores every reachable node for one stat and returns the best unallocated \
ones per point plus the weakest allocated ones. To reach a notable, find it with search_tree, then path_plan \
for a route — it takes an objective (defence, damage, speed, attributes, or a stat substring) and a max_extra \
point budget, so \"path to X optimising for defence\" is one call — and alloc_path to take it. A mastery needs \
an effect: node_info lists them, and alloc_node takes the chosen one as `effect`. Change the build with \
alloc_node / dealloc_node / select_class / set_level, equip_item_raw / unequip_item, add_gem / set_gem / \
remove_gem / set_main_skill, and set_config, which also holds the bandit and pantheon choices. For better \
gear, optimise_gear searches the real mod pool for every slot and scores each candidate with PoB, keeping \
resistances capped; apply its proposals with `apply` or equip_item_raw. For unique jewels, suggest_unique_jewels \
scores every one PoB knows in every allocated socket, variants included, and ranks them; equip a pick with \
equip_from_item_db and its `variants`. For one specific item, list_bases \
then list_affixes for the mod pool, then craft_rare, which builds it from PoB's own affix tables. Before a \
run of changes call checkpoint; every write returns `stats` and a `delta` \
against the previous state, and rollback restores a checkpoint if the result is worse. Manage alternate \
trees and gear sets with the list/select/create/copy/rename/delete _spec and _item_set tools. Use \
save_build or export_build to persist the result. The user is watching the app while you work.";

pub(crate) fn instructions(game: Game) -> &'static str {
    match game {
        Game::Poe1 => INSTRUCTIONS_POE1,
        Game::Poe2 => INSTRUCTIONS,
    }
}

const HEADLINE: &[&str] = &[
    "Life",
    "EnergyShield",
    "Mana",
    "Spirit",
    "Armour",
    "Evasion",
    "TotalEHP",
    "TotalDPS",
    "CombinedDPS",
    "FullDPS",
    "FireResist",
    "ColdResist",
    "LightningResist",
    "ChaosResist",
    "Str",
    "Dex",
    "Int",
];

pub struct ToolContext {
    pub(crate) engine: EngineHandle,
    pub(crate) pool: Arc<EnginePool>,

    pub(crate) app: AppHandle,
    pub(crate) calls: Arc<AtomicU64>,
}

pub(crate) enum ToolError {
    /// Bad arguments: reported as a JSON-RPC invalid-params error.
    Invalid(String),
    /// The engine rejected the operation: reported as a tool error the model can read.
    Failed(String),
}

impl ToolContext {
    fn call(&self, method: &str, params: Value) -> Result<Value, ToolError> {
        self.engine
            .call(method, params)
            .map(|o| o.result)
            .map_err(|e| ToolError::Failed(clean_error(&e.to_string())))
    }

    pub(crate) fn game(&self) -> Game {
        tauri::Manager::state::<crate::AppState>(&self.app).game()
    }

    fn headline(&self) -> Value {
        let poe1 = self.game() == Game::Poe1;
        let fields: Vec<&str> = HEADLINE.iter().map(|f| if poe1 && *f == "Spirit" { "ManaUnreserved" } else { *f }).collect();
        self.call("get_stats", json!({ "fields": fields }))
            .ok()
            .and_then(|v| v.get("stats").cloned())
            .unwrap_or(Value::Null)
    }

    /// Attach the headline stats to a write's result, with the change against
    /// `before` for every number that moved, so a caller can judge an edit
    /// from the result alone.
    fn with_stats(&self, mut v: Value, before: Option<&Value>) -> Value {
        let after = self.headline();
        let delta = before.map(|b| stat_delta(b, &after)).unwrap_or(Value::Null);
        match &mut v {
            Value::Object(m) => {
                m.insert("stats".into(), after);
                m.insert("delta".into(), delta);
                v
            }
            _ => json!({ "result": v, "stats": after, "delta": delta }),
        }
    }
}

fn stat_delta(before: &Value, after: &Value) -> Value {
    let (Some(b), Some(a)) = (before.as_object(), after.as_object()) else {
        return Value::Null;
    };
    let mut out = Map::new();
    for (k, av) in a {
        let (Some(x), Some(y)) = (b.get(k).and_then(Value::as_f64), av.as_f64()) else { continue };
        let d = y - x;
        if d.abs() > 1e-6 {
            let rounded = (d * 100.0).round() / 100.0;
            out.insert(k.clone(), json!(rounded));
        }
    }
    Value::Object(out)
}

fn is_read_only(name: &str) -> bool {
    defs(Game::Poe2).iter().any(|d| d.name == name && d.read_only)
}

// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
pub(crate) struct ToolDef {
    pub(crate) name: &'static str,
    pub(crate) description: String,
    pub(crate) schema: Value,
    pub(crate) output_schema: Option<Value>,
    pub(crate) read_only: bool,
    pub(crate) destructive: bool,
    pub(crate) idempotent: bool,
    pub(crate) open_world: bool,
    /// Seconds rather than milliseconds: run as a task where the client allows it.
    pub(crate) slow: bool,
}

impl ToolDef {
    fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    fn idempotent(mut self) -> Self {
        self.idempotent = true;
        self
    }

    fn open_world(mut self) -> Self {
        self.open_world = true;
        self
    }

    fn slow(mut self) -> Self {
        self.slow = true;
        self
    }

    /// For writes that return the bridge result unwrapped, with no `stats`/`delta`.
    fn no_output_schema(mut self) -> Self {
        self.output_schema = None;
        self
    }
}

fn obj(props: Value, required: &[&str]) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), json!("object"));
    m.insert("properties".into(), props);
    if !required.is_empty() {
        m.insert("required".into(), json!(required));
    }
    Value::Object(m)
}

fn prop(ty: &str, desc: &str) -> Value {
    json!({ "type": ty, "description": desc })
}

fn none() -> Value {
    json!({ "type": "object", "additionalProperties": false })
}

/// What `ToolContext::with_stats` adds to every write.
fn write_output() -> Value {
    json!({
        "type": "object",
        "properties": {
            "stats": { "type": ["object", "null"], "description": "Headline stats after the change" },
            "delta": { "type": ["object", "null"], "description": "Change against the previous state, per stat that moved" }
        },
        "additionalProperties": true
    })
}

pub(crate) fn defs(game: Game) -> Vec<ToolDef> {
    let mut all = defs_poe2();
    if game == Game::Poe1 {
        adapt_for_poe1(&mut all);
    }
    all
}

fn defs_poe2() -> Vec<ToolDef> {
    let ro = |name, description: &str, schema| ToolDef { name, description: description.into(), schema, output_schema: None, read_only: true, destructive: false, idempotent: false, open_world: false, slow: false };
    let rw = |name, description: &str, schema| ToolDef { name, description: description.into(), schema, output_schema: Some(write_output()), read_only: false, destructive: false, idempotent: false, open_world: false, slow: false };
    let del = |name, description: &str, schema| ToolDef { name, description: description.into(), schema, output_schema: Some(write_output()), read_only: false, destructive: true, idempotent: false, open_world: false, slow: false };
    let index = |what: &str| prop("integer", &format!("1-based index of the {what}"));
    let group_index = || prop("integer", "1-based socket group index (see get_skills)");
    let gem_index = || prop("integer", "1-based gem index within the group (see get_skills)");
    let node_id = || prop("integer", "Passive tree node id");
    let title = || prop("string", "Display name");
    let activates = "The new one becomes active.";

    vec![
        // Build
        rw(
            "load_build",
            "Open a build in the app, replacing the one that is open. `source` may be a PoB share code, a pobb.in / Maxroll / Mobalytics / poe.ninja / poe2db.tw / Pastebin / Rentry link (a Mobalytics build page loads the PoB code its author attached), a local path to a .xml build or a GGG .build planner file, or raw PoB build XML.",
            obj(json!({ "source": prop("string", "Share code, link, file path, or XML"), "name": prop("string", "Build name to use (optional)") }), &["source"]),
        ).open_world().destructive(),
        rw("new_build", "Start a blank build (default class, no items or skills). Replaces the open build.", obj(json!({ "name": prop("string", "Build name") }), &[])).destructive(),
        ro("list_local_builds", "List the .xml builds in the user's Path of Building builds folder. Paths can be passed to load_build.", none()),
        rw(
            "save_build",
            "Save the open build to disk as PoB XML. Uses the build's own file unless `path` is given.",
            obj(json!({ "path": prop("string", "Absolute path of the .xml file to write (optional)") }), &[]),
        ).no_output_schema(),
        ro(
            "export_build",
            "Export the open build as a shareable PoB code (default) or as full PoB XML.",
            obj(json!({ "format": { "type": "string", "enum": ["code", "xml"], "description": "Output format (default: code)" } }), &[]),
        ),
        // Character
        ro("get_character", "Class, ascendancy, level, passive points used, main skill group, and file name of the open build.", none()),
        rw("set_level", "Set the character level (1 to 100).", obj(json!({ "level": prop("integer", "Character level") }), &["level"])).idempotent(),
        ro("list_classes", "Every class and its ascendancies with ids for select_class.", none()),
        rw(
            "select_class",
            "Change class and/or ascendancy. Omit an id to leave it unchanged. Changing class deallocates nodes the new class cannot reach. An invalid id leaves the build untouched.",
            obj(json!({ "class_id": prop("integer", "Class id from list_classes"), "ascend_class_id": prop("integer", "Ascendancy id from list_classes (0 for none)") }), &[]),
        ).idempotent(),
        // Stats
        ro(
            "get_stats",
            "Calculated stats (life, ES, mana, resistances, DPS, EHP, and hundreds more) from PoB's engine. Pass `fields` to get only those keys; omit it for every scalar stat. Key names vary by build; list_stat_keys shows what exists.",
            obj(json!({ "fields": { "type": "array", "items": { "type": "string" }, "description": "Stat keys to return" } }), &[]),
        ),
        ro("list_stat_keys", "Every stat key get_stats can return for the open build.", none()),
        ro("get_sidebar", "The stat panel exactly as the app shows it: labelled rows plus PoB's warnings. Good for a quick human-style summary.", none()),
        ro(
            "sanity_check",
            "Review the open build and return ranked findings, each with `severity` (high/medium/low), `area`, `message` and a suggested `fix`. Covers resistances, the passive point budget against the character's level, ascendancy points, support count on the main skill, spirit reservation, charm slots (empty, or more charms than the belt allows), life and energy shield for the level, movement speed, unused weapon set points, unmet attribute requirements with the item or gem that sets them, affixes spent on reduced attribute requirements, and gem errors. Run it again after a change: it is the cheapest check that the change did not break something else. An empty list is not proof the build is sound, and a finding is about numbers only: it cannot see how skills interact in play.",
            none(),
        ),
        ro(
            "build_summary",
            "One compact snapshot of the open build: level, class, ascendancy, main skill and its support count, every skill group as `skills` (group index, skill, `press` = active/persistent/trigger/meta/granted, support count, enabled, main, `granted` when the game hands the skill out with a weapon or item such as Mace Strike or Raise Shield, `grantedBy` for an item's own copy and `duplicateOf` pointing at the socketed group that carries its supports), how many skills need a keypress (`activeSkills`, which leaves granted skills out; they are in `grantedSkills`), passive points used against the budget available at that level, ascendancy and weapon set points, life, energy shield, mana, spirit and its reservation, charm slots, resistances, attributes, `requirements` (per attribute: need, have, met, and the one item, gem or support-gem source that sets it; requirements are the highest single source, never a sum), movement speed, DPS, `keystones` (allocated or granted by items) and `keystoneRules`: plain rules for keystones that change which lines matter, such as life fixed at 1, energy shield becoming mana, mana taking damage first, no mana, or evasion becoming armour. Follow keystoneRules before recommending a line. Only `active` skills cost a keypress. Prefer this over several get_stats calls when starting to advise on a build.",
            none(),
        ),
        ro(
            "library",
            &format!(
                "Game knowledge PoB does not carry, written for advising on builds. Pass `topic` to read one; omit it for the index. Topics: {}.",
                crate::library::index()
            ),
            obj(json!({ "topic": prop("string", "Topic slug from the list, or a word from its description") }), &[]),
        ),
        ro(
            "checkpoint",
            "Snapshot the whole build in memory under a label, so a run of edits can be undone with rollback. Tree edits have undo; gear, gem and config edits do not, so call this before changing them. Returns the labels that exist.",
            obj(json!({ "label": prop("string", "Name for the snapshot (default: numbered)") }), &[]),
        ),
        rw(
            "rollback",
            "Restore a checkpoint, replacing the open build with it. Without a label, the most recent checkpoint. Returns the life, DPS and EHP the checkpoint had.",
            obj(json!({ "label": prop("string", "Checkpoint label (default: the latest)") }), &[]),
        ),
        // Tree
        ro(
            "get_tree_state",
            "Allocated passive nodes of the active tree: node ids, class ids, and node overrides, plus the point accounting — points used, ascendancy and weapon set points, jewel sockets, and the budget available at the character's level. The budget is a range because quest points depend on campaign progress rather than level. Use node_info for details on any id.",
            none(),
        ),
        ro(
            "search_tree",
            "Search the passive tree of the open build by name or stat text, with optional type and ascendancy filters.",
            obj(
                json!({
                    "query": prop("string", "Case-insensitive substring matched against node names and stats"),
                    "node_type": { "type": "string", "enum": ["Normal", "Notable", "Keystone", "Mastery", "Socket", "ClassStart", "AscendClassStart"], "description": "Only nodes of this type" },
                    "ascendancy_name": prop("string", "Only nodes of this ascendancy"),
                    "main_tree_only": prop("boolean", "Exclude every ascendancy node"),
                    "limit": prop("integer", "Maximum results (default 200)")
                }),
                &[],
            ),
        ),
        ro(
            "tree_suggest",
            "Score every main-tree node for one stat by running PoB's calculation with it allocated (or, for allocated nodes, as it stands), and return the best unallocated nodes by gain per point along their path (`bestToAdd`) plus the allocated nodes the build would miss least (`weakestAllocated`: `lossIfRemoved` and how many allocated nodes depend on each; a path node with dependents cannot go alone). `stat` is a PoB output key: Life, TotalEHP, Armour, Evasion, EnergyShield, CombinedDPS, TotalDPS, FullDPS, AverageDamage, Speed, CritChance, BlockChance, EffectiveMovementSpeedMod, Str, Dex, Int, LifeRegen, LifeLeechRate, Mana, and the *TakenHit keys; list_power_stats has them all. Takes a few seconds. Path costs are from the current tree; use path_plan or alloc_node to take one.",
            obj(
                json!({
                    "stat": prop("string", "Output key to score (default CombinedDPS)"),
                    "limit": prop("integer", "Rows per list (default 15)"),
                    "max_points": prop("integer", "Only unallocated nodes within this many points (default 8)"),
                    "node_type": { "type": "string", "enum": ["Notable", "Keystone", "Normal"], "description": "Only nodes of this type (default: all)" }
                }),
                &[],
            ),
        ).slow(),
        ro("list_power_stats", "Every stat tree_suggest can score.", none()),
        ro("node_info", "Name, type, stats, mods, allocation state, and path cost of one node.", obj(json!({ "node_id": node_id() }), &["node_id"])),
        ro("node_path_cost", "How many points allocating a node would cost from the current tree, and the path PoB would take. Does not allocate.", obj(json!({ "node_id": node_id() }), &["node_id"])),
        ro(
            "path_plan",
            "Plan a route from the allocated tree to a node, preferring intermediate nodes that serve an objective. \
`objective` is \"short\" (fewest points, the default), \"defence\", \"damage\", \"speed\", \"attributes\", or any stat \
substring such as \"mana\". `max_extra` permits that many points beyond the shortest route when they buy more of the \
objective — 3 to 5 is usually where a route starts picking up real nodes. Changes nothing; pass the returned node ids \
to alloc_path. Set the attribute choice first if the route crosses attribute nodes, which in this tree it usually does.",
            obj(
                json!({
                    "node_id": node_id(),
                    "objective": { "type": "string", "description": "short | defence | damage | speed | attributes, or a stat substring" },
                    "max_extra": prop("integer", "Points allowed beyond the shortest route (0-12, default 0)")
                }),
                &["node_id"],
            ),
        ),
        rw(
            "alloc_path",
            "Allocate a route from path_plan. Pass its node ids in order, destination last.",
            obj(
                json!({ "node_ids": { "type": "array", "items": { "type": "integer" }, "description": "Ordered node ids from path_plan" } }),
                &["node_ids"],
            ),
        ),
        rw(
            "set_attribute_choice",
            "Choose what the tree's switchable attribute nodes grant: 1 Strength, 2 Dexterity, 3 Intelligence. One \
setting for the whole tree. It applies to nodes allocated from then on, so set it before pathing; pass \
`apply_to_allocated` to rewrite the ones already taken.",
            obj(
                json!({
                    "attribute": prop("integer", "1 = Strength, 2 = Dexterity, 3 = Intelligence"),
                    "apply_to_allocated": prop("boolean", "Also switch attribute nodes already allocated")
                }),
                &["attribute"],
            ),
        ).idempotent(),
        rw("alloc_node", "Allocate a node and the shortest path to it, exactly as clicking it in the tree would, then recalculate.", obj(json!({ "node_id": node_id() }), &["node_id"])),
        rw("dealloc_node", "Deallocate a node and every node that depended on it for connectivity, then recalculate.", obj(json!({ "node_id": node_id() }), &["node_id"])),
        rw("tree_undo", "Undo the last tree change.", none()),
        ro("export_tree_url", "The pathofexile.com passive tree URL for the active tree.", none()),
        rw("import_tree_url", "Replace the active tree's allocation from a pathofexile.com passive tree URL.", obj(json!({ "url": prop("string", "Passive tree URL") }), &["url"])).idempotent(),
        // Specs
        ro("list_specs", "The build's passive tree specs (alternate trees) and which one is active.", none()),
        rw("select_spec", "Switch the active tree spec. Every tree tool then works on it.", obj(json!({ "index": index("spec (see list_specs)") }), &["index"])).idempotent(),
        rw("create_spec", &format!("Create a blank tree spec with the current class. {activates}"), obj(json!({ "title": title() }), &[])),
        rw("copy_spec", &format!("Duplicate a tree spec (default: the active one). {activates}"), obj(json!({ "index": index("spec to copy"), "title": title() }), &[])),
        rw("rename_spec", "Rename a tree spec.", obj(json!({ "index": index("spec"), "title": title() }), &["index", "title"])).no_output_schema().idempotent(),
        del("delete_spec", "Delete a tree spec. Fails if it is the only one.", obj(json!({ "index": index("spec") }), &["index"])),
        // Items
        ro("get_items", "Every visible equipment, flask, charm and jewel slot with the item in it (if any). Hidden and inactive slots are left out.", none()),
        ro("list_items", "Every item the build owns, equipped or not, with its id and slot.", none()),
        rw(
            "equip_item_raw",
            "Parse in-game item text (as copied from the game or written in PoB's item format) and equip it. Without `slot`, the first slot the item fits is used.",
            obj(json!({ "item_text": prop("string", "Raw item text"), "slot": prop("string", "Slot name from get_items") }), &["item_text"]),
        ),
        rw("equip_item", "Equip an item the build already owns (see list_items) into a slot.", obj(json!({ "item_id": prop("integer", "Item id from list_items"), "slot": prop("string", "Slot name from get_items") }), &["item_id", "slot"])).idempotent(),
        rw("unequip_item", "Empty a slot. The item stays in the build's item list.", obj(json!({ "slot": prop("string", "Slot name from get_items") }), &["slot"])).idempotent(),
        del("delete_item", "Remove an item from the build entirely.", obj(json!({ "item_id": prop("integer", "Item id from list_items") }), &["item_id"])),
        ro(
            "search_item_db",
            "Search PoB's unique item database (or its rare templates) by name or base. Each row carries the item's `implicits` and `mods` at its current selection, `variants` (how many exist), `variantPicks` (how many the item takes at once: a Megalomaniac takes 3 notables), the first `variantNames`, and `selectedVariants`. Paged: `total` says how many matched. For unique jewels, suggest_unique_jewels scores them against the build instead of guessing from the names.",
            obj(
                json!({
                    "query": prop("string", "Case-insensitive substring of the item or base name"),
                    "item_type": prop("string", "Only this item type, e.g. Boots"),
                    "db": { "type": "string", "enum": ["unique", "rare"], "description": "Database to search (default: unique)" },
                    "limit": prop("integer", "Maximum results (default 50)"),
                    "offset": prop("integer", "Skip this many results")
                }),
                &[],
            ),
        ),
        rw(
            "equip_from_item_db",
            "Equip an item from PoB's unique database (or a rare template) by its exact name. A unique with variants takes `variants`: one entry per pick (a variant's name, a substring of it, or its index), in the order suggest_unique_jewels or search_item_db lists them; without it the database default is equipped, which for a skill-level or notable jewel is the first alphabetical variant and almost never the one wanted. Returns the `variants` and `mods` it equipped with.",
            obj(
                json!({
                    "name": prop("string", "Exact item name from search_item_db"),
                    "db": { "type": "string", "enum": ["unique", "rare"] },
                    "slot": prop("string", "Slot name from get_items (optional)"),
                    "variants": { "type": "array", "items": { "type": "string" }, "description": "Variant per pick: name, substring or index" }
                }),
                &["name"],
            ),
        ),
        rw(
            "set_item_variant",
            "Change which variant an item already in the build uses (a legacy or current version, a Watcher's Eye's aura mods, a Loreweave's ring mods). `variants` takes one entry per pick, a variant's name, a substring of it, or its index; list_items shows each item's `variantNames` and `selectedVariants`. Returns the picks it set.",
            obj(
                json!({
                    "item_id": prop("integer", "Item id from list_items"),
                    "variants": { "type": "array", "items": { "type": "string" }, "description": "Variant per pick: name, substring or index" }
                }),
                &["item_id", "variants"],
            ),
        ),
        rw(
            "optimise_gear",
            "Design a rare for each chosen slot by greedy search over the affix families that roll on the slot's base, every candidate scored by PoB's calculation. An empty slot gets a base picked from the build: the defence type the character's attributes favour, the weapon type the main skill needs, a shield when a skill uses one, all within the character's level. Keeps resistances at 75, attribute requirements met and movement speed on boots, then maximises DPS (the minions' for a minion skill), life plus energy shield, and effective HP by the chosen aim. The balanced and defence aims never give up more than 5% effective HP for damage; only the damage aim may. Item level defaults to the character's level. Each proposal carries `lookFor` (the mod lines without numbers, what to look for in game), `base`, `baseReason`, `mods`, `raw` and a stat delta; `summary` says what happened. Nothing is equipped unless `apply` is true. Unique items are left alone, and a slot keeps its item when no designed rare scores higher (listed in `skipped`). Takes a few seconds for all slots. This is the tool for filling empty slots or improving gear; do not craft slot by slot.",
            obj(
                json!({
                    "aim": { "type": "string", "enum": ["balanced", "defence", "damage"], "description": "What to weight (default balanced)" },
                    "slots": { "type": "array", "items": { "type": "string" }, "description": "Slot names to optimise (default: every equipped non-unique slot)" },
                    "item_level": prop("integer", "Item level for the mod pool (default 82)"),
                    "range": prop("number", "Roll within each tier, 0 to 1 (default 1)"),
                    "apply": prop("boolean", "Equip every proposal (default false)")
                }),
                &[],
            ),
        ).slow(),
        ro(
            "suggest_unique_jewels",
            "Score every unique jewel PoB knows against the open build and rank them. Each is evaluated in every allocated jewel socket through PoB's own calculation (a radius jewel per socket, others once), and a jewel with variants is searched over the ones that can apply: a skill-level jewel over the skills the build runs, a notable jewel over notables not yet allocated, a multi-pick jewel filled greedily (best pick first), a stackable jewel also as 2 or 3 copies. Each suggestion carries the `socket`, the chosen `variants`, its `mods`, a stat `delta` against the build now, a `score` (log-ratio gain in DPS, life plus energy shield, and effective HP weighted by `aim`, minus penalties for resistances or attributes it breaks; 0.1 is roughly a 10% gain), `alternatives` (the next best single variants) and `raw` (item text for equip_item_raw). `notScored` lists jewels PoB's numbers cannot judge (tree-planning and Timeless jewels) with why. Nothing is equipped. Takes 5 to 20 seconds. This is the tool for \"which unique jewel should I use\"; do not equip jewels one by one to find out.",
            obj(
                json!({
                    "aim": { "type": "string", "enum": ["balanced", "defence", "damage"], "description": "What to weight (default balanced)" },
                    "sockets": { "type": "array", "items": { "type": "string" }, "description": "Only these sockets, as \"Socket #2\", \"#2\" or the slot name (default: every allocated socket)" },
                    "names": { "type": "array", "items": { "type": "string" }, "description": "Only jewels whose name contains one of these (default: all)" },
                    "range": prop("number", "Roll within each variable mod, 0 to 1 (default 0.5)"),
                    "limit": prop("integer", "Maximum suggestions returned (default 20)")
                }),
                &[],
            ),
        ).slow(),
        rw(
            "set_gem_levels",
            "Cap every skill gem at the highest level the character's level allows (tier ladder: level 40 allows level 10 gems, 58 allows 14, 90 allows 20). Imported planner builds carry max-level gems at every stage, which inflates attribute requirements and damage; call this after set_level on a levelling build.",
            obj(json!({ "level": prop("integer", "Character level to cap for (default: the build's level)") }), &[]),
        ).idempotent(),
        ro(
            "list_bases",
            "Item bases of one type with the numbers that decide between them: weapon damage, attack rate and crit; armour, evasion and energy shield; requirements; implicit; rune sockets. `type` is a family (Boots, Helmet, Ring, Two Hand Mace) or a typed list (Boots: Armour); omit it for the list of types. The best endgame bases are usually the highest requirement ones.",
            obj(json!({ "type": prop("string", "Item type or family"), "query": prop("string", "Substring of the base name"), "limit": prop("integer", "Maximum results (default 60)") }), &[]),
        ),
        ro(
            "list_affixes",
            "The mod pool for a base at an item level, from PoB's own affix tables: one row per mod family for prefixes and for suffixes, each with the best tier that item level allows (`modId`, `label` with its value range, `level` the tier needs, `tiers` available). Also gives how many prefixes and suffixes the base can hold. Use the `group` or the mod text with craft_rare.",
            obj(
                json!({
                    "type": prop("string", "Item type or family, as list_bases"),
                    "base_name": prop("string", "Base name from list_bases"),
                    "item_level": prop("integer", "Item level (default 82)"),
                    "query": prop("string", "Substring to filter mod text or family")
                }),
                &["type", "base_name"],
            ),
        ),
        rw(
            "craft_rare",
            "Build a rare item from a base and a list of wanted mods, then equip it. Every line comes from PoB's affix tables at the best tier the item level allows, so nothing is invented. Each wanted mod is a family name from list_affixes (IncreasedLife, MovementVelocity, FireResistance), a substring of the mod text (\"increased Physical Damage\"), or an exact modId. A base takes at most 3 prefixes and 3 suffixes; a family can appear once. `range` is the roll within the tier: 1 is a perfect roll, 0.5 the middle. Returns the mod lines, the item's requirements, its raw text, and the stat delta.",
            obj(
                json!({
                    "type": prop("string", "Item type or family, as list_bases"),
                    "base_name": prop("string", "Base name from list_bases"),
                    "title": prop("string", "Item name"),
                    "item_level": prop("integer", "Item level (default 82)"),
                    "prefixes": { "type": "array", "items": { "type": "string" }, "description": "Up to 3 wanted prefixes" },
                    "suffixes": { "type": "array", "items": { "type": "string" }, "description": "Up to 3 wanted suffixes" },
                    "range": prop("number", "Roll within each tier, 0 to 1 (default 1)"),
                    "runes": { "type": "array", "items": { "type": "string" }, "description": "Rune names for the item's sockets, in order (e.g. Iron Rune)" },
                    "slot": prop("string", "Slot to equip into (default: the first slot it fits)"),
                    "equip": prop("boolean", "Equip after crafting (default true)")
                }),
                &["type", "base_name"],
            ),
        ),
        // Item sets
        ro("list_item_sets", "The build's gear sets and which one is active.", none()),
        rw("select_item_set", "Switch the active gear set.", obj(json!({ "id": prop("integer", "Item set id from list_item_sets") }), &["id"])).idempotent(),
        rw("create_item_set", &format!("Create an empty gear set. {activates}"), obj(json!({ "title": title() }), &[])),
        rw("copy_item_set", &format!("Duplicate a gear set (default: the active one). {activates}"), obj(json!({ "id": prop("integer", "Item set id to copy"), "title": title() }), &[])),
        rw("rename_item_set", "Rename a gear set.", obj(json!({ "id": prop("integer", "Item set id"), "title": title() }), &["id", "title"])).no_output_schema().idempotent(),
        del("delete_item_set", "Delete a gear set. Fails if it is the only one.", obj(json!({ "id": prop("integer", "Item set id") }), &["id"])),
        // Skills
        ro("get_skills", "Socket groups, the gems in each (name, level, quality, enabled), and which group is the main skill.", none()),
        ro(
            "skill_info",
            "What a skill or support does, in PoB's own words: the gem's description, tags, cost, cooldown, reservation and stat lines at its level, with \"(Not supported in PoB yet)\" on any line the calculation ignores. Pass skill (the name of a socketed skill), or group_index (and gem_index for a support), or gem_id for any gem by id or name. Read this before removing or replacing a skill: a warcry, buff, trigger or mechanic can matter in play without moving the sheet numbers.",
            obj(json!({ "skill": prop("string", "Name of a socketed skill, e.g. Hammer of the Gods"), "group_index": group_index(), "gem_index": gem_index(), "gem_id": prop("string", "Gem id or display name of any gem, socketed or not") }), &[]),
        ),
        rw("add_socket_group", "Create an empty socket group. The first group of a build becomes the main skill.", obj(json!({ "label": prop("string", "Group label"), "slot": prop("string", "Item slot the group is socketed in") }), &[])),
        del(
            "remove_socket_group",
            "Delete a socket group and its gems. Address it by the name of its skill (safest) or by index.",
            obj(json!({ "skill": prop("string", "Name of the group's active skill, e.g. Infernal Cry"), "index": group_index() }), &[]),
        ),
        rw(
            "set_socket_group",
            "Change a socket group's label, slot, enabled state, Full DPS inclusion, or main active skill. Omit a field to leave it unchanged.",
            obj(
                json!({
                    "index": group_index(),
                    "skill": prop("string", "Name of the group's active skill, instead of index"),
                    "enabled": prop("boolean", "Enable or disable the group"),
                    "label": prop("string", "Group label"),
                    "slot": prop("string", "Item slot, or empty string for none"),
                    "include_in_full_dps": prop("boolean", "Count this group in Full DPS"),
                    "main_active_skill": prop("integer", "1-based index of the active skill within the group")
                }),
                &[],
            ),
        ).idempotent(),
        rw("set_main_skill", "Choose which socket group is the main skill for DPS, by skill name or group index.", obj(json!({ "skill": prop("string", "Name of the group's active skill"), "group_index": group_index() }), &[])).idempotent(),
        rw(
            "add_gem",
            "Add a gem to a socket group. Identify it by gem_id (internal id such as Metadata/Items/Gems/SkillGemFireball, from list_gems), by skill_id, or by name_spec (display name). Level defaults to the highest gem level the character's level allows, so requirements match the stage; pass `level` to override.",
            obj(
                json!({
                    "group_index": group_index(),
                    "gem_id": prop("string", "Gem id from list_gems"),
                    "skill_id": prop("string", "Skill id, for skills without a gem entry"),
                    "name_spec": prop("string", "Gem display name"),
                    "level": prop("integer", "Gem level"),
                    "quality": prop("integer", "Gem quality (0 to 20)")
                }),
                &["group_index"],
            ),
        ),
        rw(
            "set_gem",
            "Change a gem's level, quality, enabled state, or swap it for another gem id. Omit a field to leave it unchanged.",
            obj(
                json!({
                    "group_index": group_index(),
                    "gem_index": gem_index(),
                    "level": prop("integer", "Gem level"),
                    "quality": prop("integer", "Gem quality"),
                    "enabled": prop("boolean", "Enable or disable the gem"),
                    "gem_id": prop("string", "Replace with this gem id")
                }),
                &["group_index", "gem_index"],
            ),
        ).idempotent(),
        del("remove_gem", "Remove a gem from a socket group.", obj(json!({ "group_index": group_index(), "gem_index": gem_index() }), &["group_index", "gem_index"])),
        ro(
            "list_gems",
            "Find gem ids for add_gem. Matches the query against display names and ids. Returns `req_level` (the character level the gem needs, derived from its tier), `gem_level` (the highest gem level the character's level allows), `req_str` / `req_dex` / `req_int` (what that gem level asks of the character, by PoB's formula), `attr` (the gem's colour) and `base_sockets` (support sockets before Jeweller's Orbs, which scales with tier: 2 below tier 10, then 3, 4, and 5 at tier 20). Supports have no requirement of their own. **Gems above the open build's level are excluded by default** — set max_level to 0 to see them all, or to a number to plan for a future level. `short_by` names any attribute the build is missing.",
            obj(
                json!({
                    "query": prop("string", "Case-insensitive substring"),
                    "only_supports": prop("boolean", "true: support gems only; false: active gems only"),
                    "max_level": prop("integer", "Level cap for results. Omit to use the build's level; 0 for no cap"),
                    "limit": prop("integer", "Maximum results (default 50)"),
                }),
                &[],
            ),
        ),
        ro(
            "list_valid_supports",
            "Support gems PoB considers valid for a group's main active skill, excluding any above the character's level. Each carries its tier, req_level, `attr` (its colour) and whether it is already `socketed`. With `sort_by_dps`, PoB scores each one as if added to the group and returns `dps_delta` (CombinedDPS change, best first; takes about a second), which is how to choose supports on a damage skill. A support has no attribute requirement of its own: every support of one colour across the build forms a single requirement source of 5 each, and the character needs the highest single source, never the sum. `requirements` reports need, have and the binding source for each attribute.",
            obj(
                json!({
                    "group_index": group_index(),
                    "skill": prop("string", "Name of the group's active skill, instead of group_index"),
                    "sort_by_dps": prop("boolean", "Score each support's DPS change on this group and sort by it"),
                    "limit": prop("integer", "Maximum results (default: all)")
                }),
                &[],
            ),
        ),
        // Config
        ro("list_config_options", "Every configuration option (var, label, type, section, and list values) for set_config.", none()),
        ro("get_config", "Current configuration values of the active config set (buffs, enemy stats, map mods, and similar assumptions).", none()),
        rw(
            "set_config",
            "Set a configuration option. Pass null to reset it to its default. See list_config_options for var names and value types.",
            obj(json!({ "var": prop("string", "Option var from list_config_options"), "value": { "type": ["boolean", "number", "string", "null"], "description": "New value, or null to reset" } }), &["var"]),
        ).idempotent(),
        // Notes and loadouts
        ro("get_notes", "The build's notes text.", none()),
        rw("set_notes", "Replace the build's notes text.", obj(json!({ "text": prop("string", "Notes text") }), &["text"])).no_output_schema().idempotent(),
        ro("get_loadouts", "The build's loadouts (named tree + items + skills + config combinations) and which one is active.", none()),
        rw("select_loadout", "Activate a loadout by name.", obj(json!({ "name": prop("string", "Loadout name from get_loadouts") }), &["name"])).idempotent(),
    ]
}

/// PoE1 wording for the tools whose PoE2 text names PoE2 mechanics.
const POE1_TEXT: &[(&str, &str)] = &[
    ("load_build", "Open a build in the app, replacing the one that is open. `source` may be a PoB share code, a pobb.in / Maxroll / Mobalytics / poe.ninja / Pastebin / Rentry link (a Mobalytics build page loads the PoB code its author attached), a local path to a .xml build, or raw PoB build XML."),
    ("sanity_check", "Review the open build and return ranked findings, each with `severity` (high/medium/low), `area`, `message` and a suggested `fix`. Covers resistances, the passive point budget against the character's level, ascendancy points, supports on the main skill (5-link from level 68, 6-link from 85, when the skill is alone in its link), a main skill PoB counts as unusable because reservation leaves too little mana or life for its cost, empty flask slots, missing pantheon gods, life and energy shield for the level, movement speed, unmet attribute requirements with the item or gem that sets them, affixes spent on reduced attribute requirements, and gem errors. Run it again after a change: it is the cheapest check that the change did not break something else. An empty list is not proof the build is sound, and a finding is about numbers only: it cannot see how skills interact in play."),
    ("build_summary", "One compact snapshot of the open build: level, class, ascendancy, main skill and its support count, every skill as `skills` (group index, skill, `press` = active/persistent/trigger/granted, support count, enabled, main, `grantedBy` for an item's skill and `duplicateOf` pointing at a socketed copy); a link can hold several skills and each is listed. `persistent` means it reserves mana or life (auras, heralds, golems, spectres, stances); `trigger` means a trigger support, a skill or an item fires it. Also: how many skills need a keypress (`activeSkills`), passive points used against the budget available at that level, ascendancy points, life, energy shield, mana, mana and life reservation (`manaReserved`, `manaReservedPercent`, `manaUnreserved` and the life equivalents), flasks equipped and active, bandit and pantheon gods, resistances, attributes, `requirements` (per attribute: need, have, met, and the item or gem that sets it; the highest single source, never a sum), movement speed, DPS, `keystones` (allocated or granted by items) and `keystoneRules`: plain rules for keystones that change which lines matter. Follow keystoneRules before recommending a line. Only `active` skills cost a keypress. Prefer this over several get_stats calls when starting to advise on a build."),
    ("get_tree_state", "Allocated passive nodes of the active tree: node ids, class ids, and node overrides, plus the point accounting — points used, ascendancy points, jewel sockets, and the budget available at the character's level. The budget is a range because quest points depend on campaign progress rather than level. Use node_info for details on any id."),
    ("path_plan", "Plan a route from the allocated tree to a node, preferring intermediate nodes that serve an objective. `objective` is \"short\" (fewest points, the default), \"defence\", \"damage\", \"speed\", \"attributes\", or any stat substring such as \"mana\". `max_extra` permits that many points beyond the shortest route when they buy more of the objective — 3 to 5 is usually where a route starts picking up real nodes. Changes nothing; pass the returned node ids to alloc_path."),
    ("node_info", "Name, type, stats, mods, allocation state, and path cost of one node. For a mastery, `masteryEffects` lists each effect with the `effect` id alloc_node takes, and `takenBy` when another mastery already holds it."),
    ("alloc_node", "Allocate a node and the shortest path to it, exactly as clicking it in the tree would, then recalculate. A mastery needs `effect`, an id from node_info's masteryEffects; on an allocated mastery it changes the effect."),
    ("set_gem_levels", "Lower every gem, supports included, that the character's level cannot use to the highest level it can, from each gem level's own level requirement. Gems the level allows stay as they are, corrupted level 21 gems included. Call this after set_level on a levelling build."),
    ("list_gems", "Find gem ids for add_gem. Matches the query against display names and ids. Returns `req_level` (the character level the gem needs at gem level 1), `gem_level` (the highest gem level the character's level allows), `req_str` / `req_dex` / `req_int` (what that gem level asks of the character, by PoB's formula) and `attr` (the gem's colour). Legacy gems are left out, as in PoB's gem list. **Gems above the open build's level are excluded by default** — set max_level to 0 to see them all, or to a number to plan for a future level. `short_by` names any attribute the build is missing."),
    ("list_valid_supports", "Support gems PoB considers valid for a group's main active skill, excluding any above the character's level and legacy gems. Each carries its req_level, `attr` (its colour) and whether it is already `socketed`. With `sort_by_dps`, PoB scores each one as if added to the group and returns `dps_delta` (CombinedDPS change, best first; takes about a second), which is how to choose supports on a damage skill. Each support has its own attribute requirement by gem level; the character needs the highest single source, never the sum. `requirements` reports need, have and the binding source for each attribute."),
    ("get_items", "Every visible equipment, flask and jewel slot with the item in it (if any). Hidden and inactive slots are left out."),
    ("list_classes", "Every class and its ascendancies, and the bloodlines (`secondaryAscendancies`), with ids for select_class."),
    ("select_class", "Change class, ascendancy and/or bloodline. Omit an id to leave it unchanged. Changing class deallocates nodes the new class cannot reach. An invalid id leaves the build untouched."),
    ("list_bases", "Item bases of one type with the numbers that decide between them: weapon damage, attack rate and crit; armour, evasion and energy shield; requirements; implicit. `type` is a family (Boots, Helmet, Ring, Two Hand Mace) or a typed list (Boots: Armour); omit it for the list of types. The best endgame bases are usually the highest requirement ones."),
];

fn adapt_for_poe1(defs: &mut Vec<ToolDef>) {
    // No attribute nodes in the PoE1 tree; the library is PoE2's until PoE1 has one.
    defs.retain(|d| !matches!(d.name, "set_attribute_choice" | "library"));
    for d in defs.iter_mut() {
        if let Some((_, text)) = POE1_TEXT.iter().find(|(name, _)| *name == d.name) {
            d.description = (*text).to_string();
        }
        let props = &mut d.schema["properties"];
        if props.get("item_level").is_some() {
            props["item_level"]["description"] = json!(if d.name == "optimise_gear" {
                "Item level for the mod pool (default: the character's level, at most 86)"
            } else {
                "Item level (default 86)"
            });
        }
        match d.name {
            "alloc_node" => props["effect"] = prop("integer", "Mastery effect id from node_info's masteryEffects"),
            "select_class" => props["secondary_ascend_class_id"] = prop("integer", "Bloodline id from list_classes' secondaryAscendancies (0 for none)"),
            "craft_rare" => {
                if let Some(p) = props.as_object_mut() {
                    p.remove("runes");
                }
            }
            "optimise_gear" => d.description.push_str(" Flasks are left alone."),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Tool bodies
// ---------------------------------------------------------------------------

/// Engine errors carry a Lua traceback; the first line is the message.
fn clean_error(e: &str) -> String {
    let first = e.split("\nstack traceback").next().unwrap_or(e).trim();
    first.trim_start_matches("lua: ").trim_start_matches("runtime error: ").to_string()
}

fn arg_str(args: &JsonObject, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_string)
}

fn arg_i64(args: &JsonObject, key: &str) -> Result<Option<i64>, ToolError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f as i64))
            .map(Some)
            .ok_or_else(|| ToolError::Invalid(format!("{key} must be an integer"))),
        Some(Value::String(s)) => s
            .trim()
            .parse::<i64>()
            .map(Some)
            .map_err(|_| ToolError::Invalid(format!("{key} must be an integer"))),
        Some(_) => Err(ToolError::Invalid(format!("{key} must be an integer"))),
    }
}

fn arg_bool(args: &JsonObject, key: &str) -> Option<bool> {
    match args.get(key) {
        Some(Value::Bool(b)) => Some(*b),
        Some(Value::String(s)) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => Some(true),
            "false" | "no" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// A list argument. Small models send arrays as strings ("['Boots', 'Belt']",
/// "Boots, Belt"), so a string is parsed as JSON first and split on commas
/// otherwise; numbers stay numbers.
fn arg_list(args: &JsonObject, key: &str) -> Vec<Value> {
    match args.get(key) {
        Some(Value::Array(a)) => a.clone(),
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() {
                return Vec::new();
            }
            if let Ok(Value::Array(a)) = serde_json::from_str::<Value>(&t.replace('\'', "\"")) {
                return a;
            }
            t.trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .map(|p| p.trim().trim_matches(|c| c == '"' || c == '\''))
                .filter(|p| !p.is_empty())
                .map(|p| p.parse::<i64>().map(Value::from).unwrap_or_else(|_| Value::String(p.to_string())))
                .collect()
        }
        Some(Value::Number(n)) => vec![Value::Number(n.clone())],
        _ => Vec::new(),
    }
}

fn req_str(args: &JsonObject, key: &str) -> Result<String, ToolError> {
    arg_str(args, key)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ToolError::Invalid(format!("{key} is required")))
}

fn req_i64(args: &JsonObject, key: &str) -> Result<i64, ToolError> {
    arg_i64(args, key)?.ok_or_else(|| ToolError::Invalid(format!("{key} is required")))
}

/// Remove PoB colour escapes (`^7`, `^xRRGGBB`) from a display string.
fn strip_escapes(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < c.len() {
        if c[i] == '^' && i + 1 < c.len() {
            if c[i + 1] == 'x' && i + 8 <= c.len() && c[i + 2..i + 8].iter().all(|ch| ch.is_ascii_hexdigit()) {
                i += 8;
                continue;
            }
            if c[i + 1].is_ascii_digit() {
                i += 2;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// The single entry point for running a tool. Resolves share links (which need
/// the network) before handing the arguments to the blocking tool body, counts
/// the call, and emits `mcp:changed` on mutation. Both front doors — the rmcp
/// transport and the chat panel's `ai_call_tool` — go through here so they
/// cannot drift apart.
pub(crate) async fn dispatch(
    ctx: Arc<ToolContext>,
    name: String,
    mut args: JsonObject,
) -> Result<(Value, bool), ToolError> {
    if name == "load_build" {
        let url = args
            .get("source")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| s.starts_with("http://") || s.starts_with("https://"))
            .map(str::to_string);
        if let Some(url) = url {
            let fetched = crate::fetch_code(&url).await.map_err(ToolError::Failed)?;
            args.insert("source".into(), Value::String(fetched.code));
            args.insert("site".into(), Value::String(fetched.site));
        }
    }
    let run_ctx = ctx.clone();
    let tool = name.clone();
    let out = tokio::task::spawn_blocking(move || run_tool(&run_ctx, &tool, &args))
        .await
        .map_err(|e| ToolError::Failed(e.to_string()))?;
    ctx.calls.fetch_add(1, Ordering::Relaxed);
    if let Ok((_, true)) = out {
        let _ = ctx.app.emit("mcp:changed", json!({ "tool": name }));
    }
    out
}

/// Runs one tool. Returns the result and whether the build changed.
pub(crate) fn run_tool(ctx: &ToolContext, name: &str, args: &JsonObject) -> Result<(Value, bool), ToolError> {
    // Snapshot the headline before a write so the result can carry the change.
    let before = if is_read_only(name) { None } else { Some(ctx.headline()) };
    let stats = |v: Value| Ok((ctx.with_stats(v, before.as_ref()), true));
    let read = |v: Value| Ok((v, false));
    let tree_summary = |state: Value| -> Value {
        let points = ctx
            .call("get_build", Value::Null)
            .ok()
            .and_then(|b| b.get("points").cloned())
            .unwrap_or(Value::Null);
        json!({
            "allocatedNodeCount": state.get("allocatedNodeCount").cloned().unwrap_or(Value::Null),
            "pointsUsed": state.get("pointsUsed").cloned().unwrap_or(Value::Null),
            "points": points,
        })
    };
    match name {
        // Build
        "load_build" => {
            let source = req_str(args, "source")?;
            let name = arg_str(args, "name");
            let src = source.trim();
            let lower = src.to_ascii_lowercase();
            let is_path = (lower.ends_with(".xml") || lower.ends_with(".build")) && Path::new(src).is_file();
            let info = if src.starts_with('<') {
                ctx.call("load_build_xml", json!({ "xml": src, "name": name }))?
            } else if is_path && lower.ends_with(".build") {
                if ctx.game() == Game::Poe1 {
                    return Err(ToolError::Failed("A GGG .build planner file is a PoE2 build. Open a PoE1 build as a PoB code, a link or an .xml file.".into()));
                }
                let json = crate::read_text_lossy(src).map_err(ToolError::Failed)?;
                ctx.call("import_game_build", json!({ "json": json, "name": name }))?
            } else if is_path {
                ctx.call("load_build_file", json!({ "path": src }))?
            } else {
                ctx.call("load_build_code", json!({ "code": src, "name": name }))?
            };
            let mut out = json!({ "loaded": true, "build": info });
            if let Some(site) = arg_str(args, "site") {
                out["site"] = json!(site);
            }
            stats(out)
        }
        "new_build" => stats(ctx.call("new_build", json!({ "name": arg_str(args, "name") }))?),
        "list_local_builds" => {
            let dir = {
                use tauri::Manager;
                ctx.app.state::<crate::AppState>().builds_dir()
            };
            let builds = crate::scan_builds(&dir).map_err(ToolError::Failed)?;
            read(json!({
                "directory": dir.to_string_lossy(),
                "builds": serde_json::to_value(builds).unwrap_or(Value::Null),
            }))
        }
        "save_build" => {
            let params = match arg_str(args, "path") {
                Some(p) => json!({ "path": p }),
                None => Value::Null,
            };
            Ok((ctx.call("save_build_file", params)?, true))
        }
        "export_build" => {
            let format = arg_str(args, "format").unwrap_or_else(|| "code".into());
            match format.as_str() {
                "code" => read(ctx.call("save_build_code", Value::Null)?),
                "xml" => read(ctx.call("save_build_xml", Value::Null)?),
                _ => Err(ToolError::Invalid("format must be \"code\" or \"xml\"".into())),
            }
        }
        // Character
        "get_character" => read(ctx.call("get_build", Value::Null)?),
        "set_level" => stats(ctx.call("set_level", json!({ "level": req_i64(args, "level")? }))?),
        "list_classes" => read(ctx.call("list_classes", Value::Null)?),
        "select_class" => {
            let class_id = arg_i64(args, "class_id")?;
            let ascend = arg_i64(args, "ascend_class_id")?;
            let bloodline = arg_i64(args, "secondary_ascend_class_id")?;
            if class_id.is_none() && ascend.is_none() && bloodline.is_none() {
                return Err(ToolError::Invalid("class_id, ascend_class_id or secondary_ascend_class_id is required".into()));
            }
            let mut params = json!({ "classId": class_id, "ascendClassId": ascend });
            if let Some(id) = bloodline {
                params["secondaryAscendClassId"] = json!(id);
            }
            stats(ctx.call("select_class", params)?)
        }
        // Stats
        "get_stats" => {
            let fields = arg_list(args, "fields");
            let params = if fields.is_empty() { Value::Null } else { json!({ "fields": fields }) };
            read(ctx.call("get_stats", params)?)
        }
        "list_stat_keys" => read(ctx.call("list_stat_keys", Value::Null)?),
        "get_sidebar" => {
            let raw = ctx.call("get_sidebar", Value::Null)?;
            let rows: Vec<Value> = raw
                .get("rows")
                .and_then(Value::as_array)
                .map(|rows| {
                    rows.iter()
                        .filter_map(|r| {
                            let lhs = r.get("lhs").and_then(Value::as_str).map(strip_escapes);
                            let rhs = r.get("rhs").and_then(Value::as_str).map(strip_escapes);
                            match (lhs, rhs) {
                                (Some(l), Some(r)) if !l.trim().is_empty() => Some(json!({ "label": l.trim(), "value": r.trim() })),
                                (Some(l), None) if !l.trim().is_empty() => Some(json!({ "label": l.trim() })),
                                _ => None,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            let warnings: Vec<String> = raw
                .get("warnings")
                .and_then(Value::as_array)
                .map(|w| w.iter().filter_map(Value::as_str).map(strip_escapes).collect())
                .unwrap_or_default();
            read(json!({ "rows": rows, "warnings": warnings }))
        }
        "sanity_check" => read(ctx.call("sanity_check", Value::Null)?),
        "build_summary" => read(ctx.call("build_summary", Value::Null)?),
        "library" => {
            let topic = arg_str(args, "topic").filter(|t| !t.trim().is_empty());
            match topic {
                None => read(json!({
                    "topics": crate::library::TOPICS.iter().map(|t| json!({ "topic": t.slug, "covers": t.covers })).collect::<Vec<_>>()
                })),
                Some(t) => match crate::library::find(&t) {
                    Some(found) => read(json!({ "topic": found.slug, "text": found.text })),
                    None => Err(ToolError::Invalid(format!("no library topic matches {t:?}; topics: {}", crate::library::index()))),
                },
            }
        }
        "checkpoint" => read(ctx.call("checkpoint", json!({ "label": arg_str(args, "label") }))?),
        "rollback" => stats(ctx.call("rollback", json!({ "label": arg_str(args, "label") }))?),
        // Tree
        "get_tree_state" => read(ctx.call("get_tree_state", Value::Null)?),
        "search_tree" => {
            let ascendancy = match (arg_str(args, "ascendancy_name"), arg_bool(args, "main_tree_only")) {
                (Some(a), _) if !a.trim().is_empty() => json!(a),
                (_, Some(true)) => json!(false),
                _ => Value::Null,
            };
            read(ctx.call(
                "search_tree",
                json!({
                    "query": arg_str(args, "query"),
                    "type": arg_str(args, "node_type"),
                    "ascendancyName": ascendancy,
                    "limit": arg_i64(args, "limit")?.unwrap_or(200),
                }),
            )?)
        }
        "tree_suggest" => {
            let stat = arg_str(args, "stat").filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "CombinedDPS".into());
            let limit = arg_i64(args, "limit")?.unwrap_or(15).clamp(1, 60) as usize;
            let max_points = arg_i64(args, "max_points")?.unwrap_or(8).max(1) as f64;
            let node_type = arg_str(args, "node_type").filter(|s| !s.trim().is_empty());
            // allocated nodes are at distance 0, so a depth limit keeps every one of them
            let scored = match pob_engine::pool::power_scan(&ctx.engine, &ctx.pool, Some(&stat), Some(max_points)) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("parallel power scan failed ({e}); falling back to PowerBuilder");
                    ctx.call("tree_power", json!({ "stat": stat, "maxDepth": max_points }))?
                }
            };
            let report = scored.get("report").and_then(Value::as_array).cloned().unwrap_or_default();
            let f = |r: &Value, k: &str| r.get(k).and_then(Value::as_f64).unwrap_or(0.0);
            let keep_type = |r: &Value| node_type.as_deref().is_none_or(|t| r.get("type").and_then(Value::as_str) == Some(t));
            let ident = |r: &Value| {
                json!({
                    "id": r.get("id").cloned().unwrap_or(Value::Null),
                    "name": r.get("name").cloned().unwrap_or(Value::Null),
                    "type": r.get("type").cloned().unwrap_or(Value::Null),
                })
            };
            let mut add: Vec<&Value> = report
                .iter()
                .filter(|r| r.get("allocated") != Some(&Value::Bool(true)) && f(r, "pathPower") > 0.0 && f(r, "pathDist") <= max_points && keep_type(r))
                .collect();
            add.sort_by(|a, b| f(b, "pathPower").partial_cmp(&f(a, "pathPower")).unwrap_or(std::cmp::Ordering::Equal));
            let mut weakest: Vec<&Value> = report
                .iter()
                .filter(|r| r.get("allocated") == Some(&Value::Bool(true)) && keep_type(r))
                .collect();
            // PoB scores an allocated node as (stat without it - stat now), so the
            // least negative number is the node the build would miss least.
            weakest.sort_by(|a, b| f(b, "power").partial_cmp(&f(a, "power")).unwrap_or(std::cmp::Ordering::Equal));
            let best_rows: Vec<Value> = add
                .iter()
                .take(limit)
                .map(|r| {
                    let mut v = ident(r);
                    v["gainIfAllocated"] = json!(f(r, "power"));
                    v["gainPerPointOnPath"] = json!(f(r, "pathPower"));
                    v["pointsToReach"] = r.get("pathDist").cloned().unwrap_or(Value::Null);
                    v
                })
                .collect();
            let weak_rows: Vec<Value> = weakest
                .iter()
                .take(limit)
                .map(|r| {
                    let mut v = ident(r);
                    v["lossIfRemoved"] = json!(-f(r, "power"));
                    v["dependentNodes"] = r.get("pathDist").cloned().unwrap_or(Value::Null);
                    v
                })
                .collect();
            read(json!({
                "stat": scored.get("stat").cloned().unwrap_or(json!(stat)),
                "label": scored.get("label").cloned().unwrap_or(Value::Null),
                "bestToAdd": best_rows,
                "weakestAllocated": weak_rows,
                "scanned": report.len(),
                "ms": scored.get("ms").cloned().unwrap_or(Value::Null),
            }))
        }
        "list_power_stats" => read(ctx.call("power_stats", Value::Null)?),
        "node_info" => read(ctx.call("node_info", json!({ "id": req_i64(args, "node_id")? }))?),
        "node_path_cost" => read(ctx.call("node_path", json!({ "id": req_i64(args, "node_id")? }))?),
        "path_plan" => read(ctx.call(
            "path_plan",
            json!({
                "id": req_i64(args, "node_id")?,
                "objective": arg_str(args, "objective"),
                "max_extra": arg_i64(args, "max_extra")?,
            }),
        )?),
        "alloc_path" => {
            let ids = arg_list(args, "node_ids");
            if ids.is_empty() {
                return Err(ToolError::Invalid("node_ids must be a non-empty array of node ids".into()));
            }
            stats(ctx.call("alloc_trace", json!({ "ids": ids }))?)
        }
        "set_attribute_choice" => stats(ctx.call(
            "set_attribute_choice",
            json!({
                "attribute": req_i64(args, "attribute")?,
                "apply_to_allocated": arg_bool(args, "apply_to_allocated"),
            }),
        )?),
        "alloc_node" => {
            let mut params = json!({ "id": req_i64(args, "node_id")? });
            if let Some(effect) = arg_i64(args, "effect")? {
                params["effect"] = json!(effect);
            }
            let state = ctx.call("alloc_node", params)?;
            stats(tree_summary(state))
        }
        "dealloc_node" => {
            let state = ctx.call("dealloc_node", json!({ "id": req_i64(args, "node_id")? }))?;
            stats(tree_summary(state))
        }
        "tree_undo" => {
            let state = ctx.call("tree_undo", Value::Null)?;
            stats(tree_summary(state))
        }
        "export_tree_url" => read(ctx.call("export_tree_url", Value::Null)?),
        "import_tree_url" => {
            let state = ctx.call("import_tree_url", json!({ "url": req_str(args, "url")? }))?;
            stats(tree_summary(state))
        }
        // Specs
        "list_specs" => read(ctx.call("list_specs", Value::Null)?),
        "select_spec" => stats(ctx.call("select_spec", json!({ "index": req_i64(args, "index")? }))?),
        "create_spec" => stats(ctx.call("create_spec", json!({ "title": arg_str(args, "title") }))?),
        "copy_spec" => stats(ctx.call("copy_spec", json!({ "index": arg_i64(args, "index")?, "title": arg_str(args, "title") }))?),
        "rename_spec" => Ok((
            ctx.call("rename_spec", json!({ "index": req_i64(args, "index")?, "title": req_str(args, "title")? }))?,
            true,
        )),
        "delete_spec" => stats(ctx.call("delete_spec", json!({ "index": req_i64(args, "index")? }))?),
        // Items
        "get_items" => {
            let mut v = ctx.call("list_slots", Value::Null)?;
            if let Some(slots) = v.get_mut("slots").and_then(Value::as_array_mut) {
                slots.retain(|s| s.get("shown") != Some(&Value::Bool(false)) && s.get("inactive") != Some(&Value::Bool(true)));
            }
            read(v)
        }

        "list_items" => read(ctx.call("get_items", Value::Null)?),
        "equip_item_raw" => stats(ctx.call(
            "equip_item_raw",
            json!({ "text": req_str(args, "item_text")?, "slot": arg_str(args, "slot") }),
        )?),
        "equip_item" => stats(ctx.call(
            "equip_item",
            json!({ "itemId": req_i64(args, "item_id")?, "slot": req_str(args, "slot")? }),
        )?),
        "unequip_item" => stats(ctx.call("unequip_item", json!({ "slot": req_str(args, "slot")? }))?),
        "delete_item" => stats(ctx.call("delete_item", json!({ "itemId": req_i64(args, "item_id")? }))?),
        "search_item_db" => read(ctx.call(
            "item_db_list",
            json!({
                "query": arg_str(args, "query"),
                "type": arg_str(args, "item_type"),
                "db": arg_str(args, "db"),
                "limit": arg_i64(args, "limit")?.unwrap_or(50).max(1),
                "offset": arg_i64(args, "offset")?.unwrap_or(0).max(0),
            }),
        )?),
        "equip_from_item_db" => {
            let variants: Vec<Value> = arg_list(args, "variants").into_iter().filter(|v| v.as_str().is_some() || v.is_number()).collect();
            stats(ctx.call(
                "item_db_equip",
                json!({ "name": req_str(args, "name")?, "db": arg_str(args, "db"), "slotName": arg_str(args, "slot"), "variants": variants }),
            )?)
        }
        "set_item_variant" => {
            let picks: Vec<Value> = arg_list(args, "variants").into_iter().filter(|v| v.as_str().is_some() || v.is_number()).collect();
            stats(ctx.call("set_item_variant", json!({ "itemId": req_i64(args, "item_id")?, "picks": picks }))?)
        }
        "suggest_unique_jewels" => {
            let sockets: Vec<Value> = arg_list(args, "sockets").into_iter().filter(|v| v.as_str().is_some() || v.is_number()).collect();
            let names: Vec<Value> = arg_list(args, "names").into_iter().filter(|v| v.as_str().is_some()).collect();
            let params = json!({
                "preset": arg_str(args, "aim"),
                "sockets": sockets,
                "names": names,
                "range": args.get("range").and_then(Value::as_f64),
                "limit": arg_i64(args, "limit")?,
            });
            let result = match pob_engine::pool::jewel_scan(&ctx.engine, &ctx.pool, params.clone()) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("parallel jewel scan failed ({e}); scoring on the main engine");
                    ctx.call("suggest_unique_jewels", params)?
                }
            };
            read(result)
        }
        "optimise_gear" => {
            let slots: Vec<Value> = arg_list(args, "slots").into_iter().filter(|v| v.as_str().is_some()).collect();
            let mut result = ctx.call(
                "optimise_gear",
                json!({
                    "preset": arg_str(args, "aim"),
                    "slots": slots,
                    "itemLevel": arg_i64(args, "item_level")?,
                    "range": args.get("range").and_then(Value::as_f64),
                }),
            )?;
            // The raw text is what equips a proposal; the model does not need
            // the modId bookkeeping on top of the mod lines.
            if let Some(list) = result.get_mut("proposals").and_then(Value::as_array_mut) {
                for p in list.iter_mut() {
                    if let Some(m) = p.as_object_mut() {
                        m.remove("affixes");
                        m.remove("output");
                        m.remove("evaluations");
                    }
                }
            }
            if arg_bool(args, "apply") == Some(true) {
                let proposals: Vec<(String, String)> = result
                    .get("proposals")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|p| Some((p.get("slot")?.as_str()?.to_string(), p.get("raw")?.as_str()?.to_string())))
                            .collect()
                    })
                    .unwrap_or_default();
                for (slot, raw) in proposals {
                    ctx.call("equip_item_raw", json!({ "text": raw, "slot": slot }))?;
                }
                result["applied"] = json!(true);
                if let Some(summary) = result.get("summary").and_then(Value::as_str).map(|s| s.replace("proposed, not equipped yet", "proposed and equipped")) {
                    result["summary"] = json!(summary);
                }
                stats(result)
            } else {
                read(result)
            }
        }
        "set_gem_levels" => stats(ctx.call("set_gem_levels", json!({ "level": arg_i64(args, "level")? }))?),
        "list_bases" => read(ctx.call(
            "list_bases",
            json!({ "type": arg_str(args, "type"), "query": arg_str(args, "query"), "limit": arg_i64(args, "limit")?.unwrap_or(60) }),
        )?),
        "list_affixes" => read(ctx.call(
            "list_affixes",
            json!({
                "type": req_str(args, "type")?,
                "baseName": req_str(args, "base_name")?,
                "itemLevel": arg_i64(args, "item_level")?,
                "query": arg_str(args, "query"),
            }),
        )?),
        "craft_rare" => {
            let list = |key: &str| -> Vec<Value> { arg_list(args, key).into_iter().filter(|v| v.as_str().is_some_and(|s| !s.trim().is_empty())).collect() };
            stats(ctx.call(
                "craft_rare",
                json!({
                    "type": req_str(args, "type")?,
                    "baseName": req_str(args, "base_name")?,
                    "title": arg_str(args, "title"),
                    "itemLevel": arg_i64(args, "item_level")?,
                    "prefixes": list("prefixes"),
                    "suffixes": list("suffixes"),
                    "range": args.get("range").and_then(Value::as_f64),
                    "runes": list("runes"),
                    "slot": arg_str(args, "slot"),
                    "equip": arg_bool(args, "equip"),
                }),
            )?)
        }
        // Item sets
        "list_item_sets" => read(ctx.call("list_item_sets", Value::Null)?),
        "select_item_set" => stats(ctx.call("select_item_set", json!({ "id": req_i64(args, "id")? }))?),
        "create_item_set" => stats(ctx.call("create_item_set", json!({ "title": arg_str(args, "title") }))?),
        "copy_item_set" => stats(ctx.call("copy_item_set", json!({ "id": arg_i64(args, "id")?, "title": arg_str(args, "title") }))?),
        "rename_item_set" => Ok((
            ctx.call("rename_item_set", json!({ "id": req_i64(args, "id")?, "title": req_str(args, "title")? }))?,
            true,
        )),
        "delete_item_set" => stats(ctx.call("delete_item_set", json!({ "id": req_i64(args, "id")? }))?),
        // Skills
        "get_skills" => read(ctx.call("get_skills", Value::Null)?),
        "skill_info" => {
            let gem_id = arg_str(args, "gem_id").filter(|s| !s.trim().is_empty());
            let group = arg_i64(args, "group_index")?;
            let skill = arg_str(args, "skill").filter(|s| !s.trim().is_empty());
            if gem_id.is_none() && group.is_none() && skill.is_none() {
                return Err(ToolError::Invalid("skill, group_index or gem_id is required".into()));
            }
            read(ctx.call("skill_info", json!({ "gemId": gem_id, "groupIndex": group, "skill": skill, "gemIndex": arg_i64(args, "gem_index")? }))?)
        }
        "add_socket_group" => stats(ctx.call(
            "add_socket_group",
            json!({ "label": arg_str(args, "label"), "slot": arg_str(args, "slot") }),
        )?),
        "remove_socket_group" => {
            let index = arg_i64(args, "index")?;
            let skill = arg_str(args, "skill").filter(|s| !s.trim().is_empty());
            if index.is_none() && skill.is_none() {
                return Err(ToolError::Invalid("skill or index is required".into()));
            }
            ctx.call("remove_socket_group", json!({ "index": index, "skill": skill }))?;
            let summary = ctx.call("build_summary", Value::Null)?;
            stats(json!({
                "removed": index.map(Value::from).unwrap_or_else(|| json!(skill)),
                "mainSkillGroup": summary.get("mainSkillGroup").cloned().unwrap_or(Value::Null),
                "mainSkill": summary.get("mainSkill").cloned().unwrap_or(Value::Null),
                "activeSkills": summary.get("activeSkills").cloned().unwrap_or(Value::Null),
                "skills": summary.get("skills").cloned().unwrap_or(Value::Null),
            }))
        }
        "set_socket_group" => stats(ctx.call(
            "set_socket_group",
            json!({
                "index": arg_i64(args, "index")?,
                "skill": arg_str(args, "skill"),
                "enabled": arg_bool(args, "enabled"),
                "label": arg_str(args, "label"),
                "slot": arg_str(args, "slot"),
                "includeInFullDPS": arg_bool(args, "include_in_full_dps"),
                "mainActiveSkill": arg_i64(args, "main_active_skill")?,
            }),
        )?),
        "set_main_skill" => stats(ctx.call("set_main_skill", json!({ "index": arg_i64(args, "group_index")?, "skill": arg_str(args, "skill") }))?),
        "add_gem" => {
            let gem_id = arg_str(args, "gem_id");
            let skill_id = arg_str(args, "skill_id");
            let name_spec = arg_str(args, "name_spec");
            if gem_id.is_none() && skill_id.is_none() && name_spec.is_none() {
                return Err(ToolError::Invalid("gem_id, skill_id, or name_spec is required".into()));
            }
            stats(ctx.call(
                "add_gem",
                json!({
                    "groupIndex": req_i64(args, "group_index")?,
                    "gemId": gem_id,
                    "skillId": skill_id,
                    "nameSpec": name_spec,
                    "level": arg_i64(args, "level")?,
                    "quality": arg_i64(args, "quality")?,
                }),
            )?)
        }
        "set_gem" => stats(ctx.call(
            "set_gem",
            json!({
                "groupIndex": req_i64(args, "group_index")?,
                "gemIndex": req_i64(args, "gem_index")?,
                "level": arg_i64(args, "level")?,
                "quality": arg_i64(args, "quality")?,
                "enabled": arg_bool(args, "enabled"),
                "gemId": arg_str(args, "gem_id"),
            }),
        )?),
        "remove_gem" => stats(ctx.call(
            "remove_gem",
            json!({ "groupIndex": req_i64(args, "group_index")?, "gemIndex": req_i64(args, "gem_index")? }),
        )?),
        "list_gems" => read(ctx.call(
            "list_gems",
            json!({
                "query": arg_str(args, "query"),
                "onlySupports": arg_bool(args, "only_supports"),
                "maxLevel": arg_i64(args, "max_level")?,
                "limit": arg_i64(args, "limit")?.unwrap_or(50),
            }),
        )?),
        "list_valid_supports" => read(ctx.call(
            "list_valid_supports",
            json!({
                "groupIndex": arg_i64(args, "group_index")?,
                "skill": arg_str(args, "skill"),
                "sortByDps": arg_bool(args, "sort_by_dps"),
                "limit": arg_i64(args, "limit")?,
            }),
        )?),
        // Config
        "list_config_options" => read(ctx.call("list_config_options", Value::Null)?),
        "get_config" => read(ctx.call("get_config", Value::Null)?),
        "set_config" => stats(ctx.call(
            "set_config",
            json!({ "var": req_str(args, "var")?, "value": args.get("value").cloned().unwrap_or(Value::Null) }),
        )?),
        // Notes and loadouts
        "get_notes" => read(ctx.call("get_notes", Value::Null)?),
        "set_notes" => Ok((ctx.call("set_notes", json!({ "text": arg_str(args, "text").unwrap_or_default() }))?, true)),
        "get_loadouts" => read(ctx.call("get_loadouts", Value::Null)?),
        "select_loadout" => stats(ctx.call("select_loadout", json!({ "name": req_str(args, "name")? }))?),
        other => Err(ToolError::Invalid(format!("unknown tool {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::{defs, instructions, POE1_TEXT};
    use crate::game::Game;

    #[test]
    fn poe1_registry_speaks_poe1() {
        let d = defs(Game::Poe1);
        assert_eq!(d.len(), 74, "PoE1 tool count changed");
        for (name, _) in POE1_TEXT {
            assert!(d.iter().any(|t| t.name == *name), "POE1_TEXT names {name}, which is not a tool");
        }
        let poe2_only = ["spirit", "charm", "rune", "jeweller", "weapon set", "poe2db", ".build", "tier ladder", "attribute node"];
        let mut found = Vec::new();
        for (name, text) in d.iter().map(|t| (t.name, format!("{} {}", t.description, t.schema))).chain([("instructions", instructions(Game::Poe1).to_string())]) {
            let text = text.to_lowercase();
            found.extend(poe2_only.iter().filter(|w| text.contains(*w)).map(|w| format!("{name}: {w}")));
        }
        assert!(found.is_empty(), "PoE2 terms in PoE1 text: {found:?}");
    }

    /// The chat panel sends a curated subset of these on every turn and the user
    /// pays for the tokens, so the serialized size is a number worth watching.
    #[test]
    fn registry_is_stable_and_measured() {
        let d = defs(Game::Poe2);
        assert_eq!(d.len(), 76, "tool count changed");

        let mut names: Vec<&str> = d.iter().map(|t| t.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate tool name");

        for t in &d {
            assert!(!t.description.is_empty(), "{} has no description", t.name);
            assert_eq!(t.schema["type"], "object", "{} schema is not an object", t.name);
            if t.destructive {
                assert!(!t.read_only, "{} is both read_only and destructive", t.name);
            }
        }

        // What an MCP client pays for `tools/list`. The chat panel no longer
        // pays it: since progressive disclosure it sends the core set and loads
        // the rest through find_tools, so `bun run check:tools` is what watches
        // the prompt. This is a tripwire for runaway growth, not a budget.
        let bytes = serde_json::to_string(&d).unwrap().len();
        println!("tool registry: {} tools, {} bytes serialized (~{} tokens)", d.len(), bytes, bytes / 4);
        assert!(bytes < 60_000, "registry grew to {bytes} bytes; check what was added");
    }
}

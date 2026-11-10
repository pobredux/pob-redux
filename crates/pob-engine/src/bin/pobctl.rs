//! Headless driver for the PoB engine. Useful for smoke tests, benchmarks and
//! poking at bridge methods without the desktop app.
//!
//!   pobctl --pob-root <dir> methods
//!   pobctl --pob-root <dir> call get_build
//!   pobctl --pob-root <dir> call load_build_code '{"code":"..."}'
//!   pobctl --pob-root <dir> stats path/to/build.xml
//!   pobctl --pob-root <dir> bench

use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand};
use pob_engine::{Engine, EngineConfig};
use serde_json::{json, Value};

// Every Lua allocation goes through the global allocator (mlua hands LuaJIT a
// Rust-backed allocator); the system heap is slow for that and slow to give
// memory back.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Parser)]
#[command(name = "pobctl", about = "Drive the headless Path of Building engine")]
struct Cli {
    /// Directory containing PoB's Launch.lua (a pob-sync output or a PoB2 checkout's src/).
    #[arg(long, env = "POB_REDUX_POB_ROOT")]
    pob_root: PathBuf,
    /// Parent of "Path of Building (PoE2)/" (Builds, Settings.xml). Defaults to a temp dir.
    #[arg(long, env = "POB_REDUX_USER_DIR")]
    user_dir: Option<PathBuf>,
    /// Keep compiled Lua here between runs, as the app does. Off by default.
    #[arg(long, env = "POB_REDUX_CACHE_DIR")]
    cache_dir: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List bridge methods.
    Methods,
    /// Call a bridge method with optional JSON params.
    Call { method: String, params: Option<String> },
    /// Load a build XML file and print its sidebar stats.
    Stats { file: PathBuf },
    /// Boot and time a few full recalculations.
    Bench {
        #[arg(default_value_t = 10)]
        iterations: u32,
    },
    /// Evaluate a raw Lua expression against the live state.
    Eval { code: String },
    /// Node power for a build: PoB's sequential PowerBuilder vs the worker pool.
    Power {
        file: PathBuf,
        /// Power stat (e.g. Life, FullDPS); omit for PoB's default.
        #[arg(long)]
        stat: Option<String>,
        /// Max path distance; omit for the whole tree.
        #[arg(long)]
        depth: Option<f64>,
        /// Worker engines.
        #[arg(long, default_value_t = 4)]
        pool: usize,
        /// Skip the sequential reference run.
        #[arg(long)]
        no_sequential: bool,
        /// After the warm run, allocate the best node and rescan this many times,
        /// printing sync/scan time and each worker's Lua heap.
        #[arg(long, default_value_t = 0)]
        rounds: u32,
    },
    /// Gem DPS scoring for a socket group: sequential vs the worker pool.
    Gems {
        file: PathBuf,
        #[arg(long, default_value_t = 1)]
        group: u32,
        #[arg(long, default_value_t = 4)]
        pool: usize,
    },
    /// Point planner: spend a budget on the tree and check it against one calc.
    Plan {
        file: PathBuf,
        #[arg(long, default_value = "Life")]
        stat: String,
        #[arg(long, default_value_t = 10)]
        budget: u32,
        #[arg(long, default_value_t = 4)]
        pool: usize,
    },
    /// Run the tree planner and gear optimiser on a corpus index (the pobredux/corpus repo).
    Corpus {
        index: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Planner budget in passive points.
        #[arg(long, default_value_t = 10)]
        budget: u32,
        /// Stats to plan for, one plan each.
        #[arg(long, value_delimiter = ',', default_value = "Life,CombinedDPS")]
        stats: Vec<String>,
        #[arg(long, default_value_t = 4)]
        pool: usize,
        /// At most this many stages per ascendancy.
        #[arg(long)]
        per_ascendancy: Option<usize>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        no_gear: bool,
        #[arg(long)]
        no_tree: bool,
    },
    /// Reference outputs for every usable stage of a corpus index, optionally compared with a baseline.
    Golden {
        index: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Earlier outputs to compare against; lists every stat that moved.
        #[arg(long)]
        baseline: Option<PathBuf>,
        /// Relative change that counts as a difference.
        #[arg(long, default_value_t = 1e-6)]
        tolerance: f64,
        /// Every number PoB outputs, not the curated list.
        #[arg(long)]
        all: bool,
        /// Engines run in parallel.
        #[arg(long, default_value_t = 4)]
        jobs: usize,
        /// Exit with status 1 when the baseline differs.
        #[arg(long)]
        check: bool,
    },
    /// Unique jewel suggestions: one engine vs the worker pool, with an equality check.
    Jewels {
        file: PathBuf,
        /// balanced, defence or damage.
        #[arg(long)]
        aim: Option<String>,
        #[arg(long, default_value_t = 4)]
        pool: usize,
        /// Skip the one-engine reference run.
        #[arg(long)]
        no_sequential: bool,
    },
}

fn plan_cmd(cfg: EngineConfig, file: PathBuf, stat: String, budget: u32, pool_size: usize) -> Result<Value, pob_engine::Error> {
    use pob_engine::{EngineHandle, EnginePool};
    let engine = EngineHandle::spawn(cfg.clone());
    let pool = EnginePool::new(cfg, pool_size);
    pool.warm();
    engine.wait_ready()?;
    engine.call("load_build_file", json!({ "path": file.to_string_lossy() }))?;
    let plan = pob_engine::pool::plan_points(&engine, &pool, &stat, budget)?;
    let ids: Vec<String> = plan
        .get("picks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|p| p.get("path").and_then(Value::as_array).cloned().unwrap_or_default())
        .filter_map(|v| v.as_f64().map(|f| (f as i64).to_string()))
        .collect();
    let code = format!(
        r#"local build = main.modes["BUILD"]
local calcsTab = build.calcsTab
local ps
for _, s in ipairs(data.powerStatList) do if s.stat == "{stat}" then ps = s end end
local mode = build.viewMode
build.viewMode = "CALCS"
local calcFunc, calcBase = calcsTab:GetMiscCalculator()
local set = {{}}
for _, id in ipairs({{ {ids} }}) do set[build.spec.nodes[id]] = true end
local v = calcsTab:CalculatePowerStat(ps, calcFunc({{ addNodes = set }}), calcBase)
build.viewMode = mode
return {{ combined = v }}"#,
        ids = ids.join(", ")
    );
    let check = engine.eval(&code)?;
    let total = plan.get("total").and_then(Value::as_f64).unwrap_or(0.0);
    let combined = check.get("combined").and_then(Value::as_f64).unwrap_or(f64::NAN);
    eprintln!("plan: {:.0} ms, total {total:.3}, one-calc check {combined:.3}", plan.get("ms").and_then(Value::as_f64).unwrap_or(0.0));
    Ok(json!({ "plan": plan, "combined_check": combined, "difference": combined - total }))
}

const CORPUS_STATS: &[&str] = &[
    "Life", "EnergyShield", "TotalEHP", "CombinedDPS", "FireResist", "ColdResist", "LightningResist", "ChaosResist", "Str", "Dex", "Int",
    "ReqStr", "ReqDex", "ReqInt", "MovementSpeedMod",
];

struct CorpusOpts {
    index: PathBuf,
    out: PathBuf,
    budget: u32,
    stats: Vec<String>,
    pool: usize,
    per_ascendancy: Option<usize>,
    limit: Option<usize>,
    no_gear: bool,
    no_tree: bool,
}

fn corpus_selection(index: &Value, per_ascendancy: Option<usize>) -> Vec<Value> {
    let usable: Vec<&Value> = index["stages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["duplicateOf"].is_null() && s["nearDuplicateOf"].is_null() && s["flags"].as_array().is_some_and(|f| f.is_empty()))
        .collect();
    let Some(cap) = per_ascendancy else { return usable.into_iter().cloned().collect() };
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, [Vec<&Value>; 3]> = std::collections::HashMap::new();
    for s in usable {
        let asc = s["ascendancy"].as_str().unwrap_or("none").to_string();
        if !groups.contains_key(&asc) {
            order.push(asc.clone());
        }
        let slot = match s["kind"].as_str() {
            Some("ladder") => 0,
            Some("guide") => 1,
            _ => 2,
        };
        groups.entry(asc).or_default()[slot].push(s);
    }
    let mut out = Vec::new();
    for asc in order {
        let mut buckets = groups.remove(&asc).unwrap_or_default().map(|b| b.into_iter());
        let mut taken = 0;
        while taken < cap {
            let mut any = false;
            for b in buckets.iter_mut() {
                if taken < cap {
                    if let Some(s) = b.next() {
                        out.push(s.clone());
                        taken += 1;
                        any = true;
                    }
                }
            }
            if !any {
                break;
            }
        }
    }
    out
}

/// A corpus stage as ingest measured it: its loadout, and its main skill.
fn load_stage(engine: &pob_engine::EngineHandle, stage: &Value, xml_dir: &std::path::Path) -> Result<(), pob_engine::Error> {
    let xml = xml_dir.join(format!("{}.xml", stage["source"].as_str().unwrap_or_default()));
    engine.call("load_build_file", json!({ "path": xml.to_string_lossy() }))?;
    if let Some(name) = stage["loadout"].as_str() {
        engine.call("select_loadout", json!({ "name": name }))?;
    }
    // Ingest replaced a 0-DPS main skill; group numbers shift between PoB versions, names do not.
    if stage["mainSkillFixed"].as_bool() == Some(true) {
        let by_name = stage["mainSkill"].as_str().map(|s| engine.call("set_main_skill", json!({ "skill": s })));
        if !matches!(by_name, Some(Ok(_))) {
            if let Some(group) = stage["mainSocketGroup"].as_u64() {
                engine.call("set_main_skill", json!({ "index": group }))?;
            }
        }
    }
    Ok(())
}

fn read_json(path: &std::path::Path) -> Result<Value, pob_engine::Error> {
    let text = std::fs::read_to_string(path).map_err(|e| pob_engine::Error::Other(format!("{}: {e}", path.display())))?;
    serde_json::from_str(&text).map_err(|e| pob_engine::Error::Other(format!("{}: {e}", path.display())))
}

fn corpus_cmd(cfg: EngineConfig, o: CorpusOpts) -> Result<Value, pob_engine::Error> {
    use pob_engine::{EngineHandle, EnginePool, Error};
    use std::io::Write;
    let text = std::fs::read_to_string(&o.index).map_err(|e| Error::Other(format!("{}: {e}", o.index.display())))?;
    let index: Value = serde_json::from_str(&text).map_err(|e| Error::Other(format!("{}: {e}", o.index.display())))?;
    // PoB resolves a relative build path against its own folders, so hand it absolute ones.
    let index_path = std::path::absolute(&o.index).map_err(|e| Error::Other(format!("{}: {e}", o.index.display())))?;
    let xml_dir = index_path.parent().map(|p| p.join("xml")).unwrap_or_else(|| PathBuf::from("xml"));
    let done: std::collections::HashSet<String> = std::fs::read_to_string(&o.out)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| v["id"].as_str().map(str::to_string))
        .collect();
    let mut stages = corpus_selection(&index, o.per_ascendancy);
    stages.retain(|s| s["id"].as_str().is_some_and(|id| !done.contains(id)));
    if let Some(n) = o.limit {
        stages.truncate(n);
    }
    let engine = EngineHandle::spawn(cfg.clone());
    let pool = EnginePool::new(cfg, o.pool);
    pool.warm();
    engine.wait_ready()?;
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&o.out)
        .map_err(|e| Error::Other(format!("{}: {e}", o.out.display())))?;
    let started = Instant::now();
    let (mut ok, mut failed) = (0usize, 0usize);
    for (n, stage) in stages.iter().enumerate() {
        let t = Instant::now();
        let id = stage["id"].as_str().unwrap_or_default();
        let mut line = json!({
            "id": id,
            "ascendancy": stage["ascendancy"],
            "kind": stage["kind"],
            "stage": stage["stage"],
            "gear": stage["gear"],
            "level": stage["level"],
            "mainSkill": stage["mainSkill"],
        });
        let mut run = || -> Result<(), Error> {
            load_stage(&engine, stage, &xml_dir)?;
            line["before"] = engine.call("get_stats", json!({ "fields": CORPUS_STATS }))?.result["stats"].clone();
            let sanity = engine.call("sanity_check", Value::Null)?.result;
            line["findings"] = json!(sanity["findings"].as_array().into_iter().flatten().map(|f| f["area"].clone()).collect::<Vec<_>>());
            if !o.no_tree {
                let mut plans = serde_json::Map::new();
                for stat in &o.stats {
                    let v = match pob_engine::pool::plan_points(&engine, &pool, stat, o.budget) {
                        Ok(p) => json!({
                            "total": p["total"],
                            "spent": p["spent"],
                            "picks": p["picks"].as_array().map_or(0, Vec::len),
                            "ms": p["ms"],
                        }),
                        Err(e) => json!({ "error": e.to_string() }),
                    };
                    plans.insert(stat.clone(), v);
                }
                line["plans"] = Value::Object(plans);
            }
            if !o.no_gear && stage["gear"].as_str() == Some("full") {
                let g = engine.call("optimise_gear", json!({ "preset": "balanced" }))?.result;
                line["gearOpt"] = json!({
                    "ms": g["ms"],
                    "before": g["before"],
                    "after": g["after"],
                    "proposals": g["proposals"].as_array().map_or(0, Vec::len),
                    "skipped": g["skipped"].as_array().map_or(0, Vec::len),
                });
            }
            Ok(())
        };
        match run() {
            Ok(()) => ok += 1,
            Err(e) => {
                failed += 1;
                line["error"] = json!(e.to_string());
            }
        }
        line["ms"] = json!(t.elapsed().as_millis() as u64);
        writeln!(out, "{line}").map_err(|e| Error::Other(e.to_string()))?;
        out.flush().ok();
        eprintln!(
            "[{}/{}] {} {} {:.1}s",
            n + 1,
            stages.len(),
            stage["ascendancy"].as_str().unwrap_or("?"),
            stage["name"].as_str().unwrap_or(id),
            t.elapsed().as_secs_f64()
        );
    }
    Ok(json!({ "stages": stages.len(), "ok": ok, "failed": failed, "minutes": started.elapsed().as_secs_f64() / 60.0 }))
}

const GOLDEN_STATS: &[&str] = &[
    "Life", "Mana", "EnergyShield", "Spirit", "Ward", "Armour", "Evasion", "TotalEHP", "PhysicalMaximumHitTaken",
    "FireMaximumHitTaken", "ColdMaximumHitTaken", "LightningMaximumHitTaken", "ChaosMaximumHitTaken", "FireResist",
    "ColdResist", "LightningResist", "ChaosResist", "BlockChance", "SpellBlockChance", "SpellSuppressionChance",
    "LifeRegenRecovery", "ManaRegenRecovery", "EnergyShieldRegenRecovery", "LifeUnreserved", "ManaUnreserved", "Str",
    "Dex", "Int", "ReqStr", "ReqDex", "ReqInt", "MovementSpeedMod", "EffectiveMovementSpeedMod", "AverageHit",
    "AverageDamage", "Speed", "HitChance", "CritChance", "CritMultiplier", "TotalDPS", "CombinedDPS", "FullDPS",
    "TotalDotDPS", "BleedDPS", "IgniteDPS", "PoisonDPS", "ImpaleDPS", "ManaCost", "LifeCost",
];
const GOLDEN_MINION_STATS: &[&str] = &["Life", "EnergyShield", "TotalDPS", "CombinedDPS", "AverageHit", "Speed"];

/// Lua returning the build's main skill and its output numbers: the listed stats, or every one with `all`.
fn golden_lua(all: bool) -> String {
    let list = |keys: &[&str]| {
        if all {
            "nil".to_string()
        } else {
            format!("{{ {} }}", keys.iter().map(|k| format!("{k:?}")).collect::<Vec<_>>().join(", "))
        }
    };
    format!(
        r#"local build = main.modes["BUILD"]
local out = build.calcsTab.mainOutput
local stats = {{}}
local function put(prefix, t, keys)
	local function one(k, v)
		if type(k) == "string" and type(v) == "number" and v == v and v ~= math.huge and v ~= -math.huge then stats[prefix .. k] = v end
	end
	if keys then for _, k in ipairs(keys) do one(k, t[k]) end else for k, v in pairs(t) do one(k, v) end end
end
put("", out, {main})
if type(out.Minion) == "table" then put("Minion.", out.Minion, {minion}) end
local group = build.skillsTab.socketGroupList[build.mainSocketGroup]
return {{ skill = group and group.displayLabel or false, stats = stats }}"#,
        main = list(GOLDEN_STATS),
        minion = list(GOLDEN_MINION_STATS)
    )
}

/// The corpus commit a run used, marked when the working tree differs from it.
fn corpus_commit(dir: &std::path::Path) -> Value {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    match git(&["rev-parse", "HEAD"]) {
        Some(sha) if git(&["status", "--porcelain"]).is_some_and(|s| !s.is_empty()) => json!(format!("{sha}-dirty")),
        Some(sha) => json!(sha),
        None => Value::Null,
    }
}

struct GoldenOpts {
    index: PathBuf,
    out: PathBuf,
    baseline: Option<PathBuf>,
    tolerance: f64,
    all: bool,
    jobs: usize,
}

fn golden_cmd(cfg: EngineConfig, o: GoldenOpts) -> Result<Value, pob_engine::Error> {
    use pob_engine::{EngineHandle, Error};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let index = read_json(&o.index)?;
    let index_path = std::path::absolute(&o.index).map_err(|e| Error::Other(format!("{}: {e}", o.index.display())))?;
    let corpus_dir = index_path.parent().map(PathBuf::from).unwrap_or_default();
    let xml_dir = corpus_dir.join("xml");
    let corpus = corpus_commit(&corpus_dir);
    let stages = corpus_selection(&index, None);
    let code = golden_lua(o.all);
    let started = Instant::now();
    let next = AtomicUsize::new(0);
    let builds = std::sync::Mutex::new(std::collections::BTreeMap::<String, Value>::new());
    std::thread::scope(|s| {
        for _ in 0..o.jobs.clamp(1, 16).min(stages.len().max(1)) {
            s.spawn(|| {
                let engine = EngineHandle::spawn(cfg.clone());
                if engine.wait_ready().is_err() {
                    return;
                }
                loop {
                    let n = next.fetch_add(1, Ordering::Relaxed);
                    let Some(stage) = stages.get(n) else { break };
                    let mut entry = json!({ "ascendancy": stage["ascendancy"] });
                    match load_stage(&engine, stage, &xml_dir).and_then(|_| engine.eval(&code)) {
                        Ok(v) => {
                            entry["skill"] = v["skill"].clone();
                            entry["stats"] = if v["stats"].is_object() { v["stats"].clone() } else { json!({}) };
                        }
                        Err(e) => entry["error"] = json!(e.to_string()),
                    }
                    builds.lock().unwrap().insert(stage["id"].as_str().unwrap_or_default().to_string(), entry);
                    if (n + 1).is_multiple_of(50) {
                        eprintln!("[{}/{}] {:.0}s", n + 1, stages.len(), started.elapsed().as_secs_f64());
                    }
                }
            });
        }
    });
    let builds = builds.into_inner().unwrap();
    let failed = builds.values().filter(|b| b.get("error").is_some()).count();
    let sync = std::fs::read_to_string(cfg.pob_root.join("SYNC.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let pob = sync.map_or(Value::Null, |s| {
        json!({ "game": s["game"], "version": s["upstream_version"], "commit": s["upstream_commit"], "patches": s["patches"] })
    });
    // One build per line keeps a regenerated file reviewable as a diff.
    let mut text = format!("{{\n\"pob\": {pob},\n\"corpus\": {corpus},\n\"stats\": {},\n\"builds\": {{\n", json!(if o.all { "all" } else { "curated" }));
    for (i, (id, b)) in builds.iter().enumerate() {
        text.push_str(&format!("{}: {b}{}\n", json!(id), if i + 1 < builds.len() { "," } else { "" }));
    }
    text.push_str("}\n}\n");
    std::fs::write(&o.out, text).map_err(|e| Error::Other(format!("{}: {e}", o.out.display())))?;
    let mut summary = json!({
        "builds": builds.len(),
        "failed": failed,
        "seconds": started.elapsed().as_secs_f64().round(),
        "corpus": corpus,
        "out": o.out.display().to_string(),
    });
    if corpus.as_str().is_some_and(|c| c.ends_with("-dirty")) {
        summary["warning"] = json!("the corpus has uncommitted changes, so CI cannot check out the commit this run used");
    }
    if let Some(path) = &o.baseline {
        let baseline = read_json(path)?;
        let mut diff = golden_diff(&baseline, &json!({ "builds": builds }), o.tolerance);
        if baseline["corpus"] != corpus {
            diff["corpusChanged"] = json!({ "baseline": baseline["corpus"], "now": corpus });
        }
        summary["passed"] = json!(diff["changedValues"] == 0 && diff["missing"].as_array().is_some_and(Vec::is_empty) && failed == 0);
        summary["diff"] = diff;
    }
    Ok(summary)
}

/// Every stat that moved by more than `tolerance` (relative, floored at 1) between two golden runs.
fn golden_diff(base: &Value, new: &Value, tolerance: f64) -> Value {
    let empty = serde_json::Map::new();
    let a = base["builds"].as_object().unwrap_or(&empty);
    let b = new["builds"].as_object().unwrap_or(&empty);
    let mut per_stat: std::collections::BTreeMap<String, (usize, f64)> = std::collections::BTreeMap::new();
    let mut rows: Vec<(f64, Value)> = Vec::new();
    let mut changed_builds = 0;
    let mut skill_changes = Vec::new();
    for (id, before) in a {
        let Some(after) = b.get(id) else { continue };
        let (Some(sa), Some(sb)) = (before["stats"].as_object(), after["stats"].as_object()) else { continue };
        if before["skill"] != after["skill"] {
            skill_changes.push(json!({ "id": id, "before": before["skill"], "after": after["skill"] }));
        }
        let keys: std::collections::BTreeSet<&String> = sa.keys().chain(sb.keys()).collect();
        let mut any = false;
        for k in keys {
            let (x, y) = (sa.get(k).and_then(Value::as_f64), sb.get(k).and_then(Value::as_f64));
            let change = match (x, y) {
                (Some(x), Some(y)) if (x - y).abs() <= tolerance * x.abs().max(y.abs()).max(1.0) => continue,
                (Some(x), Some(y)) => (y - x).abs() / x.abs().max(1e-9),
                _ => f64::INFINITY,
            };
            any = true;
            let e = per_stat.entry(k.clone()).or_insert((0, 0.0));
            e.0 += 1;
            e.1 = e.1.max(change);
            rows.push((change, json!({ "id": id, "ascendancy": after["ascendancy"], "skill": after["skill"], "stat": k, "before": x, "after": y })));
        }
        if any {
            changed_builds += 1;
        }
    }
    rows.sort_by(|p, q| q.0.total_cmp(&p.0));
    let percent = |c: f64| if c.is_finite() { json!((c * 1000.0).round() / 10.0) } else { Value::Null };
    json!({
        "compared": a.keys().filter(|id| b.contains_key(*id)).count(),
        "changedBuilds": changed_builds,
        "changedValues": rows.len(),
        "perStat": per_stat.into_iter().map(|(k, (n, max))| (k, json!({ "builds": n, "maxChangePercent": percent(max) }))).collect::<serde_json::Map<_, _>>(),
        "largest": rows.iter().take(25).map(|(c, r)| { let mut r = r.clone(); r["changePercent"] = percent(*c); r }).collect::<Vec<_>>(),
        "mainSkillChanged": skill_changes,
        "missing": a.keys().filter(|id| !b.contains_key(*id)).collect::<Vec<_>>(),
        "added": b.keys().filter(|id| !a.contains_key(*id)).collect::<Vec<_>>(),
        "errors": b.iter().filter_map(|(id, v)| v.get("error").map(|e| json!({ "id": id, "error": e }))).collect::<Vec<_>>(),
    })
}

fn jewels_cmd(cfg: EngineConfig, file: PathBuf, aim: Option<String>, pool_size: usize, no_sequential: bool) -> Result<Value, pob_engine::Error> {
    use pob_engine::{EngineHandle, EnginePool};
    let engine = EngineHandle::spawn(cfg.clone());
    let pool = EnginePool::new(cfg, pool_size);
    pool.warm();
    engine.wait_ready()?;
    engine.call("load_build_file", json!({ "path": file.to_string_lossy() }))?;
    let params = json!({ "preset": aim });
    let mut seq = Value::Null;
    let mut seq_ms = 0.0;
    if !no_sequential {
        let t = Instant::now();
        seq = engine.call("suggest_unique_jewels", params.clone())?.result;
        seq_ms = t.elapsed().as_secs_f64() * 1000.0;
        eprintln!("one engine: {seq_ms:.0} ms, {} evaluations", seq.get("evaluations").and_then(Value::as_u64).unwrap_or(0));
    }
    let t = Instant::now();
    let par = pob_engine::pool::jewel_scan(&engine, &pool, params.clone())?;
    let cold_ms = t.elapsed().as_secs_f64() * 1000.0;
    let t = Instant::now();
    let par = if no_sequential { par } else { pob_engine::pool::jewel_scan(&engine, &pool, params)? };
    let warm_ms = t.elapsed().as_secs_f64() * 1000.0;
    eprintln!("pool x{}: {cold_ms:.0} ms cold (with sync), {warm_ms:.0} ms warm", pool.status().ready);
    let rows = |v: &Value| {
        v.get("suggestions")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|r| json!([r.get("name"), r.get("socket"), r.get("variants"), r.get("score")]))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let seq_rows = rows(&seq);
    let par_rows = rows(&par);
    let mismatches = if no_sequential {
        Value::Null
    } else {
        json!(seq_rows.iter().zip(par_rows.iter()).filter(|(a, b)| a != b).count() + seq_rows.len().abs_diff(par_rows.len()))
    };
    Ok(json!({
        "sequential_ms": if no_sequential { Value::Null } else { json!(seq_ms) },
        "pool_cold_ms": cold_ms,
        "pool_warm_ms": if no_sequential { Value::Null } else { json!(warm_ms) },
        "workers": pool.status().ready,
        "evaluations": par.get("evaluations").cloned().unwrap_or(Value::Null),
        "mismatches": mismatches,
        "mismatch_samples": if no_sequential { Vec::new() } else { seq_rows.iter().zip(par_rows.iter()).enumerate().filter(|(_, (a, b))| a != b).take(8).map(|(i, (a, b))| json!({ "row": i, "sequential": a, "pool": b })).collect::<Vec<_>>() },
        "rows": json!([seq_rows.len(), par_rows.len()]),
        "summary": par.get("summary").cloned().unwrap_or(Value::Null),
        "top": par_rows.iter().take(8).collect::<Vec<_>>(),
        "notScored": par.get("notScored").and_then(Value::as_array).map(|a| a.iter().map(|r| r.get("name").cloned().unwrap_or(Value::Null)).collect::<Vec<_>>()).unwrap_or_default(),
    }))
}

fn gems_cmd(cfg: EngineConfig, file: PathBuf, group: u32, pool_size: usize) -> Result<Value, pob_engine::Error> {
    use pob_engine::{EngineHandle, EnginePool};
    let engine = EngineHandle::spawn(cfg.clone());
    let pool = EnginePool::new(cfg, pool_size);
    pool.warm();
    engine.wait_ready()?;
    engine.call("load_build_file", json!({ "path": file.to_string_lossy() }))?;
    // sequential: a cold DPS-sorted search scores every candidate in this engine
    let t = Instant::now();
    let seq = engine
        .call("gem_search", json!({ "groupIndex": group, "query": "", "sortByDps": true, "limit": 400 }))?
        .result;
    let seq_ms = t.elapsed().as_secs_f64() * 1000.0;
    // invalidate the cache by touching the build, then fill it from the pool:
    // once cold (includes the worker sync) and once with workers pre-synced,
    // which is the app's steady state
    engine.call("set_level", json!({ "level": 32 }))?;
    let t = Instant::now();
    let _ = pob_engine::pool::gem_dps_fill(&engine, &pool, group)?;
    let fill_cold_ms = t.elapsed().as_secs_f64() * 1000.0;
    engine.call("set_level", json!({ "level": 33 }))?;
    pob_engine::pool::sync_from(&engine, &pool)?;
    let t = Instant::now();
    let fill = pob_engine::pool::gem_dps_fill(&engine, &pool, group)?;
    let fill_ms = t.elapsed().as_secs_f64() * 1000.0;
    eprintln!("pool fill cold (with sync): {fill_cold_ms:.0} ms; pre-synced: {fill_ms:.0} ms");
    let t = Instant::now();
    let par = engine
        .call("gem_search", json!({ "groupIndex": group, "query": "", "sortByDps": true, "limit": 400 }))?
        .result;
    let search_ms = t.elapsed().as_secs_f64() * 1000.0;
    let top = |v: &Value| {
        v.get("gems")
            .and_then(|g| g.as_array())
            .map(|g| g.iter().take(5).map(|r| json!([r.get("name"), r.get("dps")])).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    Ok(json!({
        "sequential_ms": seq_ms,
        "pool_fill_ms": fill_ms,
        "search_after_fill_ms": search_ms,
        "speedup": seq_ms / (fill_ms + search_ms).max(0.001),
        "scored": fill.get("count").cloned().unwrap_or(Value::Null),
        "workers": pool.status().ready,
        "top_sequential": top(&seq),
        "top_pool": top(&par),
    }))
}

/// Per-node power from PoB's sequential builder against the pool's: the count
/// of nodes whose scores differ and a few examples.
fn compare_power(sequential: &Value, parallel: &Value) -> (usize, Vec<Value>) {
    let mut mismatches = 0;
    let mut samples = Vec::new();
    let (Some(a), Some(b)) = (
        sequential.get("nodes").and_then(|n| n.as_object()),
        parallel.get("nodes").and_then(|n| n.as_object()),
    ) else {
        return (0, samples);
    };
    for (id, va) in a {
        match b.get(id) {
            Some(vb) => {
                for k in ["s", "o", "d", "p"] {
                    let x = va.get(k).and_then(|v| v.as_f64());
                    let y = vb.get(k).and_then(|v| v.as_f64());
                    let differs = match (x, y) {
                        (Some(x), Some(y)) => (x - y).abs() > 1e-6,
                        (None, None) => false,
                        _ => true,
                    };
                    if differs {
                        mismatches += 1;
                        if samples.len() < 8 {
                            samples.push(json!({ "id": id, "key": k, "seq": va, "pool": vb }));
                        }
                        break;
                    }
                }
            }
            None => {
                mismatches += 1;
                if samples.len() < 8 {
                    samples.push(json!({ "id": id, "seq": va, "pool": Value::Null }));
                }
            }
        }
    }
    for (id, vb) in b {
        if !a.contains_key(id) {
            mismatches += 1;
            if samples.len() < 8 {
                samples.push(json!({ "id": id, "seq": Value::Null, "pool": vb }));
            }
        }
    }
    (mismatches, samples)
}

fn power_cmd(cfg: EngineConfig, file: PathBuf, stat: Option<String>, depth: Option<f64>, pool_size: usize, no_sequential: bool, rounds: u32) -> Result<Value, pob_engine::Error> {
    use pob_engine::{EngineHandle, EnginePool};
    let t0 = Instant::now();
    let engine = EngineHandle::spawn(cfg.clone());
    let pool = EnginePool::new(cfg, pool_size);
    pool.warm();
    engine.wait_ready()?;
    eprintln!("main engine ready in {} ms", t0.elapsed().as_millis());
    engine.call("load_build_file", json!({ "path": file.to_string_lossy() }))?;

    let mut sequential = Value::Null;
    let mut seq_ms = 0.0;
    if !no_sequential {
        let t = Instant::now();
        sequential = engine.call("tree_power", json!({ "stat": stat, "maxDepth": depth }))?.result;
        seq_ms = t.elapsed().as_secs_f64() * 1000.0;
        eprintln!("sequential PowerBuilder: {seq_ms:.0} ms");
    }

    let t = Instant::now();
    let parallel = pob_engine::pool::power_scan(&engine, &pool, stat.as_deref(), depth)?;
    let par_ms = t.elapsed().as_secs_f64() * 1000.0;
    eprintln!("pool x{}: {par_ms:.0} ms (first run includes sync)", pool.status().ready);
    let t = Instant::now();
    let _ = pob_engine::pool::power_scan(&engine, &pool, stat.as_deref(), depth)?;
    let par2_ms = t.elapsed().as_secs_f64() * 1000.0;
    eprintln!("pool x{}: {par2_ms:.0} ms (warm)", pool.status().ready);

    let count = |v: &Value| v.get("nodes").and_then(|n| n.as_object()).map(|m| m.len()).unwrap_or(0);
    let (mut mismatches, mut samples) = compare_power(&sequential, &parallel);

    // Each round allocates the best node and rescans, which takes the tree-swap
    // sync path; the sequential builder checks it every round unless skipped.
    let heap = "collectgarbage('collect'); return math.floor(collectgarbage('count') / 1024)";
    for round in 1..=rounds {
        let best = engine
            .call("tree_power_result", Value::Null)?
            .result
            .get("report")
            .and_then(|r| r.as_array())
            .and_then(|list| list.iter().find(|r| r.get("allocated") != Some(&Value::Bool(true))))
            .and_then(|r| r.get("id").cloned());
        if let Some(id) = best {
            engine.call("alloc_node", json!({ "id": id }))?;
        }
        let t = Instant::now();
        let scanned = pob_engine::pool::power_scan(&engine, &pool, stat.as_deref(), depth)?;
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let mut check = String::new();
        if !no_sequential {
            let seq = engine.call("tree_power", json!({ "stat": stat, "maxDepth": depth }))?.result;
            let (m, s) = compare_power(&seq, &scanned);
            mismatches += m;
            if samples.len() < 8 {
                samples.extend(s);
            }
            check = format!("; mismatches {m}");
        }
        let heaps: Vec<String> = pool.eval_all(heap)?.iter().map(|v| v.to_string()).collect();
        let main_mb = engine.eval(heap)?;
        eprintln!("round {round}: scan {ms:.0} ms{check}; main heap {main_mb} MB; worker heaps MB [{}]", heaps.join(" "));
    }
    Ok(json!({
        "nodes": count(&parallel),
        "sequential_ms": if no_sequential { Value::Null } else { json!(seq_ms) },
        "pool_first_ms": par_ms,
        "pool_warm_ms": par2_ms,
        "speedup_warm": if no_sequential || par2_ms == 0.0 { Value::Null } else { json!(seq_ms / par2_ms) },
        "workers": pool.status().ready,
        "mismatches": if no_sequential { Value::Null } else { json!(mismatches) },
        "mismatch_samples": samples,
        "max": parallel.get("max").cloned().unwrap_or(Value::Null),
        "cpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
    }))
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    let user_dir = cli
        .user_dir
        .unwrap_or_else(|| std::env::temp_dir().join("pob-redux-cli"));

    let pooled: Option<Result<Value, pob_engine::Error>> = match &cli.cmd {
        Cmd::Power { file, stat, depth, pool, no_sequential, rounds } => {
            let cfg = EngineConfig { pob_root: cli.pob_root.clone(), user_dir: user_dir.clone(), cache_dir: cli.cache_dir.clone() };
            Some(power_cmd(cfg, file.clone(), stat.clone(), *depth, *pool, *no_sequential, *rounds))
        }
        Cmd::Gems { file, group, pool } => {
            let cfg = EngineConfig { pob_root: cli.pob_root.clone(), user_dir: user_dir.clone(), cache_dir: cli.cache_dir.clone() };
            Some(gems_cmd(cfg, file.clone(), *group, *pool))
        }
        Cmd::Jewels { file, aim, pool, no_sequential } => {
            let cfg = EngineConfig { pob_root: cli.pob_root.clone(), user_dir: user_dir.clone(), cache_dir: cli.cache_dir.clone() };
            Some(jewels_cmd(cfg, file.clone(), aim.clone(), *pool, *no_sequential))
        }
        Cmd::Plan { file, stat, budget, pool } => {
            let cfg = EngineConfig { pob_root: cli.pob_root.clone(), user_dir: user_dir.clone(), cache_dir: cli.cache_dir.clone() };
            Some(plan_cmd(cfg, file.clone(), stat.clone(), *budget, *pool))
        }
        Cmd::Corpus { index, out, budget, stats, pool, per_ascendancy, limit, no_gear, no_tree } => {
            let cfg = EngineConfig { pob_root: cli.pob_root.clone(), user_dir: user_dir.clone(), cache_dir: cli.cache_dir.clone() };
            let opts = CorpusOpts {
                index: index.clone(),
                out: out.clone(),
                budget: *budget,
                stats: stats.clone(),
                pool: *pool,
                per_ascendancy: *per_ascendancy,
                limit: *limit,
                no_gear: *no_gear,
                no_tree: *no_tree,
            };
            Some(corpus_cmd(cfg, opts))
        }
        Cmd::Golden { index, out, baseline, tolerance, all, jobs, .. } => {
            let cfg = EngineConfig { pob_root: cli.pob_root.clone(), user_dir: user_dir.clone(), cache_dir: cli.cache_dir.clone() };
            let opts = GoldenOpts { index: index.clone(), out: out.clone(), baseline: baseline.clone(), tolerance: *tolerance, all: *all, jobs: *jobs };
            Some(golden_cmd(cfg, opts))
        }
        _ => None,
    };
    if let Some(out) = pooled {
        match out {
            Ok(v) => {
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
                if matches!(cli.cmd, Cmd::Golden { check: true, .. }) && v["passed"] == false {
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(2);
            }
        }
        return;
    }

    let t0 = Instant::now();
    let engine = match Engine::boot(EngineConfig { pob_root: cli.pob_root, user_dir, cache_dir: cli.cache_dir }) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("boot failed: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("booted in {} ms", t0.elapsed().as_millis());

    let out: Result<Value, pob_engine::Error> = match cli.cmd {
        Cmd::Methods => engine.method_names().map(|m| json!(m)),
        Cmd::Call { method, params } => {
            let params: Value = match params {
                Some(p) => serde_json::from_str(&p).expect("params must be JSON"),
                None => Value::Null,
            };
            let t = Instant::now();
            let r = engine.call(&method, &params);
            eprintln!("{method}: {:.2} ms", t.elapsed().as_secs_f64() * 1000.0);
            r
        }
        Cmd::Stats { file } => {
            let xml = std::fs::read_to_string(&file).expect("read build file");
            let name = file.file_stem().map(|s| s.to_string_lossy().to_string());
            let t = Instant::now();
            engine
                .call("load_build_xml", &json!({ "xml": xml, "name": name }))
                .and_then(|_| {
                    eprintln!("load+calc: {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
                    engine.call("get_sidebar", &Value::Null)
                })
                .inspect(|side| {
                    if let Some(rows) = side.get("rows").and_then(|r| r.as_array()) {
                        for row in rows {
                            let lhs = row.get("lhs").and_then(|v| v.as_str()).unwrap_or("");
                            let rhs = row.get("rhs").and_then(|v| v.as_str()).unwrap_or("");
                            if !lhs.is_empty() || !rhs.is_empty() {
                                println!("{:<40} {}", strip(lhs), strip(rhs));
                            }
                        }
                    }
                })
        }
        Cmd::Bench { iterations } => engine
            .call("new_build", &json!({ "name": "bench" }))
            .and_then(|_| {
                let mut times = Vec::new();
                for _ in 0..iterations {
                    let t = Instant::now();
                    engine.call("refresh", &Value::Null)?;
                    times.push(t.elapsed().as_secs_f64() * 1000.0);
                }
                let avg = times.iter().sum::<f64>() / times.len().max(1) as f64;
                let min = times.iter().cloned().fold(f64::MAX, f64::min);
                Ok(json!({ "iterations": iterations, "avg_ms": avg, "min_ms": min, "boot_ms": engine.boot_ms }))
            }),
        Cmd::Eval { code } => engine.eval(&code),
        Cmd::Power { .. } | Cmd::Gems { .. } | Cmd::Jewels { .. } | Cmd::Plan { .. } | Cmd::Corpus { .. } | Cmd::Golden { .. } => {
            unreachable!("handled above")
        }
    };

    match out {
        Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    }
}

/// Drop PoB colour escapes (^7, ^xRRGGBB) for terminal output.
fn strip(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'^' && i + 1 < b.len() {
            if b[i + 1] == b'x' && i + 7 < b.len() {
                i += 8;
                continue;
            }
            if b[i + 1].is_ascii_digit() {
                i += 2;
                continue;
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

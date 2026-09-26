use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

mod art;
mod mcp;
mod tools;
mod library;
mod ai;
mod agent;
mod decide;
mod sites;
mod mobalytics;
mod maxroll;
mod character;
mod ninja;
mod links;
mod diagnostics;
mod game;

use std::sync::Arc;

// Pulls mimalloc's static library into this target so `mi_collect` resolves.
use libmimalloc_sys as _;
use pob_engine::{EngineConfig, EngineHandle, EnginePool, EngineStatus, PoolStatus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{Emitter, Manager, State};

use game::Game;

/// The engine and pool booted on one game's PoB. Replaced whole when the
/// user switches game, so every clone taken before the switch keeps driving
/// the engine it was taken from until it is dropped.
pub(crate) struct Runtime {
    pub(crate) game: Game,
    pub(crate) engine: EngineHandle,
    pub(crate) pool: Arc<EnginePool>,
    pub(crate) pob_root: PathBuf,
}

pub(crate) struct AppState {
    rt: std::sync::RwLock<Runtime>,
    pub(crate) user_dir: PathBuf,
    pub(crate) mcp: mcp::McpState,
    /// True until a game has been chosen, inferred from a file, or set by env.
    first_run: std::sync::atomic::AtomicBool,
    pending_link: std::sync::Mutex<Option<links::Link>>,
    session: SessionInfo,
    /// Cleared once the page has asked, so a reload does not offer recovery twice.
    recovery_pending: std::sync::atomic::AtomicBool,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
struct SessionInfo {
    unclean_exit: bool,
    safe_mode: bool,
}

fn session_marker(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join("session.lock"))
}

/// A leftover marker means a crash, unless another instance runs or another version (an update) wrote it.
fn begin_session(app: &tauri::AppHandle) -> SessionInfo {
    let safe_mode = std::env::args().any(|a| a == "--safe-mode") || std::env::var_os("POB_REDUX_SAFE_MODE").is_some();
    let Some(marker) = session_marker(app) else {
        return SessionInfo { unclean_exit: false, safe_mode };
    };
    let version = app.package_info().version.to_string();
    let unclean_exit = std::fs::read_to_string(&marker).is_ok_and(|m| m.lines().next() == Some(version.as_str())) && !links::instance_running();
    if let Some(dir) = marker.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&marker, format!("{version}\n{}", std::process::id()));
    if unclean_exit {
        log::warn!("session: the last run did not exit cleanly");
    }
    SessionInfo { unclean_exit, safe_mode }
}

#[tauri::command]
fn session_info(state: State<'_, AppState>) -> SessionInfo {
    SessionInfo {
        unclean_exit: state.recovery_pending.swap(false, std::sync::atomic::Ordering::Relaxed),
        safe_mode: state.session.safe_mode,
    }
}

#[tauri::command]
async fn export_diagnostics(app: tauri::AppHandle, state: State<'_, AppState>, path: String) -> Result<(), String> {
    let engine = state.engine().status();
    let pool = state.pool().status();
    let pob = std::fs::read_to_string(state.pob_root().join("SYNC.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .map(|v| {
            let field = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("?").to_string();
            format!("{} ({})", field("upstream_version"), field("upstream_commit").chars().take(12).collect::<String>())
        })
        .unwrap_or_else(|| "unknown".into());
    let mcp = state.mcp.status();
    let facts: Vec<(&str, String)> = vec![
        ("Version", app.package_info().version.to_string()),
        ("System", format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)),
        ("Game", state.game().id().to_string()),
        ("Path of Building", pob),
        ("Engine", format!("{:?}, boot {} ms{}", engine.state, engine.boot_ms.unwrap_or(0), engine.message.map(|m| format!(", {m}")).unwrap_or_default())),
        ("Worker pool", format!("{} of {} ready", pool.ready, pool.size)),
        ("Updates", update_method().to_string()),
        ("MCP server", if mcp.running { format!("on, port {}", mcp.port) } else { "off".into() }),
        ("Last run ended cleanly", if state.session.unclean_exit { "no".into() } else { "yes".into() }),
        ("Safe mode", if state.session.safe_mode { "yes".into() } else { "no".into() }),
    ];
    let mut secrets = ai::stored_keys(&app);
    secrets.extend(mcp::saved_token(&app));
    tauri::async_runtime::spawn_blocking(move || diagnostics::write_report(&app, &path, &facts, &secrets))
        .await
        .map_err(|e| e.to_string())?
}

impl AppState {
    pub(crate) fn engine(&self) -> EngineHandle {
        self.rt.read().unwrap().engine.clone()
    }
    pub(crate) fn pool(&self) -> Arc<EnginePool> {
        self.rt.read().unwrap().pool.clone()
    }
    pub(crate) fn pob_root(&self) -> PathBuf {
        self.rt.read().unwrap().pob_root.clone()
    }
    pub(crate) fn game(&self) -> Game {
        self.rt.read().unwrap().game
    }
    pub(crate) fn builds_dir(&self) -> PathBuf {
        builds_dir(&self.user_dir, self.game())
    }
}

/// Worker engines for parallel scoring: half the cores, capped — each is a
/// ~240 MB Lua state, booted on first use. `POB_REDUX_POOL=n` overrides.
fn pool_size() -> usize {
    if let Some(n) = std::env::var("POB_REDUX_POOL").ok().and_then(|v| v.parse::<usize>().ok()) {
        return n.clamp(1, 16);
    }
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    (cores / 2).clamp(1, 8)
}

#[tauri::command]
fn pool_status(state: State<'_, AppState>) -> PoolStatus {
    state.pool().status()
}

/// How long the pool must go unused before its workers are dropped.
const POOL_IDLE_RELEASE: std::time::Duration = std::time::Duration::from_secs(120);
const POOL_REAP_EVERY: std::time::Duration = std::time::Duration::from_secs(30);

/// Watch the pool and drop its workers once scanning stops. This lives here
/// rather than on a frontend timer because scans also arrive over MCP, which
/// never touches the UI.
///
/// Dropping a worker ends its thread, but mimalloc holds the heap that thread
/// abandoned and a later worker maps fresh pages instead of reusing it, so the
/// process grows by a poolful on every scan cycle. `mi_collect` reclaims those
/// segments. It runs a tick after the release because the threads wind down on
/// their own time, and there is nothing to reclaim until they have.
fn spawn_pool_reaper(app: tauri::AppHandle) {
    // libmimalloc-sys re-exports only the allocation entry points; the symbol
    // itself is in the static library mimalloc links.
    extern "C" {
        fn mi_collect(force: bool);
    }
    std::thread::spawn(move || {
        let mut collect_next = false;
        loop {
            std::thread::sleep(POOL_REAP_EVERY);
            if std::mem::take(&mut collect_next) {
                // SAFETY: mi_collect takes no pointers and is safe to call from
                // any thread at any time.
                unsafe { mi_collect(true) };
                log::info!("pool: reclaimed the released workers' heaps");
            }
            collect_next = app.state::<AppState>().pool().shrink_if_idle(POOL_IDLE_RELEASE) > 0;
        }
    });
}

/// Give back the worker engines once scanning has stopped. Each holds ~240 MB
/// whatever its garbage collector does, so trimming alone does not get it back.
#[tauri::command]
async fn pool_release(state: State<'_, AppState>) -> Result<usize, String> {
    let pool = state.pool();
    tauri::async_runtime::spawn_blocking(move || pool.shrink_if_idle(POOL_IDLE_RELEASE))
        .await
        .map_err(|e| e.to_string())
}

/// Push the current build to the workers while the user is idle, so the
/// next parallel scan skips its sync. Fire-and-forget from the frontend.
#[tauri::command]
async fn pool_presync(state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.engine();
    let pool = state.pool();
    if pool.status().ready < pool.size() {
        return Ok(());
    }
    tauri::async_runtime::spawn_blocking(move || pob_engine::pool::presync_from(&engine, &pool).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

/// Collect garbage in every engine once the user has been idle for a while.
/// A scan leaves each worker holding a few hundred MB of dead calc state,
/// and only freed memory goes back to the OS.
#[tauri::command]
async fn pool_trim(state: State<'_, AppState>) -> Result<(), String> {
    let engine = state.engine();
    let pool = state.pool();
    tauri::async_runtime::spawn_blocking(move || {
        pool.trim();
        engine.call("gc", Value::Null).map(|_| ()).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Node power scored across the worker pool; falls back to PoB's own
/// sequential PowerBuilder if the pool is unavailable.
#[tauri::command]
async fn power_scan_parallel(
    state: State<'_, AppState>,
    stat: Option<String>,
    max_depth: Option<f64>,
) -> Result<CallResult, String> {
    let engine = state.engine();
    let pool = state.pool();
    tauri::async_runtime::spawn_blocking(move || {
        let t0 = std::time::Instant::now();
        let result = match pob_engine::pool::power_scan(&engine, &pool, stat.as_deref(), max_depth) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("parallel power scan failed ({e}); falling back to PowerBuilder");
                engine
                    .call("tree_power", json!({ "stat": stat, "maxDepth": max_depth }))
                    .map_err(|e| e.to_string())?
                    .result
            }
        };
        Ok(CallResult { result, elapsed_ms: t0.elapsed().as_secs_f64() * 1000.0 })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn plan_points_parallel(state: State<'_, AppState>, stat: String, budget: u32) -> Result<CallResult, String> {
    let engine = state.engine();
    let pool = state.pool();
    tauri::async_runtime::spawn_blocking(move || {
        let t0 = std::time::Instant::now();
        let result = pob_engine::pool::plan_points(&engine, &pool, &stat, budget.clamp(1, 120)).map_err(|e| e.to_string())?;
        Ok(CallResult { result, elapsed_ms: t0.elapsed().as_secs_f64() * 1000.0 })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Fill the gem DPS cache for a socket group across the pool so the next
/// DPS-sorted gem search is instant. Best effort: errors leave the
/// sequential path in place.
#[tauri::command]
async fn gem_dps_parallel(state: State<'_, AppState>, group_index: u32) -> Result<CallResult, String> {
    let engine = state.engine();
    let pool = state.pool();
    tauri::async_runtime::spawn_blocking(move || {
        let t0 = std::time::Instant::now();
        let result = pob_engine::pool::gem_dps_fill(&engine, &pool, group_index).map_err(|e| e.to_string())?;
        Ok(CallResult { result, elapsed_ms: t0.elapsed().as_secs_f64() * 1000.0 })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
struct CallResult {
    result: Value,
    elapsed_ms: f64,
}

#[tauri::command]
async fn engine_call(
    state: State<'_, AppState>,
    method: String,
    params: Option<Value>,
) -> Result<CallResult, String> {
    let engine = state.engine();
    let out = tauri::async_runtime::spawn_blocking(move || {
        engine.call(&method, params.unwrap_or(Value::Null))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    Ok(CallResult { result: out.result, elapsed_ms: out.elapsed_ms })
}

#[tauri::command]
fn engine_status(state: State<'_, AppState>) -> EngineStatus {
    state.engine().status()
}

#[derive(Serialize)]
struct AppPaths {
    game: Game,
    pob_root: String,
    user_dir: String,
    builds_dir: String,
    sync: Option<Value>,
    /// A build file to open once the engine is ready: first CLI argument
    /// ending in .xml (file association / drag onto the exe) or POB_REDUX_OPEN.
    open_on_start: Option<String>,
    /// Dev hook: initial tab (POB_REDUX_VIEW), used by the screenshot harness.
    initial_view: Option<String>,
    /// Dev hook: open the assistant panel on boot (POB_REDUX_CHAT).
    /// Set it to "settings" to open the provider sheet too.
    chat_open: Option<String>,
    /// Dev hook: send one message on boot (POB_REDUX_CHAT_ASK). Costs API credit.
    chat_ask: Option<String>,
    /// Dev hook: skip the write-approval gate for that run (POB_REDUX_CHAT_ALLOW).
    chat_allow: Option<String>,
    /// Dev hook: write the transcript here when a run ends (POB_REDUX_CHAT_LOG).
    chat_log: Option<String>,
    /// Dev hooks: provider id and model to select on boot (POB_REDUX_CHAT_PROVIDER, POB_REDUX_CHAT_MODEL).
    chat_provider: Option<String>,
    chat_model: Option<String>,
}

fn open_on_start() -> Option<String> {
    std::env::args()
        .skip(1)
        .find(|a| a.to_ascii_lowercase().ends_with(".xml") && Path::new(a).is_file())
        .or_else(|| std::env::var("POB_REDUX_OPEN").ok().filter(|p| Path::new(p).is_file()))
}

/// How this copy gets updates. The updater replaces the running binary, which on
/// Linux only works for an AppImage. An AUR install updates through pacman, and
/// a .deb or .rpm install has to fetch the new package.
#[tauri::command]
fn update_method() -> &'static str {
    if !cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some() {
        return "self";
    }
    let from_aur = std::fs::read_dir("/var/lib/pacman/local")
        .map(|dir| dir.flatten().any(|e| e.file_name().to_string_lossy().starts_with("pob-redux-bin-")))
        .unwrap_or(false);
    if from_aur {
        "aur"
    } else {
        "package"
    }
}

#[tauri::command]
fn app_paths(state: State<'_, AppState>) -> AppPaths {
    let pob_root = state.pob_root();
    let sync = std::fs::read_to_string(pob_root.join("SYNC.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    AppPaths {
        game: state.game(),
        pob_root: pob_root.to_string_lossy().to_string(),
        user_dir: state.user_dir.to_string_lossy().to_string(),
        builds_dir: state.builds_dir().to_string_lossy().to_string(),
        sync,
        open_on_start: open_on_start(),
        initial_view: std::env::var("POB_REDUX_VIEW").ok(),
        chat_open: std::env::var("POB_REDUX_CHAT").ok(),
        chat_ask: std::env::var("POB_REDUX_CHAT_ASK").ok(),
        chat_allow: std::env::var("POB_REDUX_CHAT_ALLOW").ok(),
        chat_log: std::env::var("POB_REDUX_CHAT_LOG").ok(),
        chat_provider: std::env::var("POB_REDUX_CHAT_PROVIDER").ok(),
        chat_model: std::env::var("POB_REDUX_CHAT_MODEL").ok(),
    }
}

pub(crate) fn builds_dir(user_dir: &Path, game: Game) -> PathBuf {
    user_dir.join(game.user_subdir()).join("Builds")
}

#[tauri::command]
fn read_tree_json(state: State<'_, AppState>, version: String) -> Result<String, String> {
    if !version.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("invalid tree version".into());
    }
    let path = state.pob_root().join("TreeData").join(&version).join("tree.json");
    std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
}

#[derive(Serialize)]
pub(crate) struct BuildEntry {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) folder: String,
    pub(crate) class_name: Option<String>,
    pub(crate) ascend_class_name: Option<String>,
    pub(crate) level: Option<u32>,
    pub(crate) modified: f64,
}

#[tauri::command]
fn list_builds(state: State<'_, AppState>) -> Result<Vec<BuildEntry>, String> {
    scan_builds(&state.builds_dir())
}

pub(crate) fn scan_builds(root: &Path) -> Result<Vec<BuildEntry>, String> {
    let mut out = Vec::new();
    if !root.is_dir() {
        return Ok(out);
    }
    for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("xml") {
            continue;
        }
        let md = entry.metadata().map_err(|e| e.to_string())?;
        let modified = md
            .modified()
            .ok()
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        let head = read_head(path, 8192);
        // Attributes of the <Build> element only; gems further down also carry level="".
        let tag: &str = head
            .find("<Build ")
            .map(|i| {
                let rest = &head[i..];
                &rest[..rest.find('>').unwrap_or(rest.len())]
            })
            .unwrap_or("");
        let attr = |name: &str| -> Option<String> {
            let key = format!(" {name}=\"");
            let i = tag.find(&key)? + key.len();
            let j = tag[i..].find('"')? + i;
            Some(tag[i..j].to_string())
        };
        let folder = path
            .parent()
            .and_then(|p| p.strip_prefix(root).ok())
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        out.push(BuildEntry {
            path: path.to_string_lossy().to_string(),
            name: path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
            folder,
            class_name: attr("className"),
            ascend_class_name: attr("ascendClassName").filter(|s| s != "None"),
            level: attr("level").and_then(|l| l.parse().ok()),
            modified,
        });
    }
    out.sort_by(|a, b| b.modified.partial_cmp(&a.modified).unwrap_or(std::cmp::Ordering::Equal));
    Ok(out)
}

fn read_head(path: &Path, max: usize) -> String {
    use std::io::Read;
    let mut buf = vec![0u8; max];
    let n = std::fs::File::open(path)
        .and_then(|mut f| f.read(&mut buf))
        .unwrap_or(0);
    String::from_utf8_lossy(&buf[..n]).to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FetchedCode {
    pub(crate) site: String,
    pub(crate) code: String,
}

/// Fetch a raw PoB build code from a share link (pobb.in, Maxroll, poe.ninja, …).
/// The result feeds the bridge's `load_build_code`, PoB's own decode path.
#[tauri::command]
async fn fetch_build_code(url: String) -> Result<FetchedCode, String> {
    fetch_code(&url).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SharedLink {
    site: String,
    url: String,
}

/// Upload a build code to a sharing site and return the link, as PoB's
/// Import/Export tab does with its "Share" button.
#[tauri::command]
async fn share_build_code(state: State<'_, AppState>, site: String, code: String) -> Result<SharedLink, String> {
    let game = state.game();
    let target = sites::upload_target(game, &site).ok_or_else(|| {
        format!("Unknown share site {site:?}. Sites: {}.", sites::upload_targets(game).iter().map(|t| t.label).collect::<Vec<_>>().join(", "))
    })?;
    let code = code.trim();
    if code.is_empty() {
        return Err("nothing to share: the build code is empty".into());
    }
    let client = reqwest::Client::builder()
        .user_agent("pob-redux/0.1 Path of Building")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let body = format!("{}{}", target.post_fields, code);
    let content_type = if target.post_fields.is_empty() { "text/plain" } else { "application/x-www-form-urlencoded" };
    let resp = client
        .post(target.post_url)
        .header("content-type", content_type)
        .body(body)
        .send()
        .await
        .map_err(|e| format!("{}: {e}", target.label))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("{}: {e}", target.label))?;
    if !status.is_success() {
        let detail = text.trim();
        let detail = if detail.is_empty() { String::new() } else { format!(": {}", detail.chars().take(200).collect::<String>()) };
        return Err(format!("{} returned HTTP {status}{detail}", target.label));
    }
    let id = text.trim();
    if id.is_empty() || id.contains('<') {
        return Err(format!("{} did not return a link", target.label));
    }
    Ok(SharedLink { site: target.label.to_string(), url: format!("{}{}", target.code_out, id) })
}

/// Resolve a Mobalytics build page: its PoB code, if the author attached one,
/// and one Build Planner file per variant.
#[tauri::command]
async fn mobalytics_resolve(url: String) -> Result<mobalytics::Resolved, String> {
    mobalytics::resolve(&url).await
}

#[tauri::command]
async fn character_list(realm: String, account: String) -> Result<character::CharacterList, String> {
    character::list(&realm, &account).await
}

#[tauri::command]
async fn character_data(realm: String, account: String, character: String) -> Result<character::CharacterData, String> {
    character::data(&realm, &account, &character).await
}

#[tauri::command]
async fn ninja_characters(state: State<'_, AppState>, account: String) -> Result<ninja::CharacterList, String> {
    ninja::list(state.game(), &account).await
}

#[tauri::command]
async fn ninja_character_code(state: State<'_, AppState>, account: String, character: String, league: String) -> Result<String, String> {
    ninja::build_code(state.game(), &account, &character, &league).await
}

/// Resolve a Maxroll build guide or planner to the PoB links it offers.
#[tauri::command]
async fn maxroll_resolve(url: String) -> Result<maxroll::Resolved, String> {
    maxroll::resolve(&url).await
}

#[derive(Deserialize)]
struct GameBuildFile {
    name: String,
    json: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedGameBuild {
    name: String,
    path: String,
    replaced: bool,
}

/// Write Build Planner files into the game's folder. The game only scans the
/// folder root, so every file lands there flat. A file of the same name from
/// the same `link` is replaced (the guide was updated); any other clash gets
/// a numbered suffix.
#[tauri::command]
fn save_game_build_files(state: State<'_, AppState>, dir: Option<String>, files: Vec<GameBuildFile>) -> Result<Vec<SavedGameBuild>, String> {
    let dir = planner_dir(&state, dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out = Vec::with_capacity(files.len());
    for file in files {
        let parsed: serde_json::Value = serde_json::from_str(&file.json).map_err(|e| format!("{}: not a Build Planner file: {e}", file.name))?;
        let link = parsed.get("link").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
        let mut stem: String = file.name.chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')).collect();
        stem = stem.trim().trim_end_matches('.').chars().take(120).collect();
        if stem.is_empty() {
            stem = "Build".into();
        }
        let mut path = dir.join(format!("{stem}.build"));
        let mut replaced = false;
        let mut n = 2;
        while path.exists() {
            let same_source = !link.is_empty()
                && std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                    .and_then(|v| v.get("link").and_then(serde_json::Value::as_str).map(|l| l == link))
                    .unwrap_or(false);
            if same_source {
                replaced = true;
                break;
            }
            path = dir.join(format!("{stem} ({n}).build"));
            n += 1;
        }
        std::fs::write(&path, file.json.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
        out.push(SavedGameBuild { name: stem, path: path.to_string_lossy().to_string(), replaced });
    }
    Ok(out)
}

pub(crate) async fn fetch_code(url: &str) -> Result<FetchedCode, String> {
    if mobalytics::build_slug(url).is_some() {
        let r = mobalytics::resolve(url).await?;
        let code = r.pob_code.ok_or_else(|| {
            format!("That Mobalytics build has no Path of Building code, only {} Build Planner files (use the Builds tab to import those)", r.variants.len())
        })?;
        if code.starts_with("http://") || code.starts_with("https://") {
            return Box::pin(fetch_code(&code)).await;
        }
        return Ok(FetchedCode { site: "Mobalytics".into(), code });
    }
    if maxroll::page(url).is_some() {
        let r = maxroll::resolve(url).await?;
        let first = r.links.first().ok_or_else(|| format!("The Maxroll page {} has no Path of Building link", r.url))?;
        return Box::pin(fetch_code(&first.url)).await;
    }
    let (site, download) =
        sites::download_url(url).ok_or_else(|| format!("Unrecognised build link. Supported sites: {}.", sites::SUPPORTED))?;
    let client = reqwest::Client::builder()
        .user_agent("pob-redux/0.1 Path of Building")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&download).send().await.map_err(|e| format!("{site}: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("{site} returned HTTP {status}"));
    }
    let text = resp.text().await.map_err(|e| format!("{site}: {e}"))?;
    let code = text.trim().to_string();
    if code.is_empty() {
        return Err(format!("{site} returned an empty response"));
    }
    Ok(FetchedCode { site: site.to_string(), code })
}

fn ensure_in_builds(root: &Path, path: &Path) -> Result<(), String> {
    let canon = path.canonicalize().map_err(|e| format!("{}: {e}", path.display()))?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if canon.starts_with(&root) {
        Ok(())
    } else {
        Err("path is outside the builds folder".into())
    }
}

fn safe_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) || name == "." || name == ".." {
        return Err(format!("invalid name: {name:?}"));
    }
    Ok(name)
}

fn safe_folder(root: &Path, folder: &str) -> Result<PathBuf, String> {
    let mut dir = root.to_path_buf();
    for seg in folder.split('/').filter(|s| !s.is_empty()) {
        dir.push(safe_name(seg)?);
    }
    Ok(dir)
}

#[tauri::command]
fn rename_build(state: State<'_, AppState>, path: String, new_name: String) -> Result<String, String> {
    let src = PathBuf::from(&path);
    ensure_in_builds(&state.builds_dir(), &src)?;
    let dst = src.with_file_name(format!("{}.xml", safe_name(&new_name)?));
    if dst.exists() {
        return Err(format!("{} already exists", dst.display()));
    }
    std::fs::rename(&src, &dst).map_err(|e| e.to_string())?;
    Ok(dst.to_string_lossy().to_string())
}

#[tauri::command]
fn move_build(state: State<'_, AppState>, path: String, folder: String) -> Result<String, String> {
    let root = state.builds_dir();
    let src = PathBuf::from(&path);
    ensure_in_builds(&root, &src)?;
    let dir = safe_folder(&root, &folder)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dst = dir.join(src.file_name().ok_or("bad path")?);
    if dst == src {
        return Ok(path);
    }
    if dst.exists() {
        return Err(format!("{} already exists", dst.display()));
    }
    std::fs::rename(&src, &dst).map_err(|e| e.to_string())?;
    Ok(dst.to_string_lossy().to_string())
}

#[tauri::command]
fn delete_build(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let src = PathBuf::from(&path);
    ensure_in_builds(&state.builds_dir(), &src)?;
    if src.extension().and_then(|e| e.to_str()) != Some("xml") {
        return Err("only .xml builds can be deleted".into());
    }
    std::fs::remove_file(&src).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_build_folder(state: State<'_, AppState>, folder: String) -> Result<(), String> {
    let dir = safe_folder(&state.builds_dir(), &folder)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_build_folder(state: State<'_, AppState>, folder: String) -> Result<(), String> {
    let root = state.builds_dir();
    let dir = safe_folder(&root, &folder)?;
    if dir == root {
        return Err("that is the builds folder itself".into());
    }
    if !dir.is_dir() {
        return Err(format!("{} is not a folder", dir.display()));
    }
    // Only an empty one goes, so a build is never deleted along with it.
    if std::fs::read_dir(&dir).map_err(|e| e.to_string())?.next().is_some() {
        return Err("the folder still has something in it".into());
    }
    std::fs::remove_dir(&dir).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_build_folders(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let root = state.builds_dir();
    let mut out = Vec::new();
    if !root.is_dir() {
        return Ok(out);
    }
    for entry in walkdir::WalkDir::new(&root).min_depth(1).into_iter().flatten() {
        if entry.file_type().is_dir() {
            if let Ok(rel) = entry.path().strip_prefix(&root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameBuildList {
    dir: String,
    exists: bool,
    builds: Vec<GameBuildEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameBuildEntry {
    path: String,
    name: String,
    author: Option<String>,
    ascendancy: Option<String>,
    modified: f64,
}

/// The game's Build Planner folder, or the user's override of it.
fn planner_dir(state: &AppState, dir: Option<String>) -> PathBuf {
    dir.filter(|d| !d.trim().is_empty()).map(PathBuf::from).unwrap_or_else(|| {
        // user_dir is the Documents folder (PoB appends its own subdir)
        state.user_dir.join("My Games").join("Path of Exile 2").join("BuildPlanner")
    })
}

/// List the game's Build Planner files (*.build). `dir` overrides the default
/// `Documents\My Games\Path of Exile 2\BuildPlanner`.
#[tauri::command]
fn list_game_builds(state: State<'_, AppState>, dir: Option<String>) -> Result<GameBuildList, String> {
    let dir = planner_dir(&state, dir);
    let mut builds = Vec::new();
    let exists = dir.is_dir();
    if exists {
        for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("build") {
                continue;
            }
            let modified = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            // name and author live in the JSON; the filename is the game's
            // truncated copy of the name
            let json = read_text_lossy(&path.to_string_lossy())
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok());
            let field = |key: &str| {
                json.as_ref()
                    .and_then(|v| v.get(key))
                    .and_then(|n| n.as_str())
                    .filter(|n| !n.trim().is_empty())
                    .map(|n| n.to_string())
            };
            builds.push(GameBuildEntry {
                path: path.to_string_lossy().to_string(),
                name: field("name").unwrap_or(stem),
                author: field("author"),
                ascendancy: field("ascendancy").or_else(|| field("ascendancy_class")),
                modified,
            });
        }
        builds.sort_by(|a, b| b.modified.partial_cmp(&a.modified).unwrap_or(std::cmp::Ordering::Equal));
    }
    Ok(GameBuildList { dir: dir.to_string_lossy().to_string(), exists, builds })
}

/// Set the `name` and/or `author` of a game Build Planner file in place;
/// an empty author clears it. The rest of the JSON is kept as the game wrote
/// it, so a later import sees the same build. The file keeps its own name:
/// the game shows the name inside the JSON.
#[tauri::command]
fn set_game_build_meta(path: String, name: Option<String>, author: Option<String>) -> Result<(), String> {
    if Path::new(&path).extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("build")) != Some(true) {
        return Err("not a .build file".into());
    }
    let text = read_text_lossy(&path)?;
    let mut v: Value = serde_json::from_str(&text).map_err(|e| format!("{path}: not a valid .build file: {e}"))?;
    let obj = v.as_object_mut().ok_or_else(|| format!("{path}: not a valid .build file"))?;
    if let Some(name) = name {
        let name = name.trim();
        if name.is_empty() {
            return Err("a build needs a name".into());
        }
        obj.insert("name".into(), Value::String(name.to_string()));
    }
    if let Some(author) = author {
        let author = author.trim();
        if author.is_empty() {
            obj.remove("author");
        } else {
            obj.insert("author".into(), Value::String(author.to_string()));
        }
    }
    let out = serde_json::to_string(&v).map_err(|e| e.to_string())?;
    std::fs::write(&path, out).map_err(|e| format!("{path}: {e}"))
}

/// Read a text file tolerantly: UTF-16 (either BOM) is converted, a UTF-8 BOM
/// is stripped, and invalid UTF-8 bytes are replaced rather than failing.
pub(crate) fn read_text_lossy(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let text = if bytes.starts_with(&[0xFF, 0xFE]) {
        let utf16: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&utf16)
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        let utf16: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        String::from_utf16_lossy(&utf16)
    } else {
        let b = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&bytes);
        String::from_utf8_lossy(b).into_owned()
    };
    Ok(text)
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    read_text_lossy(&path)
}

#[tauri::command]
fn write_text_file(path: String, contents: String) -> Result<(), String> {
    if let Some(parent) = Path::new(&path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, contents).map_err(|e| format!("{path}: {e}"))
}

/// The assistant's log file: one JSON line per run, written by the panel so
/// a failure can be reported with its context. Lives next to providers.json.
fn ai_log_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?.join("logs");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("assistant.log"))
}

/// Append one line to the assistant log, rotating once it passes 4 MB so it
/// never grows without bound. Returns the log path.
#[tauri::command]
fn ai_log_append(app: tauri::AppHandle, line: String) -> Result<String, String> {
    use std::io::Write;
    let path = ai_log_path(&app)?;
    if std::fs::metadata(&path).map(|m| m.len() > 4 * 1024 * 1024).unwrap_or(false) {
        let _ = std::fs::rename(&path, path.with_extension("log.1"));
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let line = line.replace(['\n', '\r'], " ");
    writeln!(f, "{line}").map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// Show the assistant log in the file manager.
#[tauri::command]
fn ai_log_reveal(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_opener::OpenerExt;
    let path = ai_log_path(&app)?;
    if !path.is_file() {
        std::fs::write(&path, "").map_err(|e| e.to_string())?;
    }
    app.opener().reveal_item_in_dir(&path).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

/// The system instructions for the chat panel: the same text the MCP server
/// sends to external clients.
#[tauri::command]
fn ai_instructions() -> &'static str {
    tools::INSTRUCTIONS
}

/// The tool registry as JSON Schema, for the in-app chat panel. Same list the
/// MCP server serves over `tools/list`.
#[tauri::command]
fn ai_tools() -> Vec<tools::ToolDef> {
    tools::defs()
}

/// Run one tool against the open build. Goes through `tools::dispatch`, so it
/// behaves exactly as the same call would over MCP and emits `mcp:changed`.
#[tauri::command]
async fn ai_call_tool(
    app: tauri::AppHandle,
    name: String,
    args: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<serde_json::Value, String> {
    let ctx = mcp::tool_context(&app);
    tools::dispatch(ctx, name, args.unwrap_or_default())
        .await
        .map(|(value, _)| value)
        .map_err(|e| match e {
            tools::ToolError::Invalid(m) | tools::ToolError::Failed(m) => m,
        })
}

/// Locate a game's vendored PoB program: an explicit override, the bundled
/// resources, or (dev builds) the pob-sync output next to this crate.
fn find_pob_root(app: &tauri::AppHandle, game: Game) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var(game.env_root()) {
        candidates.push(PathBuf::from(p));
    }
    if let Ok(res) = app.path().resource_dir() {
        candidates.push(res.join(game.resource_dir()));
        candidates.push(res.join("resources").join(game.resource_dir()));
    }
    if cfg!(debug_assertions) {
        candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("resources").join(game.resource_dir()));
    }
    candidates.into_iter().find(|p| p.join("Launch.lua").is_file())
}

/// The game to boot: `POB_REDUX_GAME`, then the link or build file the app
/// was opened with, then the saved choice. With none of those the app boots
/// PoE2 and asks (`first_run`).
fn startup_game(app: &tauri::AppHandle, link: Option<&links::Link>) -> (Game, bool) {
    if let Some(g) = std::env::var("POB_REDUX_GAME").ok().and_then(|v| Game::from_id(&v)) {
        return (g, false);
    }
    if let Some(link) = link {
        return (link.game, false);
    }
    if let Some(g) = open_on_start().and_then(|p| Game::of_build_xml(&read_head(Path::new(&p), 4096))) {
        return (g, false);
    }
    match game::load_settings(app).game {
        Some(g) => (g, false),
        None => (Game::Poe2, true),
    }
}

/// Boot an engine, and an idle pool, on a game's PoB root.
fn boot_runtime(app: &tauri::AppHandle, game: Game, user_dir: &Path) -> Runtime {
    let pob_root = find_pob_root(app, game).unwrap_or_else(|| {
        log::error!("no PoB program found for {}; run `cargo run -p pob-sync -- --game {}` first", game.id(), game.id());
        PathBuf::from("resources").join(game.resource_dir())
    });
    log::info!("pob root: {}", pob_root.display());
    let cfg = EngineConfig { pob_root: pob_root.clone(), user_dir: user_dir.to_path_buf() };
    let engine = EngineHandle::spawn(cfg.clone());
    // Workers boot on the first scan and are released again once the pool
    // goes idle. Warming them here cost every session ~240 MB per worker for
    // a feature most sessions never reach.
    let pool = Arc::new(EnginePool::new(cfg, pool_size()));
    Runtime { game, engine, pool, pob_root }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameStatus {
    game: Game,
    first_run: bool,
}

fn game_status_of(state: &AppState) -> GameStatus {
    GameStatus {
        game: state.game(),
        first_run: state.first_run.load(std::sync::atomic::Ordering::Relaxed),
    }
}

#[tauri::command]
fn game_status(state: State<'_, AppState>) -> GameStatus {
    game_status_of(&state)
}

/// Choose the game. A change swaps the engine and pool for ones booted on the
/// other PoB; the caller polls `engine_status` until it is ready. The choice
/// is saved for the next start.
#[tauri::command]
fn set_game(app: tauri::AppHandle, state: State<'_, AppState>, game: Game) -> Result<GameStatus, String> {
    state.first_run.store(false, std::sync::atomic::Ordering::Relaxed);
    game::save_settings(&app, &game::Settings { game: Some(game) })?;
    if state.game() != game {
        if find_pob_root(&app, game).is_none() {
            return Err(format!("{} data is missing: run pob-sync --game {}", game.user_subdir(), game.id()));
        }
        if game == Game::Poe1 {
            // The MCP server and the assistant are PoE2 features.
            state.mcp.stop();
        }
        let fresh = boot_runtime(&app, game, &state.user_dir);
        let old = std::mem::replace(&mut *state.rt.write().unwrap(), fresh);
        old.pool.release();
        old.engine.shutdown();
    }
    Ok(game_status_of(&state))
}

/// Which game a build file belongs to, from its root element.
#[tauri::command]
fn build_file_game(path: String) -> Option<Game> {
    Game::of_build_xml(&read_head(Path::new(&path), 4096))
}

#[tauri::command]
fn build_xml_game(xml: String) -> Option<Game> {
    Game::of_build_xml(&xml)
}

fn find_user_dir(app: &tauri::AppHandle) -> PathBuf {
    if let Ok(p) = std::env::var("POB_REDUX_USER_DIR") {
        return PathBuf::from(p);
    }
    app.path()
        .document_dir()
        .or_else(|_| app.path().app_data_dir())
        .unwrap_or_else(|_| std::env::temp_dir().join("pob-redux"))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Serves files from the vendored PoB directory to the webview as
/// `pobasset://localhost/<relative path>` (Windows: `http://pobasset.localhost/...`).
/// Used for tree sprite sheets, which are too large to ship over IPC.
fn serve_pob_asset(
    ctx: tauri::UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<std::borrow::Cow<'static, [u8]>> {
    use tauri::http::{header, Response, StatusCode};
    let respond = |status: StatusCode, body: Vec<u8>, mime: &str| {
        Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, mime)
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(header::CACHE_CONTROL, "max-age=3600")
            .body(std::borrow::Cow::Owned(body))
            .unwrap()
    };
    let state = ctx.app_handle().state::<AppState>();
    let rel = percent_decode(request.uri().path().trim_start_matches('/'));
    if rel.contains("..") {
        return respond(StatusCode::FORBIDDEN, Vec::new(), "text/plain");
    }
    let path = state.pob_root().join(&rel);
    let mime = match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("json") => "application/json",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    };
    match std::fs::read(&path) {
        Ok(bytes) => respond(StatusCode::OK, bytes, mime),
        Err(_) => respond(StatusCode::NOT_FOUND, Vec::new(), "text/plain"),
    }
}

/// Windows and Linux pass the launch link as an argument; macOS through the deep-link plugin.
fn start_link(app: &tauri::AppHandle) -> Option<links::Link> {
    if let Some(link) = links::from_args() {
        return Some(link);
    }
    #[cfg(desktop)]
    {
        use tauri_plugin_deep_link::DeepLinkExt;
        if let Ok(Some(urls)) = app.deep_link().get_current() {
            return urls.iter().find_map(|u| links::parse(u.as_str()));
        }
    }
    None
}

fn deliver_link(app: &tauri::AppHandle, link: links::Link) {
    log::info!("link: {}", link.url);
    *app.state::<AppState>().pending_link.lock().unwrap() = Some(link);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    let _ = app.emit("open-link", ());
}

fn watch_links(app: &tauri::AppHandle) {
    let handle = app.clone();
    links::listen(move |link| deliver_link(&handle, link));
    #[cfg(desktop)]
    {
        use tauri_plugin_deep_link::DeepLinkExt;
        // An AppImage has no installer to register the schemes.
        #[cfg(target_os = "linux")]
        if std::env::var_os("APPIMAGE").is_some() {
            if let Err(e) = app.deep_link().register_all() {
                log::warn!("links: could not register the pob schemes: {e}");
            }
        }
        let handle = app.clone();
        app.deep_link().on_open_url(move |event| {
            for url in event.urls() {
                if let Some(link) = links::parse(url.as_str()) {
                    deliver_link(&handle, link);
                }
            }
        });
    }
}

#[tauri::command]
fn take_open_link(state: State<'_, AppState>) -> Option<links::Link> {
    state.pending_link.lock().unwrap().take()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // A link clicked while an instance runs goes to that instance, and this process stops.
    if let Some(raw) = std::env::args().skip(1).find(|a| links::parse(a).is_some()) {
        if links::hand_off(&raw) {
            return;
        }
    }
    diagnostics::install_panic_hook();

    tauri::Builder::default()
        .plugin(diagnostics::log_plugin())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init())
        .register_uri_scheme_protocol("pobasset", serve_pob_asset)
        .register_asynchronous_uri_scheme_protocol("pobart", art::serve)
        .setup(|app| {
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_deep_link::init())?;
            let handle = app.handle();
            let link = start_link(handle);
            let user_dir = find_user_dir(handle);
            let (game, first_run) = startup_game(handle, link.as_ref());
            log::info!("game: {}", game.id());
            log::info!("user dir: {}", user_dir.display());
            let rt = boot_runtime(handle, game, &user_dir);
            let session = begin_session(handle);
            app.manage(AppState {
                rt: std::sync::RwLock::new(rt),
                user_dir,
                mcp: mcp::McpState::new(),
                first_run: std::sync::atomic::AtomicBool::new(first_run),
                pending_link: std::sync::Mutex::new(link),
                recovery_pending: std::sync::atomic::AtomicBool::new(session.unclean_exit),
                session,
            });
            watch_links(app.handle());
            spawn_pool_reaper(app.handle().clone());
            app.manage(ai::AiState::new(&app.handle().clone()));
            app.manage(agent::AgentState::new(&app.handle().clone()));
            if game == Game::Poe2 {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    agent::statuses(&handle, false, None).await;
                });
            }
            app.manage(decide::DecideState::new(&app.handle().clone()));
            // POB_REDUX_MCP=<port> brings the MCP server up at launch (scripts, tests)
            if let Some(port) = std::env::var("POB_REDUX_MCP").ok().and_then(|v| v.parse::<u16>().ok()) {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = mcp::start_with_handle(&handle, port).await {
                        log::error!("mcp autostart: {e}");
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_call,
            engine_status,
            pool_status,
            pool_presync,
            pool_trim,
            pool_release,
            power_scan_parallel,
            plan_points_parallel,
            gem_dps_parallel,
            app_paths,
            update_method,
            take_open_link,
            session_info,
            export_diagnostics,
            diagnostics::log_frontend,
            diagnostics::reveal_logs,
            game_status,
            set_game,
            build_file_game,
            build_xml_game,
            read_tree_json,
            list_builds,
            list_game_builds,
            read_text_file,
            write_text_file,
            fetch_build_code,
            mobalytics_resolve,
            maxroll_resolve,
            character_list,
            character_data,
            ninja_characters,
            ninja_character_code,
            save_game_build_files,
            share_build_code,
            set_game_build_meta,
            rename_build,
            move_build,
            delete_build,
            create_build_folder,
            delete_build_folder,
            list_build_folders,
            mcp::mcp_status,
            mcp::mcp_start,
            mcp::mcp_stop,
            ai_tools,
            ai_instructions,
            ai_log_append,
            ai_log_reveal,
            ai_call_tool,
            ai::ai_providers,
            ai::ai_key_set,
            ai::ai_key_clear,
            ai::ai_base_set,
            ai::ai_models,
            ai::ai_warm_model,
            ai::ai_chat_stream,
            agent::agent_open,
            agent::agent_send,
            agent::agent_approve,
            agent::agent_stop,
            agent::agent_configure,
            agent::agent_close,
            agent::agent_path_set,
            agent::agent_install,
            agent::agent_sign_in,
            agent::agent_sign_in_cancel,
            agent::agent_sign_out,
            agent::agent_terminal,
            agent::agent_uninstall,
            decide::decide_status,
            decide::decide_select,
            decide::decide_configure,
            decide::decide_ask,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<agent::AgentState>().shutdown();
                if let Some(marker) = session_marker(app) {
                    let _ = std::fs::remove_file(marker);
                }
            }
        });
}

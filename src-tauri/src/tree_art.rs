//! Passive tree art: downloaded once from R2 into the app's data folder; the bundle ships only small offline copies.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::game::Game;
use crate::AppState;

const BASE: &str = "https://art.pobredux.com/tree";
const PARALLEL: usize = 6;

#[derive(Deserialize)]
struct FileList {
    files: BTreeMap<String, ArtFile>,
    offline: Vec<String>,
}

#[derive(Deserialize)]
struct ArtFile {
    hash: String,
    size: u64,
}

fn key(rel: &str, file: &ArtFile) -> String {
    let ext = rel.rsplit_once('.').map_or("bin", |(_, e)| e);
    format!("{}.{ext}", file.hash)
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    game: Game,
    /// "none" (this build has no tree art), "checking", "downloading", "ready" or "failed".
    state: &'static str,
    done: u64,
    total: u64,
    error: Option<String>,
}

impl Status {
    fn new(game: Game, state: &'static str) -> Status {
        Status { game, state, done: 0, total: 0, error: None }
    }
}

fn statuses() -> &'static Mutex<HashMap<&'static str, Status>> {
    static S: OnceLock<Mutex<HashMap<&'static str, Status>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

fn publish(app: &AppHandle, status: Status) {
    statuses().lock().unwrap().insert(status.game.id(), status.clone());
    let _ = app.emit("tree-art", status);
}

fn base_url() -> String {
    std::env::var("POB_REDUX_TREE_ART_URL").unwrap_or_else(|_| BASE.to_string())
}

fn store_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_local_data_dir().ok().map(|d| d.join("tree-art"))
}

/// Dev builds also read the files `bun run sync` built, unless a download is being tested.
fn dev_store() -> Option<PathBuf> {
    if !cfg!(debug_assertions) || std::env::var_os("POB_REDUX_TREE_ART_URL").is_some() {
        return None;
    }
    Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/tree-art/store"))
}

fn present(store: &Path, key: &str) -> bool {
    store.join(key).is_file() || dev_store().is_some_and(|d| d.join(key).is_file())
}

type ArtList = (String, Arc<FileList>);

/// The tree version with art and its file list, from a PoB root's SYNC.json.
fn file_list(pob_root: &Path) -> Option<ArtList> {
    static LISTS: OnceLock<Mutex<HashMap<PathBuf, ArtList>>> = OnceLock::new();
    let lists = LISTS.get_or_init(Default::default);
    if let Some(found) = lists.lock().unwrap().get(pob_root) {
        return Some(found.clone());
    }
    let sync: serde_json::Value = serde_json::from_slice(&std::fs::read(pob_root.join("SYNC.json")).ok()?).ok()?;
    let version = sync.get("tree_art")?.as_str()?.to_string();
    let path = pob_root.join("TreeData").join(&version).join("web").join("files.json");
    let list: FileList = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    let found = (version, Arc::new(list));
    lists.lock().unwrap().insert(pob_root.to_path_buf(), found.clone());
    Some(found)
}

/// Where a `TreeData/<version>/web/...` file the bundle does not ship lives on disk.
pub fn locate(app: &AppHandle, pob_root: &Path, rel: &str) -> Option<PathBuf> {
    let (version, inner) = rel.strip_prefix("TreeData/")?.split_once("/web/")?;
    let (art_version, list) = file_list(pob_root)?;
    if art_version != version {
        return None;
    }
    let key = key(inner, list.files.get(inner)?);
    let stored = store_dir(app)?.join(&key);
    if stored.is_file() {
        return Some(stored);
    }
    dev_store().map(|d| d.join(&key)).filter(|p| p.is_file())
}

/// Downloads the game's missing tree art in the background, reporting progress as `tree-art` events.
pub fn ensure(app: &AppHandle, game: Game, pob_root: PathBuf) {
    if matches!(statuses().lock().unwrap().get(game.id()).map(|s| s.state), Some("checking" | "downloading")) {
        return;
    }
    publish(app, Status::new(game, "checking"));
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let status = download(&app, game, &pob_root).await;
        let ready = status.state == "ready";
        if let Some(e) = &status.error {
            log::warn!("tree art for {}: {e}", game.id());
        }
        publish(&app, status);
        if ready {
            cleanup(&app);
        }
    });
}

async fn download(app: &AppHandle, game: Game, pob_root: &Path) -> Status {
    let Some((_, list)) = file_list(pob_root) else { return Status::new(game, "none") };
    let Some(store) = store_dir(app) else {
        return Status { error: Some("no app data folder".into()), ..Status::new(game, "failed") };
    };
    let offline: HashSet<&str> = list.offline.iter().map(String::as_str).collect();
    let todo: Vec<(String, String, u64)> = list
        .files
        .iter()
        .filter(|(rel, _)| !offline.contains(rel.as_str()))
        .map(|(rel, f)| (key(rel, f), f.hash.clone(), f.size))
        .filter(|(key, _, _)| !present(&store, key))
        .collect();
    let total: u64 = todo.iter().map(|(_, _, size)| size).sum();
    if todo.is_empty() {
        return Status::new(game, "ready");
    }
    publish(app, Status { total, ..Status::new(game, "downloading") });
    let client = match reqwest::Client::builder()
        .user_agent(concat!("pob-redux/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
    {
        Ok(c) => c,
        Err(e) => return Status { total, error: Some(e.to_string()), ..Status::new(game, "failed") },
    };
    let base = base_url();
    let permits = Arc::new(tokio::sync::Semaphore::new(PARALLEL));
    let done = Arc::new(AtomicU64::new(0));
    let reported = Arc::new(AtomicU64::new(0));
    let mut tasks = tokio::task::JoinSet::new();
    for (key, hash, size) in todo {
        let (client, permits, done, reported, store, app) = (client.clone(), permits.clone(), done.clone(), reported.clone(), store.clone(), app.clone());
        let url = format!("{base}/{key}");
        tasks.spawn(async move {
            let _permit = permits.acquire_owned().await.map_err(|e| e.to_string())?;
            let bytes = match fetch(&client, &url, &hash).await {
                Ok(bytes) => bytes,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    fetch(&client, &url, &hash).await?
                }
            };
            if !crate::art::write_atomic(&store.join(&key), &bytes) {
                return Err(format!("could not write {}", store.join(&key).display()));
            }
            let now = done.fetch_add(size, Ordering::Relaxed) + size;
            if now - reported.load(Ordering::Relaxed) >= total / 100 || now == total {
                reported.store(now, Ordering::Relaxed);
                publish(&app, Status { done: now, total, ..Status::new(game, "downloading") });
            }
            Ok::<(), String>(())
        });
    }
    // One failed file stops the rest, so an offline start falls back to the shipped copies quickly.
    let mut error = None;
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Err(e) if e.is_cancelled() => {}
            Ok(Err(e)) => {
                error.get_or_insert(e);
                tasks.abort_all();
            }
            Err(e) => {
                error.get_or_insert(e.to_string());
                tasks.abort_all();
            }
        }
    }
    let done = done.load(Ordering::Relaxed);
    match error {
        Some(e) => Status { done, total, error: Some(e), ..Status::new(game, "failed") },
        None => Status { done, total, ..Status::new(game, "ready") },
    }
}

async fn fetch(client: &reqwest::Client, url: &str, hash: &str) -> Result<Vec<u8>, String> {
    let res = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("{url}: {}", res.status()));
    }
    let bytes = res.bytes().await.map_err(|e| e.to_string())?;
    let actual: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    if actual != hash {
        return Err(format!("{url} does not match its hash"));
    }
    Ok(bytes.to_vec())
}

/// Deletes stored files neither game's current art lists.
fn cleanup(app: &AppHandle) {
    let Some(store) = store_dir(app) else { return };
    let mut keep = HashSet::new();
    for game in [Game::Poe1, Game::Poe2] {
        let Some((_, list)) = crate::find_pob_root(app, game).and_then(|root| file_list(&root)) else { return };
        keep.extend(list.files.iter().map(|(rel, f)| key(rel, f)));
    }
    for entry in std::fs::read_dir(&store).into_iter().flatten().flatten() {
        if !keep.contains(entry.file_name().to_string_lossy().as_ref()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[tauri::command]
pub fn tree_art_status(state: State<'_, AppState>) -> Status {
    let game = state.game();
    statuses().lock().unwrap().get(game.id()).cloned().unwrap_or_else(|| Status::new(game, "checking"))
}

#[tauri::command]
pub fn tree_art_retry(app: AppHandle, state: State<'_, AppState>) {
    ensure(&app, state.game(), state.pob_root());
}

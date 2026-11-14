//! Passive tree art: downloaded once from R2 into the app's data folder; the bundle ships only small offline copies.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::downloads::{self, Job, Status};
use crate::game::Game;
use crate::AppState;

#[derive(Deserialize)]
struct FileList {
    files: BTreeMap<String, ArtFile>,
    offline: HashSet<String>,
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

fn statuses() -> &'static Mutex<HashMap<&'static str, Status>> {
    static S: OnceLock<Mutex<HashMap<&'static str, Status>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

fn publish(app: &AppHandle, status: Status) {
    statuses().lock().unwrap().insert(status.game.id(), status.clone());
    let _ = app.emit("tree-art", status);
}

fn store_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_local_data_dir().ok().map(|d| d.join("tree-art"))
}

fn dev_store() -> Option<PathBuf> {
    downloads::dev_files().then(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/tree-art/store"))
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

/// The file behind a PoB root path, or `None` when downloaded art is not in the store yet.
pub fn locate(app: &AppHandle, pob_root: &Path, rel: &str) -> Option<PathBuf> {
    let bundled = pob_root.join(rel);
    let Some((version, inner)) = rel.strip_prefix("TreeData/").and_then(|r| r.split_once("/web/")) else { return Some(bundled) };
    let Some((art_version, list)) = file_list(pob_root) else { return Some(bundled) };
    let Some(file) = list.files.get(inner).filter(|_| art_version == version) else { return Some(bundled) };
    if list.offline.contains(inner) {
        return Some(bundled);
    }
    // An update leaves the previous version's art in the install folder, under the same names.
    let key = key(inner, file);
    store_dir(app).into_iter().chain(dev_store()).map(|d| d.join(&key)).find(|p| p.is_file())
}

/// Downloads the game's missing tree art, reporting progress as `tree-art` events.
pub async fn ensure(app: &AppHandle, game: Game, pob_root: &Path) {
    if matches!(statuses().lock().unwrap().get(game.id()).map(|s| s.state), Some("checking" | "downloading")) {
        return;
    }
    publish(app, Status::new(game, "checking"));
    let status = download(app, game, pob_root).await;
    let ready = status.state == "ready";
    if let Some(e) = &status.error {
        log::warn!("tree art for {}: {e}", game.id());
    }
    publish(app, status);
    if ready {
        cleanup(app);
    }
}

async fn download(app: &AppHandle, game: Game, pob_root: &Path) -> Status {
    let Some((_, list)) = file_list(pob_root) else { return Status::new(game, "none") };
    let Some(store) = store_dir(app) else {
        return Status { error: Some("no app data folder".into()), ..Status::new(game, "failed") };
    };
    let base = downloads::base_url();
    let jobs: Vec<Job> = list
        .files
        .iter()
        .filter(|(rel, _)| !list.offline.contains(rel.as_str()))
        .map(|(rel, f)| (key(rel, f), f))
        .filter(|(key, _)| !present(&store, key))
        .map(|(key, f)| Job { url: format!("{base}/tree/{key}"), hash: f.hash.clone(), size: f.size, dest: store.join(&key) })
        .collect();
    let total: u64 = jobs.iter().map(|j| j.size).sum();
    if jobs.is_empty() {
        return Status::new(game, "ready");
    }
    publish(app, Status { total, ..Status::new(game, "downloading") });
    let reporter = app.clone();
    let progress = downloads::throttled(total, move |done| publish(&reporter, Status { done, total, ..Status::new(game, "downloading") }));
    match downloads::fetch_all(jobs, progress).await {
        Ok(()) => Status { done: total, total, ..Status::new(game, "ready") },
        Err(e) => Status { total, error: Some(e), ..Status::new(game, "failed") },
    }
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
    downloads::start(&app, state.game(), state.pob_root());
}

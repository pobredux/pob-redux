//! PoE1's timeless jewel archives (170 MB): downloaded once from R2 into the folder the engine reads them from.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::downloads::{self, Job, Status};
use crate::game::Game;
use crate::AppState;

/// `downloads.json`, which pob-sync writes into each PoB root.
#[derive(Deserialize)]
struct FileList {
    files: BTreeMap<String, DataFile>,
}

#[derive(Deserialize)]
struct DataFile {
    hash: String,
    size: u64,
}

fn statuses() -> &'static Mutex<HashMap<&'static str, Status>> {
    static S: OnceLock<Mutex<HashMap<&'static str, Status>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

fn publish(app: &AppHandle, status: Status) {
    statuses().lock().unwrap().insert(status.game.id(), status.clone());
    let _ = app.emit("game-data", status);
}

/// The folder the engine reads downloaded files from, by their PoB path.
pub fn dir(app: &AppHandle, game: Game) -> Option<PathBuf> {
    let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources").join(format!("{}-data", game.resource_dir()));
    if downloads::dev_files() && dev.is_dir() {
        return Some(dev);
    }
    app.path().app_local_data_dir().ok().map(|d| d.join("game-data").join(game.resource_dir()))
}

/// Downloads the game's missing data files, then makes the engine read them again.
pub async fn ensure(app: &AppHandle, game: Game, pob_root: &Path) {
    if matches!(statuses().lock().unwrap().get(game.id()).map(|s| s.state), Some("checking" | "downloading")) {
        return;
    }
    publish(app, Status::new(game, "checking"));
    let status = download(app, game, pob_root).await;
    if let Some(e) = &status.error {
        log::warn!("game data for {}: {e}", game.id());
    }
    let state = app.state::<AppState>();
    if status.state == "ready" && status.total > 0 && state.game() == game {
        let (engine, pool) = (state.engine(), state.pool());
        let reloaded = tauri::async_runtime::spawn_blocking(move || {
            pool.release();
            engine.call("reload_timeless_data", serde_json::json!({}))
        })
        .await;
        if let Ok(Err(e)) = reloaded {
            log::warn!("reload timeless data: {e}");
        }
    }
    publish(app, status);
}

async fn download(app: &AppHandle, game: Game, pob_root: &Path) -> Status {
    let list: Option<FileList> = std::fs::read(pob_root.join("downloads.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let Some(list) = list.filter(|l| !l.files.is_empty()) else { return Status::new(game, "none") };
    let Some(dir) = dir(app, game) else {
        return Status { error: Some("no app data folder".into()), ..Status::new(game, "failed") };
    };
    let base = downloads::base_url();
    let jobs: Vec<Job> = list
        .files
        .iter()
        .filter(|(rel, _)| !rel.contains(".."))
        .filter(|(rel, f)| !downloads::present(&dir.join(rel), f.size) && !downloads::present(&pob_root.join(rel), f.size))
        .map(|(rel, f)| Job { url: format!("{base}/data/{}", f.hash), hash: f.hash.clone(), size: f.size, dest: dir.join(rel) })
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

#[tauri::command]
pub fn game_data_status(state: State<'_, AppState>) -> Status {
    let game = state.game();
    statuses().lock().unwrap().get(game.id()).cloned().unwrap_or_else(|| Status::new(game, "checking"))
}

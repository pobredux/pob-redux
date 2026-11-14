//! Files the app downloads once from R2 instead of shipping in every update: tree art, then PoE1's timeless jewel data.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use crate::game::Game;

const BASE: &str = "https://art.pobredux.com";
const PARALLEL: usize = 6;

/// `art.pobredux.com`, or `POB_REDUX_DOWNLOAD_URL` when testing against another server.
pub fn base_url() -> String {
    std::env::var("POB_REDUX_DOWNLOAD_URL").unwrap_or_else(|_| BASE.to_string())
}

/// Dev builds read what `bun run sync` built instead of downloading, unless a download is being tested.
pub fn dev_files() -> bool {
    cfg!(debug_assertions) && std::env::var_os("POB_REDUX_DOWNLOAD_URL").is_none()
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub game: Game,
    /// "none" (nothing to download), "checking", "downloading", "ready" or "failed".
    pub state: &'static str,
    pub done: u64,
    pub total: u64,
    pub error: Option<String>,
}

impl Status {
    pub fn new(game: Game, state: &'static str) -> Status {
        Status { game, state, done: 0, total: 0, error: None }
    }
}

pub struct Job {
    pub url: String,
    pub hash: String,
    pub size: u64,
    pub dest: PathBuf,
}

/// Downloads every job, checking its hash; returns the first error, which stops the rest.
pub async fn fetch_all(jobs: Vec<Job>, progress: Arc<dyn Fn(u64) + Send + Sync>) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("pob-redux/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let permits = Arc::new(tokio::sync::Semaphore::new(PARALLEL));
    let done = Arc::new(AtomicU64::new(0));
    let mut tasks = tokio::task::JoinSet::new();
    for job in jobs {
        let (client, permits, done, progress) = (client.clone(), permits.clone(), done.clone(), progress.clone());
        tasks.spawn(async move {
            let _permit = permits.acquire_owned().await.map_err(|e| e.to_string())?;
            let bytes = match fetch(&client, &job.url, &job.hash).await {
                Ok(bytes) => bytes,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    fetch(&client, &job.url, &job.hash).await?
                }
            };
            if !crate::art::write_atomic(&job.dest, &bytes) {
                return Err(format!("could not write {}", job.dest.display()));
            }
            progress(done.fetch_add(job.size, Ordering::Relaxed) + job.size);
            Ok::<(), String>(())
        });
    }
    // One failed file stops the rest, so an offline start gives up quickly.
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
    error.map_or(Ok(()), Err)
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

/// Reports progress about once per percent.
pub fn throttled(total: u64, report: impl Fn(u64) + Send + Sync + 'static) -> Arc<dyn Fn(u64) + Send + Sync> {
    let last = AtomicU64::new(0);
    Arc::new(move |done| {
        if done - last.load(Ordering::Relaxed) >= total / 100 || done == total {
            last.store(done, Ordering::Relaxed);
            report(done);
        }
    })
}

/// The tree art first, so the tree opens sooner, then the game data.
pub fn start(app: &AppHandle, game: Game, pob_root: PathBuf) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::tree_art::ensure(&app, game, &pob_root).await;
        crate::game_data::ensure(&app, game, &pob_root).await;
    });
}

pub fn present(path: &Path, size: u64) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.len() == size)
}

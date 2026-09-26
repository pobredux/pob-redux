//! Providers, keys, and the request proxy for the in-app chat panel.
//!
//! Claude and Codex run through the user's own CLI (`agent`). Local Ollama is
//! the one HTTP provider: the webview asks for a stream by provider id and
//! path, and this module makes the request from Rust and pushes the response
//! back in chunks.
//!
//! The credential store here now only holds the decision backends' keys. They
//! live in the OS credential store and are never sent to the webview. On
//! Windows, a key that Credential Manager refuses goes to a DPAPI-encrypted
//! file instead (`key_file`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use crate::agent::{self, AgentStatus, Cli, CLIS};

const SERVICE: &str = "dev.pobredux.desktop";

#[derive(Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ApiKind {
    /// The user's own CLI runs the conversation.
    Agent,
    /// `/chat/completions` and `/models`, run by the panel's own loop.
    OpenAiCompatible,
}

pub struct Provider {
    pub id: &'static str,
    pub label: &'static str,
    pub default_base: &'static str,
}

pub const PROVIDERS: &[Provider] = &[Provider {
    id: "ollama-local",
    label: "Ollama (Local)",
    default_base: "http://localhost:11434/v1",
}];

fn find(id: &str) -> Result<&'static Provider, String> {
    PROVIDERS.iter().find(|p| p.id == id).ok_or_else(|| format!("unknown provider {id}"))
}

/// Base URL overrides, for Ollama on another machine. Stored app-side (not
/// secret); keys are kept apart from it.
#[derive(Default)]
pub struct AiState {
    bases: Mutex<HashMap<String, String>>,
}

impl AiState {
    pub fn new(app: &AppHandle) -> Self {
        Self { bases: Mutex::new(read_bases(app).unwrap_or_default()) }
    }
}

fn bases_path(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_config_dir().ok()?.join("providers.json"))
}

fn read_bases(app: &AppHandle) -> Option<HashMap<String, String>> {
    serde_json::from_str(&std::fs::read_to_string(bases_path(app)?).ok()?).ok()
}

fn write_bases(app: &AppHandle, bases: &HashMap<String, String>) -> Result<(), String> {
    let path = bases_path(app).ok_or("no config dir")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(bases).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn base_url(app: &AppHandle, p: &Provider) -> String {
    app.state::<AiState>()
        .bases
        .lock()
        .unwrap()
        .get(p.id)
        .cloned()
        .unwrap_or_else(|| p.default_base.to_string())
}

fn known(id: &str) -> Result<(), String> {
    if crate::decide::BACKENDS.iter().any(|b| b.key_id == id) {
        Ok(())
    } else {
        Err(format!("unknown key slot {id}"))
    }
}

fn entry(id: &str) -> Result<keyring::Entry, String> {
    known(id)?;
    keyring::Entry::new(SERVICE, id).map_err(|e| e.to_string())
}

/// The fallback file wins: it only exists when the store refused the newer key,
/// so any copy still in the store is older.
pub(crate) fn stored_key(app: &AppHandle, id: &str) -> Option<String> {
    key_file::read(app, id).or_else(|| entry(id).ok().and_then(|e| e.get_password().ok()))
}

pub(crate) fn stored_keys(app: &AppHandle) -> Vec<String> {
    let mut ids: Vec<&str> = crate::decide::BACKENDS.iter().map(|b| b.key_id).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter().filter_map(|id| stored_key(app, id)).collect()
}

/// Last four characters, so the user can tell which key is stored.
pub(crate) fn hint(key: &str) -> String {
    key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect()
}

/// Windows Credential Manager can refuse a write outright (error 8 when its
/// store is full). DPAPI encrypts for the same Windows account that Credential
/// Manager does, without the store's limits.
#[cfg(windows)]
mod key_file {
    use std::path::PathBuf;

    use tauri::{AppHandle, Manager};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};

    fn path(app: &AppHandle, id: &str) -> Option<PathBuf> {
        Some(app.path().app_config_dir().ok()?.join("keys").join(format!("{id}.dpapi")))
    }

    pub fn read(app: &AppHandle, id: &str) -> Option<String> {
        let data = std::fs::read(path(app, id)?).ok()?;
        String::from_utf8(dpapi(&data, false).ok()?).ok()
    }

    pub fn write(app: &AppHandle, id: &str, key: &str) -> Result<(), String> {
        let path = path(app, id).ok_or("no config dir")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let data = dpapi(key.as_bytes(), true).map_err(|e| format!("encrypting the key: {e}"))?;
        std::fs::write(&path, data).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn remove(app: &AppHandle, id: &str) {
        if let Some(path) = path(app, id) {
            let _ = std::fs::remove_file(path);
        }
    }

    pub(super) fn dpapi(data: &[u8], protect: bool) -> std::io::Result<Vec<u8>> {
        let blob = |d: &[u8]| CRYPT_INTEGER_BLOB { cbData: d.len() as u32, pbData: d.as_ptr() as *mut u8 };
        let input = blob(data);
        let entropy = blob(super::SERVICE.as_bytes());
        let mut out = CRYPT_INTEGER_BLOB::default();
        // SAFETY: `input` and `entropy` point into slices that outlive the call,
        // and neither function writes through them. On success Windows allocates
        // `out`, which is copied and then released with LocalFree.
        unsafe {
            let ok = if protect {
                CryptProtectData(&input, std::ptr::null(), &entropy, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            } else {
                CryptUnprotectData(&input, std::ptr::null_mut(), &entropy, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
            };
            if ok == 0 {
                return Err(std::io::Error::last_os_error());
            }
            let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
            LocalFree(out.pbData.cast());
            Ok(bytes)
        }
    }
}

#[cfg(not(windows))]
mod key_file {
    use tauri::AppHandle;

    pub fn read(_: &AppHandle, _: &str) -> Option<String> {
        None
    }

    pub fn remove(_: &AppHandle, _: &str) {}
}

#[derive(Serialize)]
pub struct ProviderStatus {
    id: &'static str,
    label: &'static str,
    kind: ApiKind,
    /// HTTP providers only.
    base_url: Option<String>,
    default_base: Option<&'static str>,
    /// CLI providers only.
    agent: Option<AgentStatus>,
    login: Option<&'static str>,
    install: Option<&'static str>,
    /// Bytes the app downloads itself, for an agent it installs; such an agent also signs in from the app.
    download: Option<u64>,
    /// The vendor's install command, run in a terminal from the settings page.
    install_command: Option<&'static str>,
    /// Usable now, as far as the app can tell.
    ready: bool,
}

/// The CLIs first, then Ollama. `refresh` checks the CLIs again instead of
/// using what was found at startup.
#[tauri::command]
pub async fn ai_providers(app: AppHandle, refresh: Option<bool>, only: Option<String>) -> Vec<ProviderStatus> {
    let found = agent::statuses(&app, refresh.unwrap_or(false), only.as_deref().and_then(Cli::from_id)).await;
    let agents = CLIS.into_iter().map(|cli: Cli| {
        let status = found.get(&cli).cloned().unwrap_or_default();
        ProviderStatus {
            id: cli.id(),
            label: cli.label(),
            kind: ApiKind::Agent,
            base_url: None,
            default_base: None,
            ready: status.installed && status.error.is_none() && status.signed_in != Some(false),
            agent: Some(status),
            login: cli.login(),
            install: Some(cli.install()),
            download: cli.download(),
            install_command: cli.install_command(),
        }
    });
    let http = PROVIDERS.iter().map(|p| ProviderStatus {
        id: p.id,
        label: p.label,
        kind: ApiKind::OpenAiCompatible,
        base_url: Some(base_url(&app, p)),
        default_base: Some(p.default_base),
        agent: None,
        login: None,
        install: None,
        download: None,
        install_command: None,
        ready: true,
    });
    agents.chain(http).collect()
}

#[tauri::command]
pub fn ai_key_set(app: AppHandle, provider: String, key: String) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("key is empty".into());
    }
    let entry = entry(&provider)?;
    let err = match entry.set_password(key) {
        Ok(()) => {
            key_file::remove(&app, &provider);
            return Ok(());
        }
        Err(e) => e,
    };
    #[cfg(windows)]
    if matches!(err, keyring::Error::PlatformFailure(_) | keyring::Error::NoStorageAccess(_)) {
        let reason = keyring_error(err);
        key_file::write(&app, &provider, key).map_err(|e| format!("{reason} Saving it to an encrypted file instead also failed: {e}"))?;
        log::warn!("ai: Credential Manager refused the {provider} key ({reason}); saved it to an encrypted file instead");
        let _ = entry.delete_credential();
        return Ok(());
    }
    Err(keyring_error(err))
}

/// keyring's own errors leave out the fix: on Linux, that a D-Bus Secret
/// Service has to be running; on Windows, what error 8 means.
fn keyring_error(e: keyring::Error) -> String {
    if cfg!(target_os = "linux") {
        return format!("{e}. Storing a key needs a Secret Service (gnome-keyring or KWallet) on the session bus.");
    }
    #[cfg(windows)]
    if let keyring::Error::PlatformFailure(inner) = &e {
        if inner.downcast_ref::<keyring::windows::Error>().is_some_and(|w| w.0 == 8) {
            return format!(
                "{e}. Windows Credential Manager refused to save the key, which usually means its store is full. \
                 Remove entries you no longer need under Credential Manager > Windows Credentials > Generic Credentials, then try again."
            );
        }
    }
    format!("{e}.")
}

#[tauri::command]
pub fn ai_key_clear(app: AppHandle, provider: String) -> Result<(), String> {
    known(&provider)?;
    key_file::remove(&app, &provider);
    match entry(&provider)?.delete_credential() {
        Ok(()) => Ok(()),
        // Clearing a key that was never stored is a success from the UI's side.
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Override a provider's base URL. An empty string resets it to the default.
#[tauri::command]
pub fn ai_base_set(app: AppHandle, provider: String, base_url: String) -> Result<(), String> {
    let p = find(&provider)?;
    let url = base_url.trim().trim_end_matches('/').to_string();
    if !url.is_empty() && !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("base URL must start with http:// or https://".into());
    }
    let state = app.state::<AiState>();
    let snapshot = {
        let mut bases = state.bases.lock().unwrap();
        if url.is_empty() || url == p.default_base {
            bases.remove(p.id);
        } else {
            bases.insert(p.id.to_string(), url);
        }
        bases.clone()
    };
    write_bases(&app, &snapshot)
}

#[derive(Serialize, Clone)]
pub struct ModelInfo {
    pub(crate) id: String,
    pub(crate) label: String,
    /// The effort levels this model takes; empty hides the selector.
    pub(crate) efforts: Vec<String>,
    /// Current-generation, or one generation back. The UI groups these first.
    pub(crate) recommended: bool,
    /// Offers a faster service tier (Claude fast mode, Codex's fast tier, Cursor's fast option).
    pub(crate) fast: bool,
}

#[derive(Deserialize)]
struct ModelList {
    data: Vec<RawModel>,
}

#[derive(Deserialize)]
struct RawModel {
    id: String,
    #[serde(default)]
    created: Option<i64>,
}

/// Endpoints that are not chat models, such as embeddings.
fn is_chat_model(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    const EXCLUDE: &[&str] = &["embed", "whisper", "tts", "rerank", "guard", "clip", "bge-", "nomic"];
    !EXCLUDE.iter().any(|bad| l.contains(bad))
}

/// Widely used current open models, grouped at the top of the picker.
fn is_recommended(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    const CURRENT: &[&str] = &[
        "llama-4", "llama3.3", "deepseek-v3", "deepseek-r1", "qwen3", "gemma4", "granite4", "mistral-large", "kimi-k2", "glm-4", "gpt-oss",
    ];
    CURRENT.iter().any(|c| l.contains(c))
}

/// Reasoning models take an effort setting. Matched on id because Ollama does
/// not advertise the capability in its model list.
fn effort_capable(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    l.contains("deepseek-r") || l.contains("qwq") || l.contains("thinking") || l.contains("gpt-oss")
}

/// Load a local Ollama model into memory ahead of the first message and keep
/// it there for a while. Ollama loads on first use, which for a 6 GB model is
/// tens of seconds the user would otherwise wait on their first question.
/// Returns the load time in milliseconds.
#[tauri::command]
pub async fn ai_warm_model(app: AppHandle, provider: String, model: String) -> Result<u64, String> {
    let p = find(&provider)?;
    // The native API lives beside the OpenAI-compatible one, without /v1.
    let base = base_url(&app, p);
    let root = base.trim_end_matches('/').trim_end_matches("/v1").to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;
    let t0 = std::time::Instant::now();
    let res = client
        .post(format!("{root}/api/generate"))
        .header("content-type", "application/json")
        .body(serde_json::json!({ "model": model, "keep_alive": "30m" }).to_string())
        .send()
        .await
        .map_err(|e| format!("could not reach Ollama: {e}"))?;
    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(format!("Ollama could not load {model}: {}", body.chars().take(200).collect::<String>()));
    }
    Ok(t0.elapsed().as_millis() as u64)
}

#[tauri::command]
pub async fn ai_models(app: AppHandle, provider: String) -> Result<Vec<ModelInfo>, String> {
    if let Some(cli) = Cli::from_id(&provider) {
        return agent::models(&app, cli).await;
    }
    let p = find(&provider)?;
    let base = base_url(&app, p);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let res = client.get(format!("{base}/models")).send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() {
        return Err(format!("{} returned {}", p.label, res.status()));
    }
    let text = res.text().await.map_err(|e| e.to_string())?;
    let list: ModelList =
        serde_json::from_str(&text).map_err(|e| format!("unexpected model list from {}: {e}", p.label))?;

    let mut raw: Vec<RawModel> = list.data.into_iter().filter(|m| is_chat_model(&m.id)).collect();
    raw.sort_by(|a, b| b.created.unwrap_or(0).cmp(&a.created.unwrap_or(0)).then_with(|| a.id.cmp(&b.id)));
    Ok(raw
        .into_iter()
        .map(|m| ModelInfo {
            efforts: if effort_capable(&m.id) { ["none", "low", "medium", "high"].map(String::from).to_vec() } else { Vec::new() },
            recommended: is_recommended(&m.id),
            fast: false,
            label: m.id.clone(),
            id: m.id,
        })
        .collect())
}

/// One event in a streamed response. The webview reassembles these into a
/// `Response` so the AI SDK sees an ordinary `fetch`.
#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StreamEvent {
    Head { status: u16 },
    /// A run of response body text, split on UTF-8 boundaries rather than on
    /// SSE events: the client buffers and splits on a blank line itself.
    Chunk { text: String },
    Done,
    Error { message: String },
}

/// Request headers the webview may set. Anything else is dropped so the panel
/// cannot rewrite the host.
const FORWARDABLE: &[&str] = &["content-type", "accept"];

/// Make a request to `provider` at `path` and stream the response body back
/// over `on_event`.
///
/// `path` is joined to the provider's configured base URL. The webview never
/// supplies a host, so it cannot aim a request at a server of its choosing.
#[tauri::command]
pub async fn ai_chat_stream(
    app: AppHandle,
    provider: String,
    path: String,
    body: String,
    headers: Vec<(String, String)>,
    on_event: Channel<StreamEvent>,
) -> Result<(), String> {
    let p = find(&provider)?;
    if !path.starts_with('/') || path.starts_with("//") || path.contains("://") || path.contains("..") {
        return Err(format!("bad path {path}"));
    }
    let base = base_url(&app, p);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;

    // The base and the caller's path are concatenated, so exactly one of them
    // carries the version segment. Collapse it if both do: the request would
    // otherwise 404 with nothing to say why.
    let url = format!("{base}{path}").replace("/v1/v1/", "/v1/");
    let mut req = client.post(url).body(body);
    for (name, value) in headers {
        if FORWARDABLE.contains(&name.to_ascii_lowercase().as_str()) {
            req = req.header(name, value);
        }
    }

    let mut res = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            let _ = on_event.send(StreamEvent::Error { message: e.to_string() });
            return Ok(());
        }
    };

    on_event
        .send(StreamEvent::Head { status: res.status().as_u16() })
        .map_err(|e| e.to_string())?;

    // Chunk boundaries can land mid-character, so hold the tail until it completes.
    let mut pending: Vec<u8> = Vec::new();
    loop {
        match res.chunk().await {
            Ok(Some(bytes)) => {
                pending.extend_from_slice(&bytes);
                let valid = match std::str::from_utf8(&pending) {
                    Ok(s) => s.len(),
                    Err(e) => e.valid_up_to(),
                };
                if valid > 0 {
                    let text = String::from_utf8_lossy(&pending[..valid]).into_owned();
                    pending.drain(..valid);
                    if on_event.send(StreamEvent::Chunk { text }).is_err() {
                        return Ok(()); // receiver dropped: the user navigated away
                    }
                }
            }
            Ok(None) => break,
            Err(e) => {
                let _ = on_event.send(StreamEvent::Error { message: e.to_string() });
                return Ok(());
            }
        }
    }
    let _ = on_event.send(StreamEvent::Done);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{effort_capable, find, PROVIDERS};

    #[test]
    fn only_known_providers_resolve() {
        assert!(find("ollama-local").is_ok());
        assert!(find("anthropic").is_err());
        assert!(find("evil.example.com").is_err());
        assert!(find("").is_err());
    }

    #[test]
    fn every_provider_has_an_absolute_default_base() {
        for p in PROVIDERS {
            assert!(
                p.default_base.starts_with("http://") || p.default_base.starts_with("https://"),
                "{} has a relative base",
                p.id
            );
            assert!(!p.default_base.ends_with('/'), "{} base has a trailing slash", p.id);
        }
    }

    #[test]
    fn key_slots_are_the_decision_backends_only() {
        assert!(super::known("typesafe").is_ok());
        assert!(super::known("openrouter").is_ok());
        assert!(super::known("anthropic").is_err());
    }

    #[test]
    fn non_chat_endpoints_are_filtered_out() {
        for id in ["nomic-embed-text", "bge-m3", "llama-guard3"] {
            assert!(!super::is_chat_model(id), "{id} should be filtered");
        }
        for id in ["qwen3-vl:8b-instruct", "llama3.3:70b", "deepseek-v3"] {
            assert!(super::is_chat_model(id), "{id} should be kept");
        }
    }

    #[test]
    fn a_doubled_version_segment_collapses() {
        let join = |base: &str, path: &str| format!("{base}{path}").replace("/v1/v1/", "/v1/");
        assert_eq!(
            join("http://localhost:11434/v1", "/v1/chat/completions"),
            "http://localhost:11434/v1/chat/completions"
        );
        assert_eq!(
            join("http://localhost:11434/v1", "/chat/completions"),
            "http://localhost:11434/v1/chat/completions"
        );
    }

    #[test]
    fn effort_matches_reasoning_models_only() {
        assert!(effort_capable("deepseek-r1:70b"));
        assert!(effort_capable("gpt-oss:20b"));
        assert!(!effort_capable("llama3.2"));
        assert!(!effort_capable("qwen3-vl:8b-instruct"));
    }

    /// The OS credential store is reachable and round-trips. Uses a throwaway
    /// service name so it can never touch a key the user actually stored.
    #[test]
    fn os_store_round_trips() {
        // A headless CI runner has no D-Bus secret service for keyring to reach,
        // so there is nothing to round-trip through. Still runs on a real desktop.
        if cfg!(target_os = "linux") && std::env::var_os("CI").is_some() {
            return;
        }
        let e = keyring::Entry::new("dev.pobredux.desktop.selftest", "probe").expect("open entry");
        let _ = e.delete_credential();
        e.set_password("secret-value-1234").expect("set");
        assert_eq!(e.get_password().expect("get"), "secret-value-1234");
        e.delete_credential().expect("delete");
        assert!(matches!(e.get_password(), Err(keyring::Error::NoEntry)));
    }

    #[cfg(windows)]
    #[test]
    fn key_file_encryption_round_trips() {
        let sealed = super::key_file::dpapi(b"sk-or-v1-secret", true).expect("protect");
        assert!(!sealed.windows(15).any(|w| w == b"sk-or-v1-secret"));
        assert_eq!(super::key_file::dpapi(&sealed, false).expect("unprotect"), b"sk-or-v1-secret");
    }

    #[cfg(windows)]
    #[test]
    fn full_credential_store_gets_advice() {
        let e = keyring::Error::PlatformFailure(Box::new(keyring::windows::Error(8)));
        assert!(super::keyring_error(e).contains("Generic Credentials"));
        let e = keyring::Error::PlatformFailure(Box::new(keyring::windows::Error(5)));
        assert_eq!(super::keyring_error(e), "Platform secure storage failure: Windows error code 5.");
    }
}

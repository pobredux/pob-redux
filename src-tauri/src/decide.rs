//! Decision models that speak TypeSafe's System One API (Jev, or a Kev server), for the experimental features.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::ai;

pub struct Backend {
    pub id: &'static str,
    /// Credential slot. OpenRouter's keeps the id the old chat provider used, so a stored key still works.
    pub key_id: &'static str,
    pub label: &'static str,
    pub default_base: &'static str,
    pub default_model: &'static str,
    pub needs_key: bool,
    pub keys_url: Option<&'static str>,
}

pub const BACKENDS: &[Backend] = &[
    Backend {
        id: "typesafe",
        key_id: "typesafe",
        label: "TypeSafe Jev",
        default_base: "https://api.typesafe.ai",
        default_model: "jev-latest",
        needs_key: true,
        keys_url: Some("https://console.typesafe.ai/keys"),
    },
    Backend {
        id: "openrouter",
        key_id: "openrouter",
        label: "OpenRouter",
        default_base: "https://openrouter.ai/api",
        default_model: "jaredpalmer/kev-4b",
        needs_key: true,
        keys_url: Some("https://openrouter.ai/keys"),
    },
    Backend {
        id: "kev",
        key_id: "kev",
        label: "Kev (local)",
        default_base: "http://127.0.0.1:8009",
        default_model: "kev-latest",
        needs_key: false,
        keys_url: None,
    },
];

fn find(id: &str) -> Result<&'static Backend, String> {
    BACKENDS.iter().find(|b| b.id == id).ok_or_else(|| format!("unknown decision backend {id}"))
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct Config {
    #[serde(default)]
    backend: Option<String>,
    #[serde(default)]
    bases: HashMap<String, String>,
    #[serde(default)]
    models: HashMap<String, String>,
}

impl Config {
    fn active(&self) -> &'static Backend {
        self.backend.as_deref().and_then(|id| find(id).ok()).unwrap_or(&BACKENDS[0])
    }
    fn base(&self, b: &Backend) -> String {
        self.bases.get(b.id).cloned().unwrap_or_else(|| b.default_base.to_string())
    }
    fn model(&self, b: &Backend) -> String {
        self.models.get(b.id).cloned().unwrap_or_else(|| b.default_model.to_string())
    }
}

pub struct DecideState {
    config: Mutex<Config>,
}

impl DecideState {
    pub fn new(app: &AppHandle) -> Self {
        Self { config: Mutex::new(read_config(app).unwrap_or_default()) }
    }
}

fn config_path(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_config_dir().ok()?.join("decide.json"))
}

fn read_config(app: &AppHandle) -> Option<Config> {
    serde_json::from_str(&std::fs::read_to_string(config_path(app)?).ok()?).ok()
}

fn update(app: &AppHandle, f: impl FnOnce(&mut Config)) -> Result<(), String> {
    let snapshot = {
        let state = app.state::<DecideState>();
        let mut c = state.config.lock().unwrap();
        f(&mut c);
        c.clone()
    };
    let path = config_path(app).ok_or("no config dir")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn set_or_reset(map: &mut HashMap<String, String>, id: &str, value: String, default: &str) {
    if value.is_empty() || value == default {
        map.remove(id);
    } else {
        map.insert(id.to_string(), value);
    }
}

fn endpoint(base: &str) -> String {
    format!("{}/v1/systemone", base.trim_end_matches('/')).replace("/v1/v1/", "/v1/")
}

#[derive(Serialize)]
pub struct BackendStatus {
    id: &'static str,
    key_id: &'static str,
    label: &'static str,
    needs_key: bool,
    keys_url: Option<&'static str>,
    base_url: String,
    default_base: &'static str,
    model: String,
    default_model: &'static str,
    has_key: bool,
    hint: Option<String>,
    ready: bool,
}

#[derive(Serialize)]
pub struct DecideStatus {
    backend: &'static str,
    backends: Vec<BackendStatus>,
}

#[tauri::command]
pub fn decide_status(app: AppHandle) -> DecideStatus {
    let c = app.state::<DecideState>().config.lock().unwrap().clone();
    DecideStatus {
        backend: c.active().id,
        backends: BACKENDS
            .iter()
            .map(|b| {
                let key = ai::stored_key(&app, b.key_id);
                BackendStatus {
                    id: b.id,
                    key_id: b.key_id,
                    label: b.label,
                    needs_key: b.needs_key,
                    keys_url: b.keys_url,
                    base_url: c.base(b),
                    default_base: b.default_base,
                    model: c.model(b),
                    default_model: b.default_model,
                    has_key: key.is_some(),
                    hint: key.as_deref().map(ai::hint),
                    ready: !b.needs_key || key.is_some(),
                }
            })
            .collect(),
    }
}

#[tauri::command]
pub fn decide_select(app: AppHandle, backend: String) -> Result<(), String> {
    let b = find(&backend)?;
    update(&app, |c| c.backend = Some(b.id.to_string()))
}

/// An empty address or model resets it to the backend's default.
#[tauri::command]
pub fn decide_configure(app: AppHandle, backend: String, base_url: String, model: String) -> Result<(), String> {
    let b = find(&backend)?;
    let url = base_url.trim().trim_end_matches('/').to_string();
    if !url.is_empty() && !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("the address must start with http:// or https://".into());
    }
    let model = model.trim().to_string();
    update(&app, |c| {
        set_or_reset(&mut c.bases, b.id, url, b.default_base);
        set_or_reset(&mut c.models, b.id, model, b.default_model);
    })
}

/// The webview supplies state and questions; host, model and key never leave Rust.
#[tauri::command]
pub async fn decide_ask(app: AppHandle, state: Value, questions: Value, timeout_ms: Option<u64>) -> Result<Value, String> {
    let (b, base, model) = {
        let c = app.state::<DecideState>().config.lock().unwrap().clone();
        let b = c.active();
        (b, c.base(b), c.model(b))
    };
    let key = ai::stored_key(&app, b.key_id);
    if b.needs_key && key.is_none() {
        return Err(format!("no {} key stored", b.label));
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms.unwrap_or(15_000).clamp(500, 60_000)))
        .build()
        .map_err(|e| e.to_string())?;
    let body = serde_json::json!({ "state": state, "model": model, "questions": questions });
    let mut req = client.post(endpoint(&base)).header("content-type", "application/json").body(body.to_string());
    if let Some(k) = key {
        req = req.header("authorization", format!("Bearer {k}"));
    }
    let t0 = Instant::now();
    let res = req.send().await.map_err(|e| {
        if e.is_timeout() {
            format!("{} did not answer in time.", b.label)
        } else {
            format!("Could not reach {} at {base}: {e}", b.label)
        }
    })?;
    let status = res.status().as_u16();
    let text = res.text().await.map_err(|e| e.to_string())?;
    if !(200..300).contains(&status) {
        return Err(explain(b, status, &text));
    }
    let mut out: Value =
        serde_json::from_str(&text).map_err(|e| format!("Unexpected answer from {}: {e}", b.label))?;
    if let Some(o) = out.as_object_mut() {
        o.insert("elapsed_ms".into(), (t0.elapsed().as_millis() as u64).into());
        o.insert("backend".into(), b.id.into());
    }
    Ok(out)
}

fn explain(b: &Backend, status: u16, body: &str) -> String {
    let detail: String = body.chars().take(300).collect();
    match status {
        401 | 403 => format!("{} rejected the key.", b.label),
        404 => format!("{} has no System One endpoint at this address.", b.label),
        422 => format!("{} rejected the request: {detail}", b.label),
        429 => format!("{} is rate limiting this key. Wait a moment and try again. {detail}", b.label),
        503 | 529 => format!("{} is overloaded. Try again shortly. {detail}", b.label),
        _ => format!("{} returned {status}: {detail}", b.label),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backends_have_absolute_bases_and_their_own_key_ids() {
        for b in BACKENDS {
            assert!(b.default_base.starts_with("http://") || b.default_base.starts_with("https://"), "{}", b.id);
            assert!(!b.default_base.ends_with('/'), "{}", b.id);
            assert_eq!(b.key_id, b.id, "{} key slot", b.id);
        }
    }

    #[test]
    fn openrouter_reaches_its_systemone_route() {
        let b = find("openrouter").unwrap();
        assert_eq!(endpoint(b.default_base), "https://openrouter.ai/api/v1/systemone");
        assert_eq!(endpoint("https://openrouter.ai/api/v1"), "https://openrouter.ai/api/v1/systemone");
    }

    #[test]
    fn endpoint_joins_with_or_without_a_version_segment() {
        assert_eq!(endpoint("https://api.typesafe.ai"), "https://api.typesafe.ai/v1/systemone");
        assert_eq!(endpoint("http://127.0.0.1:8009/"), "http://127.0.0.1:8009/v1/systemone");
        assert_eq!(endpoint("https://x.modal.run/v1"), "https://x.modal.run/v1/systemone");
    }

    #[test]
    fn defaults_are_not_stored() {
        let mut map = HashMap::new();
        set_or_reset(&mut map, "kev", "http://gpu-box:8009".into(), "http://127.0.0.1:8009");
        assert_eq!(map.get("kev").map(String::as_str), Some("http://gpu-box:8009"));
        set_or_reset(&mut map, "kev", "http://127.0.0.1:8009".into(), "http://127.0.0.1:8009");
        assert!(map.is_empty());
    }

    #[test]
    fn unknown_or_missing_backend_falls_back_to_the_first() {
        let c = Config { backend: Some("nope".into()), ..Default::default() };
        assert_eq!(c.active().id, BACKENDS[0].id);
        assert!(find("nope").is_err());
    }
}

//! Decision models that speak TypeSafe's System One API (Jev, Kev, Cloudflare Clef), for the experimental features.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::ai;

#[derive(Serialize)]
pub struct ModelInfo {
    id: &'static str,
    label: &'static str,
    context_tokens: u32,
    input_usd_per_million: f64,
}

const CLOUDFLARE_MODELS: &[ModelInfo] = &[
    ModelInfo { id: "clef", label: "Clef", context_tokens: 65_536, input_usd_per_million: 0.24 },
    ModelInfo { id: "clef-flash", label: "Clef Flash", context_tokens: 24_576, input_usd_per_million: 0.038 },
    ModelInfo { id: "clef-omni", label: "Clef Omni", context_tokens: 64_000, input_usd_per_million: 0.15 },
];

pub struct Backend {
    pub id: &'static str,
    /// Credential slot. OpenRouter's keeps the id the old chat provider used, so a stored key still works.
    pub key_id: &'static str,
    pub label: &'static str,
    pub default_base: &'static str,
    pub default_model: &'static str,
    pub needs_key: bool,
    pub keys_url: Option<&'static str>,
    /// Workers AI puts the account and the model in the path and wraps the answer in `result`.
    pub workers_ai: bool,
    /// A fixed list the user picks from; empty where the model is free text.
    pub models: &'static [ModelInfo],
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
        workers_ai: false,
        models: &[],
    },
    Backend {
        id: "cloudflare",
        key_id: "cloudflare",
        label: "Cloudflare Clef",
        default_base: "https://api.cloudflare.com/client/v4",
        default_model: "clef",
        needs_key: true,
        keys_url: Some("https://dash.cloudflare.com/profile/api-tokens"),
        workers_ai: true,
        models: CLOUDFLARE_MODELS,
    },
    Backend {
        id: "openrouter",
        key_id: "openrouter",
        label: "OpenRouter",
        default_base: "https://openrouter.ai/api",
        default_model: "jaredpalmer/kev-4b",
        needs_key: true,
        keys_url: Some("https://openrouter.ai/keys"),
        workers_ai: false,
        models: &[],
    },
    Backend {
        id: "kev",
        key_id: "kev",
        label: "Kev (local)",
        default_base: "http://127.0.0.1:8009",
        default_model: "kev-latest",
        needs_key: false,
        keys_url: None,
        workers_ai: false,
        models: &[],
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
    #[serde(default)]
    accounts: HashMap<String, String>,
}

impl Config {
    fn active(&self) -> &'static Backend {
        self.backend.as_deref().and_then(|id| find(id).ok()).unwrap_or(&BACKENDS[0])
    }
    fn base(&self, b: &Backend) -> String {
        self.bases.get(b.id).cloned().unwrap_or_else(|| b.default_base.to_string())
    }
    fn model(&self, b: &Backend) -> String {
        let listed = |m: &String| b.models.is_empty() || b.models.iter().any(|known| known.id == m);
        self.models.get(b.id).filter(|m| listed(m)).cloned().unwrap_or_else(|| b.default_model.to_string())
    }
    fn account(&self, b: &Backend) -> Option<String> {
        self.accounts.get(b.id).cloned()
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

fn endpoint(b: &Backend, base: &str, account: Option<&str>, model: &str) -> Result<String, String> {
    let base = base.trim_end_matches('/');
    if !b.workers_ai {
        return Ok(format!("{base}/v1/systemone").replace("/v1/v1/", "/v1/"));
    }
    let account = account.ok_or_else(|| format!("{} needs an account ID.", b.label))?;
    Ok(format!("{base}/accounts/{account}/ai/run/@cf/cloudflare/{model}"))
}

fn path_safe(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_')
}

/// Workers AI answers `{ result, success, errors }`; the System One servers answer the result itself.
fn unwrap_result(mut out: Value) -> Value {
    match out.get_mut("result") {
        Some(inner) if inner.get("answers").is_some() => inner.take(),
        _ => out,
    }
}

#[derive(Serialize)]
pub struct BackendStatus {
    id: &'static str,
    key_id: &'static str,
    label: &'static str,
    needs_key: bool,
    keys_url: Option<&'static str>,
    needs_account: bool,
    account: Option<String>,
    base_url: String,
    default_base: &'static str,
    model: String,
    default_model: &'static str,
    models: &'static [ModelInfo],
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
                let account = c.account(b);
                BackendStatus {
                    id: b.id,
                    key_id: b.key_id,
                    label: b.label,
                    needs_key: b.needs_key,
                    keys_url: b.keys_url,
                    needs_account: b.workers_ai,
                    ready: (!b.needs_key || key.is_some()) && (!b.workers_ai || account.is_some()),
                    account,
                    base_url: c.base(b),
                    default_base: b.default_base,
                    model: c.model(b),
                    default_model: b.default_model,
                    models: b.models,
                    has_key: key.is_some(),
                    hint: key.as_deref().map(ai::hint),
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
pub fn decide_configure(
    app: AppHandle,
    backend: String,
    base_url: String,
    model: String,
    account: Option<String>,
) -> Result<(), String> {
    let b = find(&backend)?;
    let account = account.unwrap_or_default().trim().to_string();
    if b.workers_ai && !account.is_empty() && !path_safe(&account) {
        return Err("the account ID is the 32-character ID from the Cloudflare dashboard".into());
    }
    let url = base_url.trim().trim_end_matches('/').to_string();
    if !url.is_empty() && !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("the address must start with http:// or https://".into());
    }
    let model = model.trim().to_string();
    if !b.models.is_empty() && !model.is_empty() && !b.models.iter().any(|known| known.id == model) {
        return Err(format!("{} has no model {model}", b.label));
    }
    update(&app, |c| {
        set_or_reset(&mut c.bases, b.id, url, b.default_base);
        set_or_reset(&mut c.models, b.id, model, b.default_model);
        if b.workers_ai {
            set_or_reset(&mut c.accounts, b.id, account, "");
        }
    })
}

/// The webview supplies state and questions; host, model and key never leave Rust.
#[tauri::command]
pub async fn decide_ask(app: AppHandle, state: Value, questions: Value, timeout_ms: Option<u64>) -> Result<Value, String> {
    let (b, base, model, account) = {
        let c = app.state::<DecideState>().config.lock().unwrap().clone();
        let b = c.active();
        (b, c.base(b), c.model(b), c.account(b))
    };
    let url = endpoint(b, &base, account.as_deref(), &model)?;
    let key = ai::stored_key(&app, b.key_id);
    if b.needs_key && key.is_none() {
        return Err(format!("no {} key stored", b.label));
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms.unwrap_or(15_000).clamp(500, 60_000)))
        .build()
        .map_err(|e| e.to_string())?;
    let body = serde_json::json!({ "state": state, "model": model, "questions": questions });
    let mut req = client.post(url).header("content-type", "application/json").body(body.to_string());
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
        unwrap_result(serde_json::from_str(&text).map_err(|e| format!("Unexpected answer from {}: {e}", b.label))?);
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
        404 if b.workers_ai => format!("{} did not find this account or model: {detail}", b.label),
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
        assert_eq!(endpoint(b, b.default_base, None, "x").unwrap(), "https://openrouter.ai/api/v1/systemone");
        assert_eq!(endpoint(b, "https://openrouter.ai/api/v1", None, "x").unwrap(), "https://openrouter.ai/api/v1/systemone");
    }

    #[test]
    fn cloudflare_puts_the_account_and_model_in_the_path() {
        let b = find("cloudflare").unwrap();
        assert_eq!(
            endpoint(b, b.default_base, Some("abc123"), "clef-flash").unwrap(),
            "https://api.cloudflare.com/client/v4/accounts/abc123/ai/run/@cf/cloudflare/clef-flash"
        );
        assert!(endpoint(b, b.default_base, None, "clef").is_err());
        assert!(path_safe("clef-flash") && !path_safe("../x") && !path_safe(""));
    }

    #[test]
    fn a_model_outside_the_list_falls_back_to_the_default() {
        let b = find("cloudflare").unwrap();
        assert!(b.models.iter().any(|m| m.id == b.default_model));
        assert!(b.models.iter().all(|m| path_safe(m.id)));
        let mut c = Config::default();
        c.models.insert("cloudflare".into(), "clef-flash".into());
        assert_eq!(c.model(b), "clef-flash");
        c.models.insert("cloudflare".into(), "gone".into());
        assert_eq!(c.model(b), "clef");
    }

    #[test]
    fn workers_ai_envelope_is_unwrapped() {
        let wrapped = serde_json::json!({ "result": { "model": "clef", "answers": { "a": { "type": "noul", "noul": 0.9 } } }, "success": true });
        assert_eq!(unwrap_result(wrapped)["answers"]["a"]["noul"], 0.9);
        let plain = serde_json::json!({ "model": "jev", "answers": {} });
        assert_eq!(unwrap_result(plain.clone()), plain);
    }

    #[test]
    fn endpoint_joins_with_or_without_a_version_segment() {
        let b = find("kev").unwrap();
        assert_eq!(endpoint(b, "https://api.typesafe.ai", None, "x").unwrap(), "https://api.typesafe.ai/v1/systemone");
        assert_eq!(endpoint(b, "http://127.0.0.1:8009/", None, "x").unwrap(), "http://127.0.0.1:8009/v1/systemone");
        assert_eq!(endpoint(b, "https://x.modal.run/v1", None, "x").unwrap(), "https://x.modal.run/v1/systemone");
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

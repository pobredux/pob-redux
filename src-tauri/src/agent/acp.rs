//! Agents that speak the Agent Client Protocol (JSON-RPC 2.0 over stdio):
//! Cursor, Grok, OpenCode and Antigravity.

use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::watch;

use super::rpc::{Conn, Handler};
use super::which::Launch;
use super::{AgentEvent, AgentStatus, Outcome, Session};
use crate::ai::ModelInfo;

const SERVER: &str = "pobredux";
const AUTH_REQUIRED: &str = "Authentication required";

enum Check {
    CursorAbout,
    GrokModels,
    OpenCodeAuth,
    Unknown,
}

/// How one ACP agent is started and signed in.
pub(crate) struct Spec {
    name: &'static str,
    args: &'static [&'static str],
    /// Sent to `authenticate`; the agent's own saved login backs it.
    auth: &'static str,
    /// An API key in this variable switches `authenticate` to the second method.
    key: Option<(&'static str, &'static str)>,
    env: &'static [(&'static str, &'static str)],
    /// The terminal command that signs in; `None` when the app runs the sign-in itself.
    login: Option<&'static str>,
    check: Check,
    /// A vendor request that lists models when `session/new` does not.
    list_models: Option<&'static str>,
}

pub(crate) static CURSOR: Spec = Spec {
    name: "Cursor",
    args: &["acp"],
    auth: "cursor_login",
    key: None,
    env: &[],
    login: Some("cursor-agent login"),
    check: Check::CursorAbout,
    list_models: Some("cursor/list_available_models"),
};

pub(crate) static GROK: Spec = Spec {
    name: "Grok",
    args: &["--permission-mode", "default", "agent", "stdio"],
    auth: "cached_token",
    key: Some(("XAI_API_KEY", "xai.api_key")),
    env: &[("GROK_OAUTH2_REFERRER", "pob-redux")],
    login: Some("grok login"),
    check: Check::GrokModels,
    list_models: None,
};

/// OpenCode allows its own tools unless told otherwise; this leaves only ours.
pub(crate) static OPENCODE: Spec = Spec {
    name: "OpenCode",
    args: &["acp"],
    auth: "opencode-login",
    key: None,
    env: &[("OPENCODE_CONFIG_CONTENT", r#"{"tools":{"*":false,"pobredux_*":true},"permission":{"*":"deny","pobredux_*":"allow"}}"#)],
    login: Some("opencode auth login"),
    check: Check::OpenCodeAuth,
    list_models: None,
};

/// Google's agent, downloaded and run with a private profile by `antigravity.rs`.
pub(crate) static ANTIGRAVITY: Spec = Spec {
    name: "Antigravity",
    args: &[],
    auth: "oauth-personal",
    key: None,
    env: &[],
    login: None,
    check: Check::Unknown,
    list_models: None,
};

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c.flat_map(char::to_lowercase)).collect()).unwrap_or_default()
}

fn strip_ansi(s: &str) -> String {
    let re = regex::Regex::new(r"\x1b\[[0-9;]*[A-Za-z]").expect("valid regex");
    re.replace_all(s, "").into_owned()
}

pub(crate) async fn auth(spec: &Spec, launch: &Launch, status: &mut AgentStatus) {
    let run = |args: &'static [&'static str]| launch.output(args, Duration::from_secs(15));
    match spec.check {
        Check::CursorAbout => {
            let Ok((out, err, _)) = run(&["about", "--format", "json"]).await else { return };
            let text = format!("{out}\n{err}");
            if let Ok(v) = serde_json::from_str::<Value>(out.trim()) {
                status.signed_in = Some(v["userEmail"].as_str().is_some_and(|e| !e.is_empty()));
                status.account = v["subscriptionTier"].as_str().filter(|t| !t.is_empty()).map(|t| format!("Cursor {}", title_case(t)));
            } else if text.to_ascii_lowercase().contains("not logged in") || text.to_ascii_lowercase().contains("login required") {
                status.signed_in = Some(false);
            }
        }
        Check::GrokModels => {
            let Ok((out, err, ok)) = run(&["models"]).await else { return };
            let text = strip_ansi(&format!("{out}\n{err}")).to_ascii_lowercase();
            status.signed_in = Some(ok && !text.contains("not logged in") && !text.contains("not authenticated"));
            status.account = Some(if std::env::var_os("XAI_API_KEY").is_some() { "xAI API key" } else { "Grok account" }.into());
        }
        Check::OpenCodeAuth => {
            let Ok((out, err, _)) = run(&["auth", "list"]).await else { return };
            let (total, names) = opencode_accounts(&strip_ansi(&format!("{out}\n{err}")));
            status.signed_in = Some(total > 0);
            status.account = (!names.is_empty()).then(|| names.join(", "));
        }
        Check::Unknown => {}
    }
}

/// Stored credentials plus provider keys found in the environment, and their provider names.
fn opencode_accounts(text: &str) -> (u32, Vec<String>) {
    let count = |word: &str| {
        regex::Regex::new(&format!(r"(\d+)\s+{word}")).ok().and_then(|re| re.captures(text).and_then(|c| c[1].parse::<u32>().ok())).unwrap_or(0)
    };
    let names = text
        .lines()
        .filter_map(|l| l.trim_start_matches(['│', ' ']).strip_prefix('●'))
        .filter_map(|l| l.split_whitespace().next().map(str::to_string))
        .collect();
    (count("credential") + count("environment variable"), names)
}

fn option_id(options: &[Value], kinds: [&str; 2], fallback: &str) -> Value {
    kinds.iter().find_map(|k| options.iter().find(|o| o["kind"] == *k)).map(|o| o["optionId"].clone()).unwrap_or_else(|| json!(fallback))
}

/// Only our own tools run; anything else the agent wants permission for is refused.
fn handler(allowed: Vec<String>) -> Handler {
    Box::new(move |method, params| {
        let method = method.trim_start_matches('_');
        match method {
            "session/request_permission" => {
                let call = &params["toolCall"];
                let text = format!("{} {} {}", call["name"], call["title"], call["rawInput"]["server"]).to_ascii_lowercase();
                let name = call["name"].as_str().or(call["title"].as_str()).unwrap_or("");
                let mcp_marked = call["_meta"]["is_mcp_tool_call"] == true && allowed.iter().any(|t| text.contains(t.as_str()));
                let ours = mcp_marked || text.contains(SERVER) || allowed.iter().any(|t| name == t || name.ends_with(&format!("__{t}")));
                let options = params["options"].as_array().cloned().unwrap_or_default();
                let id = if ours {
                    option_id(&options, ["allow_once", "allow_always"], "allow-once")
                } else {
                    option_id(&options, ["reject_once", "reject_always"], "reject-once")
                };
                json!({ "result": { "outcome": { "outcome": "selected", "optionId": id } } })
            }
            "x.ai/ask_user_question" => json!({ "result": { "outcome": "cancelled" } }),
            "x.ai/exit_plan_mode" => json!({ "result": { "outcome": "abandoned", "feedback": "Plan mode is not available here. Answer directly." } }),
            "cursor/create_plan" => json!({ "result": { "accepted": false } }),
            _ => json!({ "error": { "code": -32601, "message": "Method not found" } }),
        }
    })
}

pub(crate) fn signed_out(spec: &Spec) -> String {
    match spec.login {
        Some(cmd) => format!("{} is not signed in. Run `{cmd}` in a terminal, then check again in the assistant settings.", spec.name),
        None => format!("{} is not signed in. Sign in under Settings > Assistant.", spec.name),
    }
}

/// Run the agent's own sign-in. It opens the browser itself; the link it
/// prints is passed to `on_link` in case the browser did not open.
pub(crate) async fn sign_in(spec: &Spec, launch: &Launch, cwd: &std::path::Path, on_link: impl Fn(String)) -> Result<(), String> {
    let mut cmd = launch.command();
    cmd.current_dir(cwd).args(spec.args);
    let conn = Conn::spawn(cmd, spec.name, true, handler(Vec::new()))?;
    conn.rpc
        .request_for(
            "initialize",
            json!({
                "protocolVersion": 1,
                "clientCapabilities": {
                    "fs": { "readTextFile": false, "writeTextFile": false },
                    "terminal": false,
                    "session": { "configOptions": { "boolean": {} } },
                },
                "clientInfo": { "name": "pob-redux", "title": "PoB Redux", "version": env!("CARGO_PKG_VERSION") },
            }),
            Some(Duration::from_secs(180)),
        )
        .await
        .map_err(|e| conn.explain(e))?;
    let link = regex::Regex::new(r#"https://accounts\.google\.com/o/oauth2/[^\s"'<>]+"#).expect("valid regex");
    let rpc = conn.rpc.clone();
    let mut auth = Box::pin(async move { rpc.request_for("authenticate", json!({ "methodId": spec.auth }), Some(Duration::from_secs(300))).await });
    let mut shown = false;
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    loop {
        tokio::select! {
            out = &mut auth => {
                out.map_err(|e| conn.explain(e))?;
                break;
            }
            _ = tick.tick(), if !shown => {
                if let Some(m) = link.find(&conn.chatter()) {
                    shown = true;
                    on_link(m.as_str().to_string());
                }
            }
        }
    }
    let session = conn
        .rpc
        .request_for("session/new", json!({ "cwd": cwd.to_string_lossy(), "mcpServers": [] }), Some(Duration::from_secs(120)))
        .await
        .map_err(|e| if e.contains(AUTH_REQUIRED) { signed_out(spec) } else { conn.explain(e) })?;
    if let Some(id) = session["sessionId"].as_str() {
        let _ = conn.rpc.request_for("session/close", json!({ "sessionId": id }), Some(Duration::from_secs(5))).await;
    }
    Ok(())
}

/// Start the agent and open a session; the reply also lists its models. Listing
/// models skips `authenticate`, which for some agents opens a browser sign-in.
async fn open(spec: &Spec, launch: &Launch, cwd: &std::path::Path, mcp: Value, allowed: Vec<String>, sign_in: bool) -> Result<(Conn, Value), String> {
    let mut cmd = launch.command();
    cmd.current_dir(cwd).args(spec.args);
    for (k, v) in spec.env {
        cmd.env(k, v);
    }
    let conn = Conn::spawn(cmd, spec.name, true, handler(allowed))?;
    let init = conn
        .rpc
        .request_for(
            "initialize",
            json!({
                "protocolVersion": 1,
                "clientCapabilities": {
                    "fs": { "readTextFile": false, "writeTextFile": false },
                    "terminal": false,
                    "session": { "configOptions": { "boolean": {} } },
                },
                "clientInfo": { "name": "pob-redux", "title": "PoB Redux", "version": env!("CARGO_PKG_VERSION") },
            }),
            Some(Duration::from_secs(180)),
        )
        .await
        .map_err(|e| conn.explain(e))?;
    let has_http = init["agentCapabilities"]["mcpCapabilities"]["http"].as_bool().unwrap_or(false);
    if mcp.as_array().is_some_and(|a| !a.is_empty()) && !has_http {
        return Err(format!("This version of {} cannot reach the build's tools (no HTTP MCP support). Update it and try again.", spec.name));
    }
    if sign_in {
        let method = match spec.key {
            Some((var, method)) if std::env::var_os(var).is_some_and(|v| !v.is_empty()) => method,
            _ => spec.auth,
        };
        let offered = init["authMethods"].as_array().is_some_and(|m| m.iter().any(|a| a["id"] == method));
        if offered {
            conn.rpc.request_for("authenticate", json!({ "methodId": method }), Some(Duration::from_secs(300))).await.map_err(|_| signed_out(spec))?;
        }
    }
    let session = conn
        .rpc
        .request_for("session/new", json!({ "cwd": cwd.to_string_lossy(), "mcpServers": mcp }), Some(Duration::from_secs(120)))
        .await
        .map_err(|e| if e.contains(AUTH_REQUIRED) { signed_out(spec) } else { conn.explain(e) })?;
    Ok((conn, session))
}

struct Choice {
    id: String,
    label: String,
    efforts: Vec<String>,
}

fn efforts_of(list: &Value) -> Vec<String> {
    list.as_array().into_iter().flatten().filter_map(|e| e["value"].as_str().or(e["id"].as_str()).or(e.as_str()).map(str::to_string)).collect()
}

/// The agent's models: a `model` config option, or the older `models` list
/// (whose entries may carry their own reasoning levels, as Grok's do).
fn model_choices(session: &Value) -> (Vec<Choice>, Option<String>) {
    let (effort_levels, _) = effort_option(session);
    if let Some(opt) = session["configOptions"].as_array().and_then(|o| o.iter().find(|c| c["category"] == "model" && c["type"] == "select")) {
        let mut out = Vec::new();
        for o in opt["options"].as_array().into_iter().flatten() {
            for item in o["options"].as_array().cloned().unwrap_or_else(|| vec![o.clone()]) {
                if let Some(v) = item["value"].as_str() {
                    out.push(Choice { id: v.into(), label: item["name"].as_str().unwrap_or(v).into(), efforts: effort_levels.clone() });
                }
            }
        }
        return (out, opt["id"].as_str().map(str::to_string));
    }
    let list = session["models"]["availableModels"].as_array().into_iter().flatten();
    let out = list
        .filter_map(|m| {
            let id = m["modelId"].as_str()?.to_string();
            let own = efforts_of(&m["_meta"]["reasoningEfforts"]);
            Some(Choice { label: m["name"].as_str().unwrap_or(&id).into(), efforts: if own.is_empty() { effort_levels.clone() } else { own }, id })
        })
        .collect();
    (out, None)
}

/// A fast-mode switch among the session's options, as Cursor offers: its id and whether it is a boolean.
fn fast_option(session: &Value) -> Option<(String, bool)> {
    let opt = session["configOptions"].as_array()?.iter().find(|c| {
        let id = c["id"].as_str().unwrap_or("").to_ascii_lowercase();
        let name = c["name"].as_str().unwrap_or("").to_ascii_lowercase();
        id == "fast" || name.contains("fast mode") || (c["category"] == "model_config" && id.contains("fast"))
    })?;
    Some((opt["id"].as_str()?.to_string(), opt["type"] == "boolean"))
}

/// The select value that turns a fast switch on or off.
fn fast_value(session: &Value, id: &str, on: bool) -> Option<String> {
    let opt = session["configOptions"].as_array()?.iter().find(|c| c["id"] == id)?;
    let words: &[&str] = if on { &["fast", "on", "true", "enabled"] } else { &["standard", "normal", "off", "false", "default", "disabled"] };
    opt["options"].as_array()?.iter().filter_map(|o| o["value"].as_str()).find(|v| words.iter().any(|w| v.to_ascii_lowercase().contains(w))).map(str::to_string)
}

fn current_model(session: &Value) -> Option<String> {
    session["configOptions"]
        .as_array()
        .and_then(|o| o.iter().find(|c| c["category"] == "model"))
        .and_then(|c| c["currentValue"].as_str())
        .or(session["models"]["currentModelId"].as_str())
        .map(str::to_string)
}

fn effort_option(session: &Value) -> (Vec<String>, Option<String>) {
    let Some(opt) = session["configOptions"].as_array().and_then(|o| o.iter().find(|c| c["category"] == "thought_level" && c["type"] == "select")) else {
        return (Vec::new(), None);
    };
    (efforts_of(&opt["options"]), opt["id"].as_str().map(str::to_string))
}

pub(crate) async fn models(spec: &Spec, launch: &Launch) -> Result<Vec<ModelInfo>, String> {
    let (conn, session) = open(spec, launch, &std::env::temp_dir(), json!([]), Vec::new(), false).await?;
    let (mut models, _) = model_choices(&session);
    if models.is_empty() {
        if let Some(method) = spec.list_models {
            if let Ok(v) = conn.rpc.request(method, json!({})).await {
                models = v["models"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|m| {
                        let id = m["value"].as_str()?.to_string();
                        Some(Choice { label: m["name"].as_str().unwrap_or(&id).into(), id, efforts: Vec::new() })
                    })
                    .collect();
            }
        }
    }
    if let Some(id) = session["sessionId"].as_str() {
        let _ = conn.rpc.request_for("session/close", json!({ "sessionId": id }), Some(Duration::from_secs(2))).await;
    }
    let current = current_model(&session);
    let fast = fast_option(&session).is_some();
    Ok(models
        .into_iter()
        .map(|c| ModelInfo { recommended: current.as_deref().is_none_or(|m| m == c.id), id: c.id, label: c.label, efforts: c.efforts, fast })
        .collect())
}

pub(crate) struct Proc {
    conn: Conn,
    session: String,
    choices: Value,
    model: Option<String>,
    effort: Option<String>,
    fast: Option<bool>,
    primed: bool,
}

impl Proc {
    pub async fn start(spec: &Spec, s: &Session, launch: &Launch, allowed: Vec<String>) -> Result<Self, String> {
        let mcp = json!([{
            "type": "http",
            "name": SERVER,
            "url": s.mcp_url,
            "headers": [{ "name": "Authorization", "value": format!("Bearer {}", s.mcp_token) }],
        }]);
        let (conn, session) = open(spec, launch, &s.dir, mcp, allowed, true).await?;
        let id = session["sessionId"].as_str().ok_or_else(|| format!("{} did not open a session", spec.name))?.to_string();
        let current = current_model(&session);
        Ok(Self { conn, session: id, choices: session, model: current, effort: None, fast: None, primed: false })
    }

    /// Apply the chosen model and effort where the agent offers them.
    async fn configure(&mut self, s: &Session) {
        let cfg = s.config();
        let (models, model_opt) = model_choices(&self.choices);
        let Some(choice) = models.iter().find(|m| m.id == cfg.model) else { return };
        let effort = cfg.effort.filter(|e| choice.efforts.contains(e));
        let model_changed = self.model.as_deref() != Some(cfg.model.as_str());
        let (_, effort_opt) = effort_option(&self.choices);
        match &model_opt {
            Some(opt) if model_changed => {
                if self.set_option(opt, json!(cfg.model), false).await {
                    self.model = Some(cfg.model.clone());
                }
            }
            None if model_changed || (effort_opt.is_none() && effort.is_some() && effort != self.effort) => {
                let mut params = json!({ "sessionId": self.session, "modelId": cfg.model });
                if effort_opt.is_none() {
                    if let Some(e) = &effort {
                        params["_meta"] = json!({ "reasoningEffort": e });
                    }
                }
                if self.conn.rpc.request("session/set_model", params).await.is_ok() {
                    self.model = Some(cfg.model.clone());
                    if effort_opt.is_none() {
                        self.effort = effort.clone();
                    }
                }
            }
            _ => {}
        }
        if let (Some(e), Some(opt)) = (effort, effort_opt) {
            if self.effort.as_ref() != Some(&e) && self.set_option(&opt, json!(e), false).await {
                self.effort = Some(e);
            }
        }
        if let Some((id, boolean)) = fast_option(&self.choices) {
            if self.fast != Some(cfg.fast) {
                let value = if boolean { Some(json!(cfg.fast)) } else { fast_value(&self.choices, &id, cfg.fast).map(Value::String) };
                if let Some(value) = value {
                    if self.set_option(&id, value, boolean).await {
                        self.fast = Some(cfg.fast);
                    }
                }
            }
        }
    }

    async fn set_option(&mut self, id: &str, value: Value, boolean: bool) -> bool {
        let mut params = json!({ "sessionId": self.session, "configId": id, "value": value });
        if boolean {
            params["type"] = json!("boolean");
        }
        match self.conn.rpc.request("session/set_config_option", params).await {
            Ok(v) => {
                if v["configOptions"].is_array() {
                    self.choices["configOptions"] = v["configOptions"].clone();
                }
                true
            }
            Err(_) => false,
        }
    }

    fn finish(s: &Session, reason: &str, stopping: bool) -> Result<Outcome, String> {
        match reason {
            "cancelled" => Ok(Outcome::Stopped),
            _ if stopping => Ok(Outcome::Stopped),
            "refusal" => Err("The model refused this request.".into()),
            "rate_limit" => Err("Your plan's usage limit is reached.".into()),
            "error" => Err("The agent ended the turn with an error.".into()),
            "max_tokens" => {
                s.gate.emit(AgentEvent::Notice { message: "The reply hit the model's output limit.".into() });
                Ok(Outcome::Done)
            }
            _ => Ok(Outcome::Done),
        }
    }

    pub async fn turn(&mut self, s: &Session, text: &str, mut stop: watch::Receiver<u64>) -> Result<Outcome, String> {
        self.configure(s).await;
        // ACP has no system prompt, so the instructions lead the first message.
        let text = if self.primed { text.to_string() } else { format!("{}\n\n---\n\n{text}", s.config().instructions) };
        self.primed = true;
        stop.borrow_and_update();

        let rpc = self.conn.rpc.clone();
        let params = json!({ "sessionId": self.session, "prompt": [{ "type": "text", "text": text }] });
        let mut prompt = Box::pin(async move { rpc.request_for("session/prompt", params, None).await });
        let mut in_text = false;
        let mut message: Option<String> = None;
        let mut stopping: Option<tokio::time::Instant> = None;
        loop {
            let deadline = stopping.map(|t| t + Duration::from_secs(15));
            tokio::select! {
                out = &mut prompt => {
                    while let Ok(msg) = self.conn.events.try_recv() {
                        self.update(s, &msg, &mut in_text, &mut message);
                    }
                    let out = match out {
                        Ok(v) => v,
                        Err(_) if stopping.is_some() => return Ok(Outcome::Stopped),
                        Err(e) if e.contains("free tier can only be used") => {
                            return Err("OpenCode's free models only work inside OpenCode itself. Pick a model from a provider you signed in to.".into())
                        }
                        Err(e) => return Err(self.conn.explain(e)),
                    };
                    let u = &out["usage"];
                    if u.is_object() {
                        let n = |k: &str| u[k].as_u64().unwrap_or(0);
                        s.gate.emit(AgentEvent::Usage { input: n("inputTokens"), output: n("outputTokens"), cache_read: n("cachedReadTokens"), cache_write: n("cachedWriteTokens") });
                    }
                    return Self::finish(s, out["stopReason"].as_str().unwrap_or("end_turn"), stopping.is_some());
                }
                msg = self.conn.events.recv() => match msg {
                    Some(msg) if msg["method"].as_str().is_some_and(|m| m.trim_start_matches('_') == "x.ai/session/prompt_complete")
                        && msg["params"]["sessionId"].as_str().is_none_or(|id| id == self.session) => {
                        return Self::finish(s, msg["params"]["stopReason"].as_str().unwrap_or("end_turn"), stopping.is_some());
                    }
                    Some(msg) => self.update(s, &msg, &mut in_text, &mut message),
                    None => return Err(self.conn.explain("the agent stopped".into())),
                },
                _ = stop.changed(), if stopping.is_none() => {
                    let _ = self.conn.rpc.notify("session/cancel", Some(json!({ "sessionId": self.session }))).await;
                    stopping = Some(tokio::time::Instant::now());
                }
                _ = async {
                    match deadline {
                        Some(d) => tokio::time::sleep_until(d).await,
                        None => std::future::pending().await,
                    }
                } => return Err("The agent did not stop when asked.".into()),
            }
        }
    }

    fn update(&mut self, s: &Session, msg: &Value, in_text: &mut bool, message: &mut Option<String>) {
        if msg["method"] != "session/update" || msg["params"]["sessionId"].as_str().is_some_and(|id| id != self.session) {
            return;
        }
        let u = &msg["params"]["update"];
        match u["sessionUpdate"].as_str().unwrap_or("") {
            "agent_message_chunk" => {
                let Some(t) = u["content"]["text"].as_str() else { return };
                let id = u["messageId"].as_str().map(str::to_string);
                if !*in_text || (id.is_some() && id != *message) {
                    s.gate.emit(AgentEvent::TextStart);
                }
                *in_text = true;
                *message = id;
                s.gate.emit(AgentEvent::Text { text: t.to_string() });
            }
            "tool_call" | "tool_call_update" => *in_text = false,
            "usage_update" => {
                if let (Some(used), Some(size)) = (u["used"].as_u64(), u["size"].as_u64()) {
                    s.gate.emit(AgentEvent::Context { used, size });
                }
            }
            "config_option_update" => {
                if u["configOptions"].is_array() {
                    self.choices["configOptions"] = u["configOptions"].clone();
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn permission_is_granted_only_for_our_tools() {
        let h = super::handler(vec!["get_stats".into()]);
        let options = json!([
            { "optionId": "a", "kind": "allow_once" },
            { "optionId": "r", "kind": "reject_once" },
        ]);
        let pick = |call: serde_json::Value, options: &serde_json::Value| {
            h("session/request_permission", &json!({ "toolCall": call, "options": options }))["result"]["outcome"]["optionId"].clone()
        };
        assert_eq!(pick(json!({ "title": "get_stats (pobredux MCP Server)" }), &options), "a");
        assert_eq!(pick(json!({ "name": "mcp__pobredux__get_stats" }), &options), "a");
        assert_eq!(pick(json!({ "title": "rm -rf /", "kind": "execute" }), &options), "r");
        assert_eq!(pick(json!({ "title": "Write file" }), &json!([])), "reject-once");
        assert_eq!(h("fs/read_text_file", &json!({}))["error"]["code"], -32601);
        assert_eq!(h("_x.ai/ask_user_question", &json!({}))["result"]["outcome"], "cancelled");
    }

    #[test]
    fn models_come_from_config_options_or_the_legacy_list() {
        let grouped = json!({ "configOptions": [
            { "id": "model", "category": "model", "type": "select", "options": [
                { "group": "g", "name": "G", "options": [{ "value": "m1", "name": "One" }] },
                { "group": "h", "name": "H", "options": [{ "value": "m2", "name": "Two" }] },
            ] },
            { "id": "effort", "category": "thought_level", "type": "select", "options": [{ "value": "low" }, { "value": "high" }] },
        ] });
        let (m, opt) = super::model_choices(&grouped);
        assert_eq!(m.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["m1", "m2"]);
        assert_eq!(m[0].efforts, ["low", "high"]);
        assert_eq!(opt.as_deref(), Some("model"));
        let legacy = json!({ "models": { "availableModels": [
            { "modelId": "grok-4.6", "name": "Grok 4.6", "_meta": { "reasoningEfforts": [{ "value": "low" }, { "id": "high" }] } },
        ] } });
        let (m, opt) = super::model_choices(&legacy);
        assert_eq!(m[0].id, "grok-4.6");
        assert_eq!(m[0].efforts, ["low", "high"]);
        assert!(opt.is_none());
    }

    #[test]
    fn fast_switches_are_found_as_boolean_or_select() {
        let boolean = json!({ "configOptions": [{ "id": "fast", "category": "model_config", "type": "boolean", "currentValue": false }] });
        assert_eq!(super::fast_option(&boolean), Some(("fast".to_string(), true)));
        let select = json!({ "configOptions": [{ "id": "speed", "name": "Fast mode", "type": "select", "options": [{ "value": "standard" }, { "value": "fast" }] }] });
        assert_eq!(super::fast_option(&select), Some(("speed".to_string(), false)));
        assert_eq!(super::fast_value(&select, "speed", true).as_deref(), Some("fast"));
        assert_eq!(super::fast_value(&select, "speed", false).as_deref(), Some("standard"));
        assert_eq!(super::fast_option(&json!({ "configOptions": [{ "id": "model", "category": "model" }] })), None);
    }

    #[test]
    fn opencode_accounts_are_counted() {
        let text = "┌  Credentials ~/auth.json\n│\n└  0 credentials\n\n┌  Environment\n│\n●  Anthropic ANTHROPIC_API_KEY\n│\n└  1 environment variable";
        assert_eq!(super::opencode_accounts(text), (1, vec!["Anthropic".to_string()]));
        assert_eq!(super::opencode_accounts("└  0 credentials").0, 0);
    }
}

//! Claude Code in print mode, exchanging stream-json lines over stdio.

use std::collections::HashSet;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::{mpsc, watch};

use super::which::Launch;
use super::{AgentEvent, AgentStatus, Outcome, Session};
use crate::ai::ModelInfo;

const SERVER: &str = "pobredux";
const TOKEN_ENV: &str = "POB_REDUX_MCP_TOKEN";
/// Set when the app itself was started from a Claude Code session; they would tie the child to that session.
const INHERITED: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "CLAUDE_AGENT_SDK_VERSION",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_SSE_PORT",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_ENABLE_TASKS",
    "CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "MCP_CONNECTION_NONBLOCKING",
];

const EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];
const EFFORTS_NO_XHIGH: &[&str] = &["low", "medium", "high", "max"];
const EFFORTS_TO_HIGH: &[&str] = &["low", "medium", "high"];

struct Model {
    id: &'static str,
    label: &'static str,
    /// The oldest Claude Code that can run it.
    since: Option<[u32; 3]>,
    efforts: &'static [&'static str],
    fast: bool,
}

const MODELS: &[Model] = &[
    Model { id: "claude-opus-5-5", label: "Claude Opus 5.5", since: Some([2, 1, 280]), efforts: EFFORTS, fast: true },
    Model { id: "claude-fable-5-1", label: "Claude Fable 5.1", since: Some([2, 1, 257]), efforts: EFFORTS, fast: false },
    Model { id: "claude-fable-5", label: "Claude Fable 5", since: Some([2, 1, 169]), efforts: EFFORTS, fast: false },
    Model { id: "claude-opus-5", label: "Claude Opus 5", since: Some([2, 1, 219]), efforts: EFFORTS, fast: true },
    Model { id: "claude-opus-4-8", label: "Claude Opus 4.8", since: Some([2, 1, 154]), efforts: EFFORTS, fast: true },
    Model { id: "claude-opus-4-7", label: "Claude Opus 4.7", since: Some([2, 1, 111]), efforts: EFFORTS_NO_XHIGH, fast: true },
    Model { id: "claude-opus-4-6", label: "Claude Opus 4.6", since: None, efforts: EFFORTS_NO_XHIGH, fast: true },
    Model { id: "claude-opus-4-5", label: "Claude Opus 4.5", since: None, efforts: EFFORTS_NO_XHIGH, fast: true },
    Model { id: "claude-sonnet-5", label: "Claude Sonnet 5", since: None, efforts: EFFORTS, fast: false },
    Model { id: "claude-sonnet-4-6", label: "Claude Sonnet 4.6", since: None, efforts: EFFORTS_TO_HIGH, fast: false },
    Model { id: "claude-haiku-4-5", label: "Claude Haiku 4.5", since: None, efforts: &[], fast: false },
];

const DEFAULT_MODEL: &str = "claude-sonnet-5";

/// `2.1.283 (Claude Code)` as numbers.
fn cli_version(s: &str) -> Option<[u32; 3]> {
    let mut parts = s.split_whitespace().next()?.split('.').map(|p| p.parse().ok());
    Some([parts.next()??, parts.next()??, parts.next()??])
}

/// The models the installed Claude Code can run; all of them when its version is unknown.
pub(crate) fn models(installed: Option<&str>) -> Vec<ModelInfo> {
    let installed = installed.and_then(cli_version);
    MODELS
        .iter()
        .filter(|m| m.since.zip(installed).is_none_or(|(since, v)| v >= since))
        .map(|m| ModelInfo {
            id: m.id.into(),
            label: m.label.into(),
            efforts: m.efforts.iter().map(|e| e.to_string()).collect(),
            recommended: m.id == DEFAULT_MODEL,
            fast: m.fast,
        })
        .collect()
}

pub(crate) async fn auth(launch: &Launch, status: &mut AgentStatus) {
    let out = match launch.output(&["auth", "status"], Duration::from_secs(15)).await {
        Ok((out, _, _)) => out,
        Err(e) => {
            status.error = Some(e);
            return;
        }
    };
    let Ok(v) = serde_json::from_str::<Value>(out.trim()) else {
        return;
    };
    status.signed_in = v["loggedIn"].as_bool();
    status.account = match (v["subscriptionType"].as_str(), v["authMethod"].as_str()) {
        (Some(plan), _) if !plan.is_empty() => Some(format!("Claude {}{}", plan[..1].to_uppercase(), &plan[1..])),
        (_, Some(method)) if method.contains("key") => Some("API key".into()),
        (_, Some(method)) => Some(method.to_string()),
        _ => None,
    };
}

pub(crate) struct Proc {
    _child: Child,
    _tree: Option<super::which::Tree>,
    stdin: ChildStdin,
    lines: mpsc::UnboundedReceiver<Value>,
    stderr: Arc<Mutex<String>>,
    /// The CLI reports usage summed over the process, so each turn reports the difference.
    totals: [u64; 4],
    seq: u64,
}

impl Proc {
    pub async fn start(s: &Session, launch: &Launch) -> Result<Self, String> {
        let cfg = s.config();
        let prompt = s.dir.join("system-prompt.md");
        std::fs::write(&prompt, &cfg.instructions).map_err(|e| format!("{}: {e}", prompt.display()))?;
        let mcp = json!({
            "mcpServers": {
                SERVER: {
                    "type": "http",
                    "url": s.mcp_url,
                    "headers": { "Authorization": format!("Bearer ${{{TOKEN_ENV}}}") },
                    "alwaysLoad": true,
                }
            }
        });

        let mut cmd = launch.command();
        for var in INHERITED {
            cmd.env_remove(var);
        }
        // An empty --setting-sources= keeps the user's own CLAUDE.md, hooks and settings out.
        cmd.current_dir(&s.dir)
            .stdin(Stdio::piped())
            .args(["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"])
            .args(["--include-partial-messages", "--tools", "", "--strict-mcp-config", "--disable-slash-commands"])
            .arg("--mcp-config")
            .arg(mcp.to_string())
            .args(["--allowedTools", &format!("mcp__{SERVER}"), "--permission-mode", "dontAsk"])
            .arg("--setting-sources=")
            .arg("--system-prompt-file")
            .arg(&prompt)
            .args(["--model", &cfg.model])
            .env(TOKEN_ENV, &s.mcp_token)
            .env("ENABLE_CLAUDEAI_MCP_SERVERS", "false")
            .env("MCP_TOOL_TIMEOUT", "3600000")
            .env("CLAUDE_CODE_AUTO_CONNECT_IDE", "0")
            .env("CLAUDE_AGENT_SDK_CLIENT_APP", concat!("pob-redux/", env!("CARGO_PKG_VERSION")));
        if let Some(effort) = &cfg.effort {
            cmd.args(["--effort", effort]);
        }
        if cfg.fast && cfg.model == "opus" {
            cmd.arg("--settings").arg(json!({ "fastMode": true }).to_string());
        }
        if let Some(id) = s.resume.lock().unwrap().clone() {
            cmd.args(["--resume", &id]);
        }
        let mut child = cmd.spawn().map_err(|e| format!("could not start Claude Code ({}): {e}", launch.display()))?;
        let tree = super::which::Tree::attach(&child);
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let stderr = super::rpc::drain(child.stderr.take().ok_or("no stderr")?);

        let (tx, lines) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                match serde_json::from_str::<Value>(&line) {
                    Ok(v) => {
                        if tx.send(v).is_err() {
                            break;
                        }
                    }
                    Err(_) => log::debug!("claude: {line}"),
                }
            }
        });
        Ok(Self { _child: child, _tree: tree, stdin, lines, stderr, totals: [0; 4], seq: 0 })
    }

    async fn write(&mut self, line: &Value) -> Result<(), String> {
        let sent = async {
            self.stdin.write_all(format!("{line}\n").as_bytes()).await?;
            self.stdin.flush().await
        };
        match sent.await {
            Ok(()) => Ok(()),
            Err(_) => Err(self.exited()),
        }
    }

    fn usage(&mut self, msg: &Value) -> AgentEvent {
        let mut now = [0u64; 4];
        if let Some(models) = msg["modelUsage"].as_object().filter(|m| !m.is_empty()) {
            for m in models.values() {
                for (i, k) in ["inputTokens", "outputTokens", "cacheReadInputTokens", "cacheCreationInputTokens"].iter().enumerate() {
                    now[i] += m[k].as_u64().unwrap_or(0);
                }
            }
        } else {
            let u = &msg["usage"];
            for (i, k) in ["input_tokens", "output_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"].iter().enumerate() {
                now[i] = self.totals[i] + u[k].as_u64().unwrap_or(0);
            }
        }
        let d: Vec<u64> = (0..4).map(|i| now[i].saturating_sub(self.totals[i])).collect();
        self.totals = now;
        AgentEvent::Usage { input: d[0], output: d[1], cache_read: d[2], cache_write: d[3] }
    }

    fn exited(&self) -> String {
        let tail = self.stderr.lock().unwrap().trim().to_string();
        if tail.is_empty() {
            "Claude Code stopped without saying why.".into()
        } else {
            format!("Claude Code stopped: {tail}")
        }
    }

    pub async fn turn(&mut self, s: &Session, text: &str, mut stop: watch::Receiver<u64>) -> Result<Outcome, String> {
        self.write(&json!({
            "type": "user",
            "message": { "role": "user", "content": [{ "type": "text", "text": text }] },
            "parent_tool_use_id": null,
            "session_id": "",
        }))
        .await?;
        stop.borrow_and_update();

        let mut streamed: HashSet<String> = HashSet::new();
        let mut failure: Option<String> = None;
        let mut stopping: Option<tokio::time::Instant> = None;
        // What the latest request carried in and wrote out: the conversation's current size.
        let (mut ctx_in, mut ctx_out) = (0u64, 0u64);
        loop {
            let deadline = stopping.map(|t| t + Duration::from_secs(10));
            let msg = tokio::select! {
                m = self.lines.recv() => m,
                _ = stop.changed(), if stopping.is_none() => {
                    self.seq += 1;
                    let id = format!("stop-{}", self.seq);
                    self.write(&json!({ "type": "control_request", "request_id": id, "request": { "subtype": "interrupt" } })).await?;
                    stopping = Some(tokio::time::Instant::now());
                    continue;
                }
                _ = async {
                    match deadline {
                        Some(d) => tokio::time::sleep_until(d).await,
                        None => std::future::pending().await,
                    }
                } => return Err("Claude Code did not stop when asked.".into()),
            };
            let Some(msg) = msg else {
                return Err(failure.unwrap_or_else(|| self.exited()));
            };
            if msg["type"] == "control_request" {
                self.refuse(&msg).await?;
                continue;
            }
            if !msg["parent_tool_use_id"].is_null() {
                continue;
            }
            match msg["type"].as_str().unwrap_or("") {
                "system" if msg["subtype"] == "init" => {
                    if let Some(id) = msg["session_id"].as_str() {
                        *s.resume.lock().unwrap() = Some(id.to_string());
                    }
                    let ours = msg["mcp_servers"].as_array().and_then(|a| a.iter().find(|m| m["name"] == SERVER));
                    if let Some(status) = ours.and_then(|m| m["status"].as_str()).filter(|st| *st != "connected") {
                        s.gate.emit(AgentEvent::Notice { message: format!("Claude Code could not reach the build's tools ({status}).") });
                    }
                }
                "stream_event" => {
                    let ev = &msg["event"];
                    match ev["type"].as_str().unwrap_or("") {
                        "message_start" => {
                            if let Some(id) = ev["message"]["id"].as_str() {
                                streamed.insert(id.to_string());
                            }
                            let u = &ev["message"]["usage"];
                            let n = |k: &str| u[k].as_u64().unwrap_or(0);
                            ctx_in = n("input_tokens") + n("cache_creation_input_tokens") + n("cache_read_input_tokens");
                            ctx_out = 0;
                        }
                        "message_delta" => ctx_out = ev["usage"]["output_tokens"].as_u64().unwrap_or(ctx_out),
                        "content_block_start" if ev["content_block"]["type"] == "text" => s.gate.emit(AgentEvent::TextStart),
                        "content_block_delta" if ev["delta"]["type"] == "text_delta" => {
                            if let Some(t) = ev["delta"]["text"].as_str() {
                                s.gate.emit(AgentEvent::Text { text: t.to_string() });
                            }
                        }
                        _ => {}
                    }
                }
                "assistant" => {
                    let message = &msg["message"];
                    let text: String = message["content"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|b| b["type"] == "text")
                        .filter_map(|b| b["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    if let Some(kind) = msg["error"].as_str() {
                        failure = Some(explain(kind, &text));
                        continue;
                    }
                    let seen = message["id"].as_str().is_some_and(|id| streamed.contains(id));
                    if !seen && !text.is_empty() {
                        s.gate.emit(AgentEvent::TextStart);
                        s.gate.emit(AgentEvent::Text { text });
                    }
                }
                "rate_limit_event" => {
                    let info = &msg["rate_limit_info"];
                    if info["status"] != "allowed" {
                        s.gate.emit(AgentEvent::Notice { message: limit_notice(info) });
                    }
                }
                "result" => {
                    let usage = self.usage(&msg);
                    s.gate.emit(usage);
                    let window = msg["modelUsage"].as_object().into_iter().flatten().filter_map(|(_, m)| m["contextWindow"].as_u64()).max();
                    if let (Some(size), true) = (window, ctx_in > 0) {
                        s.gate.emit(AgentEvent::Context { used: ctx_in + ctx_out, size });
                    }
                    if stopping.is_some() {
                        return Ok(Outcome::Stopped);
                    }
                    if msg["is_error"].as_bool().unwrap_or(false) || msg["subtype"] != "success" {
                        let detail = msg["errors"].as_array().and_then(|e| e.first()).and_then(Value::as_str).map(str::to_string);
                        let reason = msg["subtype"].as_str().unwrap_or("error").to_string();
                        return Err(failure.or(detail).unwrap_or(format!("Claude Code ended the turn early ({reason}).")));
                    }
                    return Ok(Outcome::Done);
                }
                _ => {}
            }
        }
    }

    /// Nothing should ask with dontAsk and no built-in tools; if something does, say no rather than hang.
    async fn refuse(&mut self, msg: &Value) -> Result<(), String> {
        let id = msg["request_id"].clone();
        let reply = match msg["request"]["subtype"].as_str() {
            Some("can_use_tool") => json!({ "subtype": "success", "request_id": id, "response": { "behavior": "deny", "message": "Not available in PoB Redux." } }),
            Some("elicitation") => json!({ "subtype": "success", "request_id": id, "response": { "action": "decline" } }),
            _ => json!({ "subtype": "error", "request_id": id, "error": "not supported" }),
        };
        self.write(&json!({ "type": "control_response", "response": reply })).await
    }
}

fn explain(kind: &str, text: &str) -> String {
    match kind {
        "authentication_failed" => "Claude Code is not signed in. Run `claude auth login` in a terminal, then check again in the assistant's provider settings.".into(),
        "billing_error" => format!("Claude Code reported a billing problem: {text}"),
        "rate_limit" => format!("Your Claude plan's usage limit is reached. {text}"),
        _ if !text.is_empty() => format!("Claude Code: {text}"),
        _ => format!("Claude Code failed ({kind})."),
    }
}

fn limit_notice(info: &Value) -> String {
    let when = info["resetsAt"].as_i64().map(|t| {
        let mins = (t - std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(t)) / 60;
        if mins > 90 {
            format!(" It resets in about {} hours.", (mins + 30) / 60)
        } else {
            format!(" It resets in about {} minutes.", mins.max(1))
        }
    });
    let head = if info["status"] == "rejected" { "Your Claude plan's usage limit is reached." } else { "You are close to your Claude plan's usage limit." };
    format!("{head}{}", when.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    #[test]
    fn older_cli_hides_newer_models() {
        let ids = |v| super::models(v).into_iter().map(|m| m.id).collect::<Vec<_>>();
        let old = ids(Some("2.1.200 (Claude Code)"));
        assert!(old.contains(&"claude-fable-5".to_string()));
        assert!(!old.contains(&"claude-opus-5-5".to_string()) && !old.contains(&"claude-opus-5".to_string()));
        assert_eq!(ids(Some("2.1.283 (Claude Code)")).len(), 11);
        assert_eq!(ids(None).len(), 11);
    }
}

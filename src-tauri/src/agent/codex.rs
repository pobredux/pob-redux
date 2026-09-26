//! Codex through `codex app-server`: JSON-RPC over stdio, one request or
//! notification per line.

use std::collections::HashSet;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::watch;

use super::rpc::Conn;
use super::which::Launch;
use super::{AgentEvent, AgentStatus, Outcome, Session};
use crate::ai::ModelInfo;

const SERVER: &str = "pobredux";
const TOKEN_ENV: &str = "POB_REDUX_MCP_TOKEN";

/// Nothing should ask with approvals off and a read-only sandbox; if something does, say no rather than hang.
fn refusal(method: &str) -> Value {
    if method == "item/permissions/requestApproval" {
        json!({ "result": { "permissions": {}, "scope": "turn" } })
    } else if method.ends_with("requestApproval") {
        json!({ "result": { "decision": "decline" } })
    } else if method.contains("elicitation") {
        json!({ "result": { "action": "decline", "content": null } })
    } else {
        json!({ "error": { "code": -32601, "message": "not supported by PoB Redux" } })
    }
}

async fn start(launch: &Launch, overrides: &[String], env: &[(&str, &str)], cwd: &std::path::Path) -> Result<Conn, String> {
    let mut cmd = launch.command();
    cmd.current_dir(cwd).arg("app-server");
    for o in overrides {
        cmd.arg("-c").arg(o);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let conn = Conn::spawn(cmd, "Codex", false, Box::new(|method, _| refusal(method)))?;
    conn.rpc
        .request(
            "initialize",
            json!({
                "clientInfo": { "name": "pob_redux", "title": "PoB Redux", "version": env!("CARGO_PKG_VERSION") },
                "capabilities": { "experimentalApi": true },
            }),
        )
        .await
        .map_err(|e| conn.explain(e))?;
    conn.rpc.notify("initialized", None).await?;
    Ok(conn)
}

pub(crate) async fn auth(launch: &Launch, status: &mut AgentStatus) {
    let dir = std::env::temp_dir();
    let server = match start(launch, &[], &[], &dir).await {
        Ok(s) => s,
        Err(e) => {
            status.error = Some(e);
            return;
        }
    };
    match server.rpc.request("account/read", json!({ "refreshToken": false })).await {
        Ok(v) => {
            let account = &v["account"];
            status.signed_in = Some(!account.is_null() || !v["requiresOpenaiAuth"].as_bool().unwrap_or(true));
            status.account = match account["type"].as_str() {
                Some("chatgpt") => Some(match account["planType"].as_str().filter(|p| !p.is_empty() && *p != "unknown") {
                    Some(plan) => format!("ChatGPT {}{}", plan[..1].to_uppercase(), &plan[1..]),
                    None => "ChatGPT".into(),
                }),
                Some("apiKey") => Some("API key".into()),
                Some(other) => Some(other.to_string()),
                None => None,
            };
        }
        Err(e) => status.error = Some(server.explain(e)),
    }
}

pub(crate) async fn models(launch: &Launch) -> Result<Vec<ModelInfo>, String> {
    let dir = std::env::temp_dir();
    let server = start(launch, &[], &[], &dir).await?;
    let mut all: Vec<Value> = Vec::new();
    let mut cursor = Value::Null;
    for _ in 0..20 {
        let v = server.rpc.request("model/list", json!({ "cursor": cursor })).await.map_err(|e| server.explain(e))?;
        all.extend(v["data"].as_array().cloned().unwrap_or_default());
        cursor = v["nextCursor"].clone();
        if cursor.is_null() {
            break;
        }
    }
    let mut list: Vec<&Value> = all.iter().filter(|m| !m["hidden"].as_bool().unwrap_or(false)).collect();
    list.sort_by_key(|m| !m["isDefault"].as_bool().unwrap_or(false));
    Ok(list
        .into_iter()
        .filter_map(|m| {
            let id = m["model"].as_str().or(m["id"].as_str())?.to_string();
            let tiers = format!("{} {}", m["serviceTiers"], m["additionalSpeedTiers"]).to_ascii_lowercase();
            Some(ModelInfo {
                fast: tiers.contains("fast"),
                label: m["displayName"].as_str().unwrap_or(&id).to_string(),
                efforts: m["supportedReasoningEfforts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e["reasoningEffort"].as_str().or(e.as_str()).map(str::to_string))
                    .collect(),
                recommended: true,
                id,
            })
        })
        .collect())
}

/// Every built-in tool off, so the model can only reach the build, and none of the user's own context loaded.
fn overrides(s: &Session) -> Vec<String> {
    let mut o = vec![
        format!("mcp_servers.{SERVER}.url=\"{}\"", s.mcp_url),
        format!("mcp_servers.{SERVER}.bearer_token_env_var=\"{TOKEN_ENV}\""),
        format!("mcp_servers.{SERVER}.tool_timeout_sec=3600"),
        format!("mcp_servers.{SERVER}.startup_timeout_sec=30"),
        format!("mcp_servers.{SERVER}.required=true"),
        format!("mcp_servers.{SERVER}.default_tools_approval_mode=\"approve\""),
        format!("mcp_servers.{SERVER}.omit_tools_from=[\"deferred\",\"code_mode\"]"),
    ];
    for setting in [
        "web_search=\"disabled\"",
        "agents.enabled=false",
        "tools.experimental_request_user_input.enabled=false",
        "include_permissions_instructions=false",
        "include_environment_context=false",
        "include_apps_instructions=false",
        "include_collaboration_mode_instructions=false",
        "project_doc_max_bytes=0",
        "skills.include_instructions=false",
        "skills.bundled.enabled=false",
    ] {
        o.push(setting.to_string());
    }
    for feature in [
        "shell_tool", "unified_exec", "view_image", "sleep_tool", "image_generation", "multi_agent", "multi_agent_v2", "plugins", "apps",
        "tool_suggest", "goals", "memories", "hooks", "code_mode", "code_mode_only", "request_permissions_tool",
    ] {
        o.push(format!("features.{feature}=false"));
    }
    o
}

pub(crate) struct Proc {
    server: Conn,
    thread: String,
    totals: [u64; 4],
}

impl Proc {
    pub async fn start(s: &Session, launch: &Launch) -> Result<Self, String> {
        let cfg = s.config();
        let server = start(launch, &overrides(s), &[(TOKEN_ENV, s.mcp_token.as_str())], &s.dir).await?;

        // The user's own MCP servers stay off in our threads.
        let mut config = serde_json::Map::new();
        if let Ok(v) = server.rpc.request("config/read", json!({})).await {
            let servers = v["config"]["mcp_servers"].as_object().or(v["config"]["mcpServers"].as_object());
            for name in servers.into_iter().flat_map(|m| m.keys()).filter(|n| *n != SERVER) {
                config.insert(format!("mcp_servers.{name}.enabled"), json!(false));
            }
        }
        let mut params = json!({
            "model": cfg.model,
            "cwd": s.dir.to_string_lossy(),
            "approvalPolicy": "never",
            "sandbox": "read-only",
            "baseInstructions": cfg.instructions,
            "ephemeral": true,
            "config": config,
        });

        let resume = s.resume.lock().unwrap().clone();
        let mut started = match resume {
            Some(id) => server.rpc.request("thread/resume", json!({ "threadId": id, "excludeTurns": true, "config": params["config"] })).await.ok(),
            None => None,
        };
        if started.is_none() {
            // No environment means no shell, file or image tools and no project AGENTS.md; older versions lack the field.
            params["environments"] = json!([]);
            started = match server.rpc.request("thread/start", params.clone()).await {
                Ok(v) => Some(v),
                Err(_) => {
                    params.as_object_mut().map(|p| p.remove("environments"));
                    Some(server.rpc.request("thread/start", params).await.map_err(|e| server.explain(e))?)
                }
            };
        }
        let started = started.unwrap_or_default();
        let thread = started["thread"]["id"].as_str().ok_or("Codex did not open a conversation")?.to_string();
        *s.resume.lock().unwrap() = Some(thread.clone());
        Ok(Self { server, thread, totals: [0; 4] })
    }

    fn usage(&mut self, total: &Value) -> AgentEvent {
        let n = |k: &str| total[k].as_u64().unwrap_or(0);
        let now = [
            n("inputTokens").saturating_sub(n("cachedInputTokens")),
            n("outputTokens"),
            n("cachedInputTokens"),
            n("cacheWriteInputTokens"),
        ];
        let d: Vec<u64> = (0..4).map(|i| now[i].saturating_sub(self.totals[i])).collect();
        self.totals = now;
        AgentEvent::Usage { input: d[0], output: d[1], cache_read: d[2], cache_write: d[3] }
    }

    pub async fn turn(&mut self, s: &Session, text: &str, mut stop: watch::Receiver<u64>) -> Result<Outcome, String> {
        let cfg = s.config();
        let mut params = json!({
            "threadId": self.thread,
            "input": [{ "type": "text", "text": text }],
            "model": cfg.model,
            "environments": [],
        });
        if let Some(effort) = &cfg.effort {
            params["effort"] = json!(effort);
        }
        if cfg.fast {
            params["serviceTier"] = json!("fast");
        }
        stop.borrow_and_update();
        let started = match self.server.rpc.request("turn/start", params.clone()).await {
            Ok(v) => v,
            Err(_) => {
                params.as_object_mut().map(|p| p.remove("environments"));
                self.server.rpc.request("turn/start", params).await.map_err(|e| self.server.explain(e))?
            }
        };
        let turn = started["turn"]["id"].as_str().unwrap_or_default().to_string();

        let mut streamed: HashSet<String> = HashSet::new();
        let mut failure: Option<String> = None;
        let mut stopping: Option<tokio::time::Instant> = None;
        loop {
            let deadline = stopping.map(|t| t + Duration::from_secs(10));
            let msg = tokio::select! {
                m = self.server.events.recv() => m,
                _ = stop.changed(), if stopping.is_none() => {
                    let rpc = self.server.rpc.clone();
                    let params = json!({ "threadId": self.thread, "turnId": turn });
                    tokio::spawn(async move { let _ = rpc.request("turn/interrupt", params).await; });
                    stopping = Some(tokio::time::Instant::now());
                    continue;
                }
                _ = async {
                    match deadline {
                        Some(d) => tokio::time::sleep_until(d).await,
                        None => std::future::pending().await,
                    }
                } => return Err("Codex did not stop when asked.".into()),
            };
            let Some(msg) = msg else {
                return Err(self.server.explain(failure.unwrap_or_else(|| "Codex stopped without saying why".into())));
            };
            let p = &msg["params"];
            if p["threadId"].as_str().is_some_and(|t| t != self.thread) {
                continue;
            }
            match msg["method"].as_str().unwrap_or("") {
                "item/started" if p["item"]["type"] == "agentMessage" => s.gate.emit(AgentEvent::TextStart),
                "item/agentMessage/delta" => {
                    if let Some(id) = p["itemId"].as_str() {
                        streamed.insert(id.to_string());
                    }
                    if let Some(d) = p["delta"].as_str() {
                        s.gate.emit(AgentEvent::Text { text: d.to_string() });
                    }
                }
                "item/completed" if p["item"]["type"] == "agentMessage" => {
                    let item = &p["item"];
                    let seen = item["id"].as_str().is_some_and(|id| streamed.contains(id));
                    if let Some(t) = item["text"].as_str().filter(|t| !seen && !t.is_empty()) {
                        s.gate.emit(AgentEvent::Text { text: t.to_string() });
                    }
                }
                "thread/tokenUsage/updated" => {
                    let t = &p["tokenUsage"];
                    let usage = self.usage(&t["total"]);
                    s.gate.emit(usage);
                    if let (Some(used), Some(size)) = (t["last"]["totalTokens"].as_u64(), t["modelContextWindow"].as_u64()) {
                        s.gate.emit(AgentEvent::Context { used, size });
                    }
                }
                "mcpServer/startupStatus/updated" if p["name"] == SERVER && p["status"] == "failed" => {
                    s.gate.emit(AgentEvent::Notice { message: "Codex could not reach the build's tools.".into() });
                }
                "error" if !p["willRetry"].as_bool().unwrap_or(false) => {
                    failure = explain(&p["error"]);
                }
                "turn/completed" if p["turn"]["id"].as_str().is_none_or(|t| t == turn) => {
                    return match p["turn"]["status"].as_str().unwrap_or("") {
                        "completed" => Ok(Outcome::Done),
                        "interrupted" => Ok(Outcome::Stopped),
                        _ => Err(failure.or_else(|| explain(&p["turn"]["error"])).unwrap_or_else(|| "Codex ended the turn with an error.".into())),
                    };
                }
                _ => {}
            }
        }
    }
}

fn explain(error: &Value) -> Option<String> {
    let message = error["message"].as_str().unwrap_or("").trim();
    let info = format!("{} {message}", error["codexErrorInfo"]).to_ascii_lowercase();
    if info.contains("unauthorized") || info.contains("401") {
        return Some("Codex is not signed in. Run `codex login` in a terminal, then check again in the assistant's provider settings.".into());
    }
    if info.contains("usagelimitexceeded") {
        return Some(format!("Your ChatGPT plan's Codex usage limit is reached. {message}").trim().to_string());
    }
    (!message.is_empty()).then(|| format!("Codex: {message}"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn turn_errors_read_plainly() {
        let unauthorized = json!({ "message": "unexpected status 401", "codexErrorInfo": { "responseStreamDisconnected": { "httpStatusCode": 401 } } });
        assert!(super::explain(&unauthorized).unwrap().contains("codex login"));
        assert!(super::explain(&json!({ "message": "slow down", "codexErrorInfo": "usageLimitExceeded" })).unwrap().contains("usage limit"));
        assert!(super::explain(&json!({ "message": "unexpected status 401 Unauthorized: Missing bearer", "codexErrorInfo": "other" })).unwrap().contains("codex login"));
        assert_eq!(super::explain(&json!({ "message": "" })), None);
    }
}

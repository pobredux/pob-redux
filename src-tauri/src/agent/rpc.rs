//! Newline-delimited JSON-RPC with a child process over stdio: Codex's
//! app-server (no `jsonrpc` field) and ACP agents (JSON-RPC 2.0).

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin};
use tokio::sync::{mpsc, oneshot};

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

/// Answers a request from the child: `{"result": ...}` or `{"error": {...}}`.
pub(super) type Handler = Box<dyn Fn(&str, &Value) -> Value + Send + Sync>;

pub(super) struct Rpc {
    name: &'static str,
    jsonrpc: bool,
    stdin: tokio::sync::Mutex<ChildStdin>,
    pending: Pending,
    next: AtomicU64,
}

impl Rpc {
    async fn send(&self, mut msg: Value) -> Result<(), String> {
        if self.jsonrpc {
            msg["jsonrpc"] = json!("2.0");
        }
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(format!("{msg}\n").as_bytes()).await.map_err(|e| e.to_string())?;
        stdin.flush().await.map_err(|e| e.to_string())
    }

    /// `timeout: None` waits as long as the child works, for a whole prompt turn.
    pub async fn request_for(&self, method: &str, params: Value, timeout: Option<Duration>) -> Result<Value, String> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        self.send(json!({ "id": id, "method": method, "params": params })).await?;
        let out = match timeout {
            Some(t) => tokio::time::timeout(t, rx).await.map_err(|_| format!("{} did not answer {method}", self.name))?,
            None => rx.await,
        };
        self.pending.lock().unwrap().remove(&id);
        out.map_err(|_| format!("{} stopped before answering {method}", self.name))?
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        self.request_for(method, params, Some(Duration::from_secs(60))).await
    }

    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), String> {
        let mut msg = json!({ "method": method });
        if let Some(p) = params {
            msg["params"] = p;
        }
        self.send(msg).await
    }
}

/// Collect the tail of a child's stderr, for error messages.
pub(super) fn drain(mut err: ChildStderr) -> Arc<Mutex<String>> {
    let tail = Arc::new(Mutex::new(String::new()));
    let sink = tail.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        while let Ok(n) = err.read(&mut buf).await {
            if n == 0 {
                break;
            }
            let mut s = sink.lock().unwrap();
            s.push_str(&String::from_utf8_lossy(&buf[..n]));
            if s.len() > 4000 {
                let cut = s.len() - 4000;
                let cut = (cut..s.len()).find(|&i| s.is_char_boundary(i)).unwrap_or(s.len());
                s.drain(..cut);
            }
        }
    });
    tail
}

/// A running child: requests go out through `rpc`, notifications come back on `events`.
pub(super) struct Conn {
    _child: Child,
    _tree: Option<super::which::Tree>,
    pub rpc: Arc<Rpc>,
    pub events: mpsc::UnboundedReceiver<Value>,
    stderr: Arc<Mutex<String>>,
    /// Lines on stdout that were not JSON, such as a sign-in link.
    noise: Arc<Mutex<String>>,
}

impl Conn {
    pub fn spawn(mut cmd: tokio::process::Command, name: &'static str, jsonrpc: bool, handler: Handler) -> Result<Self, String> {
        cmd.stdin(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| format!("could not start {name}: {e}"))?;
        let tree = super::which::Tree::attach(&child);
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        let stderr = drain(child.stderr.take().ok_or("no stderr")?);

        let rpc = Arc::new(Rpc { name, jsonrpc, stdin: tokio::sync::Mutex::new(stdin), pending: Pending::default(), next: AtomicU64::new(1) });
        let (tx, events) = mpsc::unbounded_channel();
        let reader = rpc.clone();
        let noise = Arc::new(Mutex::new(String::new()));
        let heard = noise.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                    log::debug!("{name}: {line}");
                    let mut n = heard.lock().unwrap();
                    if n.len() < 16_000 {
                        n.push_str(&line);
                        n.push('\n');
                    }
                    continue;
                };
                match (msg.get("id").filter(|id| !id.is_null()), msg["method"].as_str()) {
                    (Some(id), Some(method)) => {
                        let mut reply = handler(method, &msg["params"]);
                        reply["id"] = id.clone();
                        let _ = reader.send(reply).await;
                    }
                    (Some(id), None) => {
                        let Some(id) = id.as_u64() else { continue };
                        if let Some(tx) = reader.pending.lock().unwrap().remove(&id) {
                            let out = match msg.get("error") {
                                Some(e) => Err(e["message"].as_str().unwrap_or("error").to_string()),
                                None => Ok(msg.get("result").cloned().unwrap_or(Value::Null)),
                            };
                            let _ = tx.send(out);
                        }
                    }
                    (None, Some(_)) => {
                        if tx.send(msg).is_err() {
                            break;
                        }
                    }
                    (None, None) => {}
                }
            }
            reader.pending.lock().unwrap().clear();
        });
        Ok(Self { _child: child, _tree: tree, rpc, events, stderr, noise })
    }

    /// Everything the child printed that was not protocol traffic.
    pub fn chatter(&self) -> String {
        format!("{}\n{}", self.noise.lock().unwrap(), self.stderr.lock().unwrap())
    }

    pub fn explain(&self, e: String) -> String {
        let tail = self.stderr.lock().unwrap().trim().to_string();
        if tail.is_empty() {
            e
        } else {
            format!("{e}: {tail}")
        }
    }
}

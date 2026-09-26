//! The assistant through the user's own coding agent (Claude Code, Codex,
//! Cursor, Grok, OpenCode, Antigravity), so their subscription pays
//! for it. The agent runs the conversation; the build's tools reach it over a
//! private MCP server that only this session can call, and every call passes
//! the gate below so the panel can show it and hold writes for approval.

mod acp;
mod antigravity;
mod claude;
mod codex;
mod rpc;
mod terminal;
mod which;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tokio::sync::{oneshot, watch};

use crate::tools::{defs, dispatch, JsonObject, ToolContext, ToolError};
use which::Launch;

/// Characters of one tool result the model sees; the panel still gets all of it.
const MAX_RESULT_CHARS: usize = 6000;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Cli {
    Claude,
    Codex,
    Cursor,
    Grok,
    OpenCode,
    Antigravity,
}

pub const CLIS: [Cli; 6] = [Cli::Claude, Cli::Codex, Cli::Cursor, Cli::Grok, Cli::OpenCode, Cli::Antigravity];

struct Info {
    id: &'static str,
    label: &'static str,
    binary: &'static str,
    login: Option<&'static str>,
    install: &'static str,
}

impl Cli {
    fn info(self) -> Info {
        let (id, label, binary, login, install) = match self {
            Cli::Claude => ("claude", "Claude", "claude", Some("claude auth login"), "https://code.claude.com/docs/en/setup"),
            Cli::Codex => ("codex", "Codex", "codex", Some("codex login"), "https://developers.openai.com/codex/cli"),
            Cli::Cursor => ("cursor", "Cursor", "cursor-agent", Some("cursor-agent login"), "https://cursor.com/docs/cli/installation"),
            Cli::Grok => ("grok", "Grok", "grok", Some("grok login"), "https://www.npmjs.com/package/@xai-official/grok"),
            Cli::OpenCode => ("opencode", "OpenCode", "opencode", Some("opencode auth login"), "https://opencode.ai/docs/"),
            Cli::Antigravity => ("antigravity", "Antigravity", "agy_acp_server", None, "https://antigravity.google/docs/ide/extensions"),
        };
        Info { id, label, binary, login, install }
    }

    pub fn id(self) -> &'static str {
        self.info().id
    }

    pub fn from_id(id: &str) -> Option<Cli> {
        CLIS.into_iter().find(|c| c.id() == id)
    }

    pub fn label(self) -> &'static str {
        self.info().label
    }

    pub fn login(self) -> Option<&'static str> {
        self.info().login
    }

    /// The vendor's own install command for this computer: PowerShell on Windows, a shell line elsewhere.
    pub fn install_command(self) -> Option<&'static str> {
        let (windows, unix) = match self {
            Cli::Claude => ("irm https://claude.ai/install.ps1 | iex", "curl -fsSL https://claude.ai/install.sh | bash"),
            Cli::Codex => ("irm https://chatgpt.com/codex/install.ps1 | iex", "curl -fsSL https://chatgpt.com/codex/install.sh | sh"),
            Cli::Cursor => ("irm 'https://cursor.com/install?win32=true' | iex", "curl https://cursor.com/install -fsS | bash"),
            Cli::Grok => ("irm https://x.ai/cli/install.ps1 | iex", "curl -fsSL https://x.ai/cli/install.sh | bash"),
            Cli::OpenCode => ("npm install -g opencode-ai", "curl -fsSL https://opencode.ai/install | bash"),
            Cli::Antigravity => return None,
        };
        Some(if cfg!(windows) { windows } else { unix })
    }

    fn login_args(self) -> &'static [&'static str] {
        match self {
            Cli::Claude | Cli::OpenCode => &["auth", "login"],
            Cli::Codex | Cli::Cursor | Cli::Grok => &["login"],
            Cli::Antigravity => &[],
        }
    }

    /// Bytes the app downloads itself, for an agent it installs rather than finds.
    pub fn download(self) -> Option<u64> {
        (self == Cli::Antigravity).then(antigravity::asset).flatten().map(|a| a.bytes)
    }

    pub fn install(self) -> &'static str {
        self.info().install
    }

    fn acp(self) -> Option<&'static acp::Spec> {
        match self {
            Cli::Cursor => Some(&acp::CURSOR),
            Cli::Grok => Some(&acp::GROK),
            Cli::OpenCode => Some(&acp::OPENCODE),
            Cli::Antigravity => Some(&acp::ANTIGRAVITY),
            Cli::Claude | Cli::Codex => None,
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AgentEvent {
    TextStart,
    Text { text: String },
    Tool { id: String, name: String, args: Value, read_only: bool, awaiting: bool },
    ToolDone { id: String, result: Value },
    ToolFailed { id: String, error: String, skipped: bool },
    Usage { input: u64, output: u64, cache_read: u64, cache_write: u64 },
    /// Tokens the conversation now fills, out of the model's window.
    Context { used: u64, size: u64 },
    Notice { message: String },
}

/// Narrows the registry to the panel's tools, reports each call, and holds
/// writes until the panel answers.
pub(crate) struct Gate {
    prefix: String,
    allowed: HashSet<String>,
    sink: Mutex<Option<Channel<AgentEvent>>>,
    pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
    seq: AtomicU64,
    seen: Mutex<HashMap<String, u32>>,
}

fn clip(name: &str, value: &Value) -> String {
    let text = match value {
        Value::String(s) => s.clone(),
        v => v.to_string(),
    };
    if text.chars().count() <= MAX_RESULT_CHARS {
        return text;
    }
    let head: String = text.chars().take(MAX_RESULT_CHARS).collect();
    let rest = text.chars().count() - MAX_RESULT_CHARS;
    format!("{head}\n[cut: {rest} more characters. Call {name} again with a limit, a filter or a narrower argument if you need the rest.]")
}

impl Gate {
    fn new(prefix: String, allowed: HashSet<String>) -> Self {
        Self {
            prefix,
            allowed,
            sink: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            seq: AtomicU64::new(0),
            seen: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn lists(&self, name: &str) -> bool {
        self.allowed.contains(name)
    }

    fn names(&self) -> Vec<String> {
        self.allowed.iter().cloned().collect()
    }

    pub(crate) fn emit(&self, event: AgentEvent) {
        if let Some(sink) = self.sink.lock().unwrap().as_ref() {
            let _ = sink.send(event);
        }
    }

    fn begin(&self, sink: Channel<AgentEvent>) {
        self.seen.lock().unwrap().clear();
        *self.sink.lock().unwrap() = Some(sink);
    }

    fn end(&self) {
        self.decline_all();
        *self.sink.lock().unwrap() = None;
    }

    fn resolve(&self, id: &str, ok: bool) {
        if let Some(tx) = self.pending.lock().unwrap().remove(id) {
            let _ = tx.send(ok);
        }
    }

    fn decline_all(&self) {
        for (_, tx) in self.pending.lock().unwrap().drain() {
            let _ = tx.send(false);
        }
    }

    pub(crate) async fn call(&self, ctx: Arc<ToolContext>, name: String, args: JsonObject) -> Result<String, String> {
        let Some(def) = defs().into_iter().find(|d| d.name == name).filter(|d| self.allowed.contains(d.name)) else {
            return Err(format!("{name} is not available in the assistant."));
        };
        if def.read_only {
            let key = format!("{name}:{}", Value::Object(args.clone()));
            let mut seen = self.seen.lock().unwrap();
            let n = seen.entry(key).or_insert(0);
            *n += 1;
            if *n >= 3 {
                return Ok(format!(
                    "You already called {name} with these arguments {} times this turn and the result did not change. Do not call it again. Use what you have and answer the user now.",
                    *n - 1
                ));
            }
        }

        let id = format!("{}-{}", self.prefix, self.seq.fetch_add(1, Ordering::Relaxed) + 1);
        let awaiting = !def.read_only;
        let rx = awaiting.then(|| {
            let (tx, rx) = oneshot::channel();
            self.pending.lock().unwrap().insert(id.clone(), tx);
            rx
        });
        self.emit(AgentEvent::Tool { id: id.clone(), name: name.clone(), args: Value::Object(args.clone()), read_only: def.read_only, awaiting });
        if let Some(rx) = rx {
            if !rx.await.unwrap_or(false) {
                self.emit(AgentEvent::ToolFailed { id, error: String::new(), skipped: true });
                return Err("The user declined this change. Do not retry it; suggest an alternative or ask why.".into());
            }
        }

        match dispatch(ctx, name.clone(), args).await {
            Ok((value, _)) => {
                let text = clip(&name, &value);
                self.emit(AgentEvent::ToolDone { id, result: value });
                Ok(text)
            }
            Err(ToolError::Invalid(msg) | ToolError::Failed(msg)) => {
                self.emit(AgentEvent::ToolFailed { id, error: msg.clone(), skipped: false });
                Err(msg)
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct Config {
    pub model: String,
    pub effort: Option<String>,
    /// The provider's faster, dearer service where the model offers one.
    pub fast: bool,
    pub instructions: String,
}

pub(crate) struct Session {
    cli: Cli,
    launch: Launch,
    pub(crate) gate: Arc<Gate>,
    pub(crate) mcp_url: String,
    pub(crate) mcp_token: String,
    server: tauri::async_runtime::JoinHandle<()>,
    pub(crate) dir: PathBuf,
    config: Mutex<Config>,
    /// The CLI's own id for the conversation, so a restarted process resumes it.
    pub(crate) resume: Mutex<Option<String>>,
    proc: tokio::sync::Mutex<Option<Proc>>,
    /// Claude takes the model and effort at launch, so a change restarts it before the next message.
    restart: AtomicBool,
    stop: watch::Sender<u64>,
}

enum Proc {
    Claude(claude::Proc),
    Codex(codex::Proc),
    Acp(acp::Proc),
}

pub(crate) enum Outcome {
    Done,
    Stopped,
}

impl Session {
    pub(crate) fn config(&self) -> Config {
        self.config.lock().unwrap().clone()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.server.abort();
        let _ = std::fs::remove_dir_all(&self.dir);
        if let Ok(mut proc) = self.proc.try_lock() {
            *proc = None;
        }
        remove_later(&self.launch);
    }
}

/// Delete a launch's temporary folder once its processes have let go of it.
fn remove_later(launch: &Launch) {
    let Some(scratch) = launch.scratch.clone() else { return };
    std::thread::spawn(move || {
        for _ in 0..20 {
            if std::fs::remove_dir_all(&scratch).is_ok() || !scratch.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
}

#[derive(Serialize, Clone, Default)]
pub struct AgentStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    /// None when the CLI could not say.
    pub signed_in: Option<bool>,
    pub account: Option<String>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct AgentState {
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    status: tokio::sync::Mutex<HashMap<Cli, AgentStatus>>,
    paths: Mutex<HashMap<String, String>>,
    /// Listing an agent's models starts it, which for some takes seconds; kept until "Check again".
    models: Mutex<HashMap<Cli, Vec<crate::ai::ModelInfo>>>,
    signing_in: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

fn paths_file(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_config_dir().ok()?.join("agents.json"))
}

impl AgentState {
    pub fn new(app: &AppHandle) -> Self {
        if let Ok(dir) = app.path().app_local_data_dir() {
            let _ = std::fs::remove_dir_all(dir.join("agent"));
        }
        let paths = paths_file(app)
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { paths: Mutex::new(paths), ..Default::default() }
    }

    fn configured(&self, cli: Cli) -> Option<String> {
        self.paths.lock().unwrap().get(cli.id()).cloned()
    }

    fn launch(&self, app: &AppHandle, cli: Cli) -> Option<Launch> {
        let configured = self.configured(cli);
        match cli {
            Cli::Antigravity => antigravity::launch(app, configured.as_deref()),
            // Cursor's newer installs name it `agent`, which Grok's installer also ships; only Cursor's counts.
            Cli::Cursor => which::find("cursor-agent", configured.as_deref()).or_else(|| {
                which::find("agent", None).filter(|l| l.display().to_ascii_lowercase().contains("cursor"))
            }),
            _ => which::find(cli.info().binary, configured.as_deref()),
        }
    }

    pub fn shutdown(&self) {
        self.sessions.lock().unwrap().clear();
    }
}

async fn detect(launch: Option<Launch>, cli: Cli) -> AgentStatus {
    let Some(launch) = launch else {
        return AgentStatus::default();
    };
    let mut status = AgentStatus { installed: true, path: Some(launch.display()), ..Default::default() };
    match launch.output(&["--version"], Duration::from_secs(15)).await {
        Ok((out, err, true)) => status.version = Some(out.trim().lines().next().unwrap_or(err.trim()).to_string()),
        Ok((out, err, false)) => status.error = Some(format!("{} {}", out.trim(), err.trim()).trim().to_string()),
        Err(e) => status.error = Some(e),
    }
    if status.error.is_some() {
        return status;
    }
    match (cli, cli.acp()) {
        (_, Some(spec)) => acp::auth(spec, &launch, &mut status).await,
        (Cli::Codex, None) => codex::auth(&launch, &mut status).await,
        _ => claude::auth(&launch, &mut status).await,
    }
    status
}

/// Installed, version and sign-in state per CLI. Cached; `refresh` checks again,
/// all of them or `only` one.
pub(crate) async fn statuses(app: &AppHandle, refresh: bool, only: Option<Cli>) -> HashMap<Cli, AgentStatus> {
    let state = app.state::<AgentState>();
    let mut cache = state.status.lock().await;
    if refresh || cache.len() < CLIS.len() {
        if refresh {
            which::refresh_search_path();
            match only {
                Some(cli) => {
                    state.models.lock().unwrap().remove(&cli);
                }
                None => state.models.lock().unwrap().clear(),
            }
        }
        let probed: Vec<Cli> =
            CLIS.into_iter().filter(|c| *c != Cli::Antigravity && (cache.len() < CLIS.len() || only.is_none_or(|o| o == *c))).collect();
        let checks: Vec<_> = probed.iter().map(|&cli| tauri::async_runtime::spawn(detect(state.launch(app, cli), cli))).collect();
        for (cli, check) in probed.into_iter().zip(checks) {
            cache.insert(cli, check.await.unwrap_or_default());
        }
        cache.insert(Cli::Antigravity, antigravity::status(app, state.configured(Cli::Antigravity).as_deref()));
    }
    cache.clone()
}

pub(crate) async fn models(app: &AppHandle, cli: Cli) -> Result<Vec<crate::ai::ModelInfo>, String> {
    if cli == Cli::Claude {
        let installed = statuses(app, false, None).await.remove(&cli).and_then(|s| s.version);
        return Ok(claude::models(installed.as_deref()));
    }
    let state = app.state::<AgentState>();
    if let Some(list) = state.models.lock().unwrap().get(&cli) {
        return Ok(list.clone());
    }
    let launch = state.launch(app, cli).ok_or_else(|| format!("{} is not installed", cli.label()))?;
    let list = match cli.acp() {
        Some(spec) => acp::models(spec, &launch).await,
        None => codex::models(&launch).await,
    };
    remove_later(&launch);
    let list = list?;
    if !list.is_empty() {
        state.models.lock().unwrap().insert(cli, list.clone());
    }
    Ok(list)
}

fn managed_cli(provider: &str) -> Result<Cli, String> {
    Cli::from_id(provider).filter(|c| *c == Cli::Antigravity).ok_or_else(|| format!("{provider} is not installed by the app"))
}

/// Download and unpack an agent the app installs itself.
#[tauri::command]
pub async fn agent_install(app: AppHandle, provider: String, on_progress: Channel<antigravity::Progress>) -> Result<(), String> {
    managed_cli(&provider)?;
    antigravity::install(&app, &on_progress).await?;
    statuses(&app, true, Some(Cli::Antigravity)).await;
    Ok(())
}

/// Run the agent's own sign-in. Resolves when it finishes; the sign-in link
/// arrives as an `agent:sign-in` event in case the browser did not open.
#[tauri::command]
pub async fn agent_sign_in(app: AppHandle, provider: String) -> Result<(), String> {
    let cli = managed_cli(&provider)?;
    let state = app.state::<AgentState>();
    let launch = state.launch(&app, cli).ok_or("Antigravity is not installed")?;
    let profile = antigravity::profile(&app)?;
    let (tx, rx) = oneshot::channel();
    let emitter = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        let out = acp::sign_in(&acp::ANTIGRAVITY, &launch, &profile, |url| {
            let _ = tauri::Emitter::emit(&emitter, "agent:sign-in", serde_json::json!({ "provider": "antigravity", "url": url }));
        })
        .await;
        let _ = tx.send(out);
    });
    if let Some(old) = state.signing_in.lock().unwrap().replace(task) {
        old.abort();
    }
    let out = rx.await.unwrap_or_else(|_| Err("Sign-in was cancelled.".into()));
    antigravity::mark_signed_in(&app, out.is_ok());
    statuses(&app, true, Some(cli)).await;
    out
}

/// Open a terminal running the vendor's install command, or its sign-in with
/// the program the app found. Returns the command shown to the user.
#[tauri::command]
pub async fn agent_terminal(app: AppHandle, provider: String, action: String) -> Result<String, String> {
    let cli = Cli::from_id(&provider).ok_or_else(|| format!("unknown provider {provider}"))?;
    let path = which::search_path();
    let title = format!("PoB Redux: {} {}", cli.label(), if action == "install" { "install" } else { "sign-in" });
    if action == "install" {
        let cmd = cli.install_command().ok_or_else(|| format!("{} is installed from the app", cli.label()))?;
        terminal::open(&title, cmd, cmd, &path)?;
        return Ok(cmd.to_string());
    }
    let launch = app.state::<AgentState>().launch(&app, cli).ok_or_else(|| format!("{} is not installed", cli.label()))?;
    let parts: Vec<String> = std::iter::once(launch.program.to_string_lossy().into_owned())
        .chain(launch.prefix.iter().map(|p| p.to_string_lossy().into_owned()))
        .chain(cli.login_args().iter().map(|a| a.to_string()))
        .collect();
    let env_ps: String = launch.env.iter().map(|(k, v)| format!("$env:{k}={}; ", terminal::ps_quote(&v.to_string_lossy()))).collect();
    let env_sh: String = launch.env.iter().map(|(k, v)| format!("{k}={} ", terminal::sh_quote(&v.to_string_lossy()))).collect();
    let ps = format!("{env_ps}& {}", parts.iter().map(|p| terminal::ps_quote(p)).collect::<Vec<_>>().join(" "));
    let sh = format!("{env_sh}{}", parts.iter().map(|p| terminal::sh_quote(p)).collect::<Vec<_>>().join(" "));
    terminal::open(&title, &ps, &sh, &path)?;
    Ok(cli.login().unwrap_or_default().to_string())
}

/// Delete an agent the app downloaded.
#[tauri::command]
pub async fn agent_uninstall(app: AppHandle, provider: String) -> Result<(), String> {
    managed_cli(&provider)?;
    antigravity::uninstall(&app)?;
    statuses(&app, true, Some(Cli::Antigravity)).await;
    Ok(())
}

#[tauri::command]
pub fn agent_sign_in_cancel(app: AppHandle) {
    if let Some(task) = app.state::<AgentState>().signing_in.lock().unwrap().take() {
        task.abort();
    }
}

#[tauri::command]
pub async fn agent_sign_out(app: AppHandle, provider: String) -> Result<(), String> {
    managed_cli(&provider)?;
    antigravity::sign_out(&app)?;
    statuses(&app, true, Some(Cli::Antigravity)).await;
    Ok(())
}

fn session(app: &AppHandle, id: &str) -> Result<Arc<Session>, String> {
    app.state::<AgentState>().sessions.lock().unwrap().get(id).cloned().ok_or_else(|| "the assistant session ended; send the message again".into())
}

fn random_hex(bytes: usize) -> Result<String, String> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

#[tauri::command]
pub async fn agent_path_set(app: AppHandle, provider: String, path: String) -> Result<(), String> {
    let cli = Cli::from_id(&provider).ok_or_else(|| format!("unknown provider {provider}"))?;
    let state = app.state::<AgentState>();
    let snapshot = {
        let mut paths = state.paths.lock().unwrap();
        let path = path.trim();
        if path.is_empty() {
            paths.remove(cli.id());
        } else {
            paths.insert(cli.id().to_string(), path.to_string());
        }
        paths.clone()
    };
    let file = paths_file(&app).ok_or("no config dir")?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&file, serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    statuses(&app, true, Some(cli)).await;
    Ok(())
}

/// Start a conversation. The CLI itself starts on the first message.
#[tauri::command]
pub async fn agent_open(
    app: AppHandle,
    provider: String,
    model: String,
    effort: Option<String>,
    fast: Option<bool>,
    tools: Vec<String>,
    instructions: String,
) -> Result<String, String> {
    let cli = Cli::from_id(&provider).ok_or_else(|| format!("unknown provider {provider}"))?;
    if app.state::<crate::AppState>().game() != crate::game::Game::Poe2 {
        return Err("the assistant is a PoE2 feature".into());
    }
    let state = app.state::<AgentState>();
    let launch = state.launch(&app, cli).ok_or_else(|| format!("{} is not installed", cli.label()))?;
    let id = random_hex(8)?;
    let token = random_hex(32)?;
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("agent").join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;

    let gate = Arc::new(Gate::new(id[..6].to_string(), tools.into_iter().collect()));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.map_err(|e| format!("cannot open the tool server: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let server = crate::mcp::serve(crate::mcp::tool_context(&app), listener, token.clone(), Some(gate.clone()));

    let session = Arc::new(Session {
        cli,
        launch,
        gate,
        mcp_url: format!("http://127.0.0.1:{port}/mcp"),
        mcp_token: token,
        server,
        dir,
        config: Mutex::new(Config { model, effort: effort.filter(|e| !e.is_empty()), fast: fast.unwrap_or(false), instructions }),
        resume: Mutex::new(None),
        proc: tokio::sync::Mutex::new(None),
        restart: AtomicBool::new(false),
        stop: watch::channel(0).0,
    });
    state.sessions.lock().unwrap().insert(id.clone(), session);
    Ok(id)
}

/// Send one message and stream the reply. Resolves when the turn ends: "done",
/// or "stopped" after `agent_stop`.
#[tauri::command]
pub async fn agent_send(
    app: AppHandle,
    session: String,
    text: String,
    on_event: Channel<AgentEvent>,
) -> Result<String, String> {
    let s = self::session(&app, &session)?;
    let mut proc = s.proc.lock().await;
    let stop = s.stop.subscribe();
    s.gate.begin(on_event);
    let out = run_turn(&s, &mut proc, &text, stop).await;
    s.gate.end();
    match out {
        Ok(Outcome::Done) => Ok("done".into()),
        Ok(Outcome::Stopped) => Ok("stopped".into()),
        Err(e) => {
            *proc = None;
            if s.cli == Cli::Antigravity && e == acp::signed_out(&acp::ANTIGRAVITY) {
                antigravity::mark_signed_in(&app, false);
            }
            Err(e)
        }
    }
}

async fn run_turn(s: &Arc<Session>, proc: &mut Option<Proc>, text: &str, stop: watch::Receiver<u64>) -> Result<Outcome, String> {
    if s.restart.swap(false, Ordering::Relaxed) {
        *proc = None;
    }
    if proc.is_none() {
        *proc = Some(match (s.cli, s.cli.acp()) {
            (_, Some(spec)) => Proc::Acp(acp::Proc::start(spec, s, &s.launch, s.gate.names()).await?),
            (Cli::Codex, None) => Proc::Codex(codex::Proc::start(s, &s.launch).await?),
            _ => Proc::Claude(claude::Proc::start(s, &s.launch).await?),
        });
    }
    match proc.as_mut() {
        Some(Proc::Claude(p)) => p.turn(s, text, stop).await,
        Some(Proc::Codex(p)) => p.turn(s, text, stop).await,
        Some(Proc::Acp(p)) => p.turn(s, text, stop).await,
        None => unreachable!(),
    }
}

#[tauri::command]
pub fn agent_approve(app: AppHandle, session: String, id: String, ok: bool) -> Result<(), String> {
    self::session(&app, &session)?.gate.resolve(&id, ok);
    Ok(())
}

#[tauri::command]
pub fn agent_stop(app: AppHandle, session: String) -> Result<(), String> {
    let s = self::session(&app, &session)?;
    s.gate.decline_all();
    s.stop.send_modify(|n| *n += 1);
    Ok(())
}

/// Change the model or effort. Takes effect on the next message.
#[tauri::command]
pub async fn agent_configure(app: AppHandle, session: String, model: String, effort: Option<String>, fast: Option<bool>) -> Result<(), String> {
    let s = self::session(&app, &session)?;
    let changed = {
        let mut c = s.config.lock().unwrap();
        let effort = effort.filter(|e| !e.is_empty());
        let fast = fast.unwrap_or(false);
        let changed = c.model != model || c.effort != effort || c.fast != fast;
        c.model = model;
        c.effort = effort;
        c.fast = fast;
        changed
    };
    if changed && s.cli == Cli::Claude {
        s.restart.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub fn agent_close(app: AppHandle, session: String) {
    let removed = app.state::<AgentState>().sessions.lock().unwrap().remove(&session);
    if let Some(s) = removed {
        s.gate.decline_all();
        s.stop.send_modify(|n| *n += 1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn long_results_are_clipped_with_a_note() {
        let long = serde_json::Value::String("x".repeat(super::MAX_RESULT_CHARS + 10));
        let out = super::clip("get_items", &long);
        assert!(out.contains("10 more characters"));
        assert!(out.contains("Call get_items again"));
        assert_eq!(super::clip("get_stats", &serde_json::json!({"a": 1})), r#"{"a":1}"#);
    }
}

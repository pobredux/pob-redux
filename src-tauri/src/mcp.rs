//! First-party MCP server. Exposes the build that is open in the app to AI
//! clients over Streamable HTTP on localhost. Off until enabled in Options.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, CacheScope, ContentBlock,
    CreateTaskResult, ElicitRequestParams, ElicitationSchema, GetTaskParams, GetTaskResult,
    Implementation, InputRequiredResult, JsonObject, ListToolsResult, PaginatedRequestParams,
    ProtocolVersion, ServerCapabilities, ServerInfo, Tool, ToolAnnotations, UpdateTaskParams,
    CancelTaskParams,
};
use rmcp::service::RequestContext;
use rmcp::task_manager::{TaskExit, TaskManager, TaskOptions};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, RoleServer, ServerHandler};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::agent::Gate;
use crate::AppState;
use crate::tools::{defs, dispatch, ToolContext, ToolDef, ToolError, INSTRUCTIONS};

pub const DEFAULT_PORT: u16 = 7315;

/// The tool table only changes when the binary does, so clients may hold it for
/// a whole session. Deterministic order plus a TTL is what SEP-2549 asks for.
const TOOL_LIST_TTL_MS: u64 = 3_600_000;
const TASK_TTL_MS: u64 = 600_000;
const CONFIRM_KEY: &str = "confirm";

/// The revisions this server is built and tested against. rmcp would otherwise
/// advertise every revision it knows, back to 2024-11-05.
fn versions() -> Cow<'static, [ProtocolVersion]> {
    Cow::Owned(vec![
        ProtocolVersion::V_2025_06_18,
        ProtocolVersion::V_2025_11_25,
        ProtocolVersion::V_2026_07_28,
    ])
}

/// A tool context bound to the live engine. Shared by the MCP transport and by
/// the chat panel's `ai_call_tool`, so both drive the same build.
pub(crate) fn tool_context(app: &AppHandle) -> Arc<ToolContext> {
    let state = app.state::<AppState>();
    Arc::new(ToolContext {
        engine: state.engine(),
        pool: state.pool(),

        app: app.clone(),
        calls: state.mcp.calls.clone(),
    })
}

// ---------------------------------------------------------------------------
// Access token
// ---------------------------------------------------------------------------

fn token_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("mcp-token"))
}

pub(crate) fn saved_token(app: &AppHandle) -> Option<String> {
    let path = app.path().app_config_dir().ok()?.join("mcp-token");
    std::fs::read_to_string(path).ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

/// The bearer token every request must carry. Written once and reused, so a
/// client configured with it keeps working across restarts.
fn load_or_create_token(app: &AppHandle) -> Result<String, String> {
    let path = token_path(app)?;
    if let Ok(existing) = std::fs::read_to_string(&path) {
        let existing = existing.trim().to_string();
        if existing.len() >= 32 {
            return Ok(existing);
        }
    }
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(&path, &token).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(token)
}

#[derive(Serialize, Clone)]
pub struct McpStatus {
    pub running: bool,
    pub port: u16,
    pub url: Option<String>,
    pub token: Option<String>,
    pub error: Option<String>,
    pub calls: u64,
}

struct Running {
    port: u16,
    task: tauri::async_runtime::JoinHandle<()>,
}

pub struct McpState {
    running: Mutex<Option<Running>>,
    port: Mutex<u16>,
    token: Mutex<Option<String>>,
    error: Mutex<Option<String>>,
    calls: Arc<AtomicU64>,
}

impl McpState {
    pub fn new() -> Self {
        Self {
            running: Mutex::new(None),
            port: Mutex::new(DEFAULT_PORT),
            token: Mutex::new(None),
            error: Mutex::new(None),
            calls: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn status(&self) -> McpStatus {
        let running = self.running.lock().unwrap();
        let port = running.as_ref().map(|r| r.port).unwrap_or(*self.port.lock().unwrap());
        McpStatus {
            running: running.is_some(),
            port,
            url: running.as_ref().map(|r| format!("http://127.0.0.1:{}/mcp", r.port)),
            token: running.as_ref().and(self.token.lock().unwrap().clone()),
            error: self.error.lock().unwrap().clone(),
            calls: self.calls.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn stop(&self) {
        if let Some(r) = self.running.lock().unwrap().take() {
            r.task.abort();
            log::info!("mcp server stopped");
        }
    }

    async fn start(&self, ctx: Arc<ToolContext>, port: u16, token: String) -> Result<(), String> {
        self.stop();
        *self.port.lock().unwrap() = port;
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
            .await
            .map_err(|e| format!("cannot listen on 127.0.0.1:{port}: {e}"))?;
        let task = serve(ctx, listener, token.clone(), None);
        *self.running.lock().unwrap() = Some(Running { port, task });
        *self.token.lock().unwrap() = Some(token);
        *self.error.lock().unwrap() = None;
        log::info!("mcp server listening on http://127.0.0.1:{port}/mcp");
        Ok(())
    }
}

/// Serve the registry on a bound listener. With a `gate`, the server belongs to
/// one assistant session: it lists only that session's tools and reports every
/// call to the panel.
pub(crate) fn serve(
    ctx: Arc<ToolContext>,
    listener: tokio::net::TcpListener,
    token: String,
    gate: Option<Arc<Gate>>,
) -> tauri::async_runtime::JoinHandle<()> {
    // Host header validation (DNS-rebinding protection) is rmcp's default:
    // only localhost hosts are accepted.
    // One manager for the whole server: 2026-07-28 builds a fresh handler
    // per request, so a per-handler store would lose every task handle.
    let tasks = TaskManager::new();
    let service = StreamableHttpService::new(
        move || {
            Ok(PobMcp {
                ctx: ctx.clone(),
                tasks: tasks.clone(),
                gate: gate.clone(),
            })
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let expected = Arc::new(token);
    let router = axum::Router::new()
        .nest_service("/mcp", service)
        .layer(axum::middleware::from_fn(move |req, next| {
            guard(expected.clone(), req, next)
        }));
    tauri::async_runtime::spawn(async move {
        if let Err(e) = axum::serve(listener, router).await {
            log::error!("mcp server: {e}");
        }
    })
}

/// Two gates in front of the transport. The bearer token stops any other local
/// process driving the build; rejecting a request that carries `Origin` keeps a
/// web page out even if it finds a way past the browser's own preflight.
async fn guard(
    expected: Arc<String>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::http::StatusCode;

    let deny = |code: StatusCode, msg: &str| -> axum::response::Response {
        log::warn!("mcp request rejected: {msg}");
        (code, msg.to_string()).into_response()
    };
    use axum::response::IntoResponse;

    if req.headers().contains_key(axum::http::header::ORIGIN) {
        return deny(StatusCode::FORBIDDEN, "browser origins are not accepted");
    }
    let ok = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|t| t.trim() == expected.as_str());
    if !ok {
        return deny(StatusCode::UNAUTHORIZED, "missing or wrong bearer token");
    }
    next.run(req).await
}

#[tauri::command]
pub fn mcp_status(state: State<'_, AppState>) -> McpStatus {
    state.mcp.status()
}

#[tauri::command]
pub async fn mcp_start(app: AppHandle, port: u16) -> Result<McpStatus, String> {
    start_with_handle(&app, port).await
}

/// Start (or restart) the server on `port`. Also used by the `POB_REDUX_MCP`
/// launch hook, which brings the server up without the UI.
pub async fn start_with_handle(app: &AppHandle, port: u16) -> Result<McpStatus, String> {
    if port < 1024 {
        return Err("port must be 1024 or higher".into());
    }
    if app.state::<AppState>().game() != crate::game::Game::Poe2 {
        return Err("the MCP server is a PoE2 feature; switch game to start it".into());
    }
    let ctx = tool_context(app);
    let token = load_or_create_token(app)?;
    let state = app.state::<AppState>();
    let result = state.mcp.start(ctx, port, token).await;
    if let Err(e) = &result {
        *state.mcp.error.lock().unwrap() = Some(e.clone());
    }
    let status = state.mcp.status();
    let _ = app.emit("mcp:status", status.clone());
    result.map(|_| status)
}

#[tauri::command]
pub fn mcp_stop(app: AppHandle, state: State<'_, AppState>) -> McpStatus {
    state.mcp.stop();
    *state.mcp.error.lock().unwrap() = None;
    let status = state.mcp.status();
    let _ = app.emit("mcp:status", status.clone());
    status
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct PobMcp {
    ctx: Arc<ToolContext>,
    tasks: TaskManager,
    gate: Option<Arc<Gate>>,
}

fn supports_tasks(context: &RequestContext<RoleServer>) -> bool {
    context.client_capabilities().is_some_and(|c| c.supports_tasks())
}

/// MRTR arrived in 2026-07-28; rmcp refuses to send an `InputRequiredResult` to
/// anything older, so the confirmation is only offered where it can be answered.
fn supports_confirm(context: &RequestContext<RoleServer>) -> bool {
    context.protocol_version().is_some_and(|v| v >= ProtocolVersion::V_2026_07_28)
        && context.client_capabilities().is_some_and(|c| c.elicitation.is_some())
}

fn confirm_request(def: &ToolDef) -> InputRequiredResult {
    let schema = ElicitationSchema::builder().required_bool(CONFIRM_KEY).build();
    let mut requests = std::collections::BTreeMap::new();
    if let Ok(requested_schema) = schema {
        requests.insert(
            CONFIRM_KEY.to_string(),
            rmcp::model::InputRequest::Elicitation(rmcp::model::ElicitRequest::new(
                ElicitRequestParams::FormElicitationParams {
                    meta: None,
                    message: format!(
                        "{} discards work that cannot be recovered. Run it?",
                        def.name
                    ),
                    requested_schema,
                },
            )),
        );
    }
    // No request state: the retry carries the tool name and arguments again.
    InputRequiredResult::new(Some(requests), None)
}

/// True when the client came back with an accepted elicitation.
fn confirmed(response: &Value) -> bool {
    response.get("action").and_then(Value::as_str) == Some("accept")
        && response
            .get("content")
            .and_then(|c| c.get(CONFIRM_KEY))
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

fn to_response(out: Result<(Value, bool), ToolError>) -> Result<CallToolResult, ErrorData> {
    match out {
        Ok((value, _)) => Ok(CallToolResult::structured(value)),
        Err(ToolError::Invalid(msg)) => Err(ErrorData::invalid_params(msg, None)),
        Err(ToolError::Failed(msg)) => Ok(CallToolResult::error(vec![ContentBlock::text(msg)])),
    }
}

impl ServerHandler for PobMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().enable_tasks().build())
            .with_server_info(Implementation::new("pob-redux", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        versions()
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = match &self.gate {
            None => tool_list(),
            // Gated results are clipped text, so no output schema may promise structured content.
            Some(gate) => tool_list()
                .into_iter()
                .filter(|t| gate.lists(&t.name))
                .map(|mut t| {
                    t.output_schema = None;
                    t
                })
                .collect(),
        };
        Ok(ListToolsResult::with_all_items(tools)
            .with_ttl_ms(TOOL_LIST_TTL_MS)
            .with_cache_scope(CacheScope::Private))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let name = request.name.to_string();
        let args = request.arguments.unwrap_or_default();
        if let Some(gate) = &self.gate {
            let ct = context.ct.clone();
            let out = tokio::select! {
                out = gate.call(self.ctx.clone(), name, args) => out,
                _ = ct.cancelled() => Err("cancelled by the client".to_string()),
            };
            return Ok(match out {
                Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
                Err(msg) => CallToolResult::error(vec![ContentBlock::text(msg)]),
            }
            .into());
        }
        let def = defs().into_iter().find(|d| d.name == name);

        if let Some(def) = def.as_ref().filter(|d| d.destructive) {
            if supports_confirm(&context) {
                match request.input_responses.as_ref().and_then(|r| r.get(CONFIRM_KEY)) {
                    None => return Ok(CallToolResponse::InputRequired(confirm_request(def))),
                    Some(answer) if !confirmed(answer) => {
                        return Ok(CallToolResult::error(vec![ContentBlock::text(
                            "The user declined. Do not retry it; suggest an alternative or ask why.",
                        )])
                        .into());
                    }
                    Some(_) => {}
                }
            }
        }

        if def.as_ref().is_some_and(|d| d.slow) && supports_tasks(&context) {
            let ctx = self.ctx.clone();
            let task = self.tasks.spawn(
                TaskOptions::new()
                    .with_ttl_ms(TASK_TTL_MS)
                    .with_status_message(format!("running {name}")),
                move |tc| {
                    Box::pin(async move {
                        let out = dispatch(ctx, name, args).await;
                        // PoB is not interruptible mid-calculation, so a cancel
                        // is honoured at the boundary rather than during.
                        if tc.is_cancel_requested() {
                            return Err(TaskExit::Cancelled);
                        }
                        to_response(out).map_err(TaskExit::Error)
                    })
                },
            );
            return Ok(CallToolResponse::Task(CreateTaskResult::new(task)));
        }

        let ct = context.ct.clone();
        tokio::select! {
            out = dispatch(self.ctx.clone(), name, args) => to_response(out).map(Into::into),
            _ = ct.cancelled() => Err(ErrorData::invalid_request("cancelled by the client", None)),
        }
    }

    async fn get_task(
        &self,
        request: GetTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetTaskResult, ErrorData> {
        self.tasks.get_task(&request.task_id).map(GetTaskResult::new)
    }

    async fn update_task(
        &self,
        request: UpdateTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        self.tasks.update_task(&request.task_id, request.input_responses)
    }

    async fn cancel_task(
        &self,
        request: CancelTaskParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        self.tasks.cancel_task(&request.task_id)
    }
}

// ---------------------------------------------------------------------------
// Tool table
// ---------------------------------------------------------------------------

fn tool_list() -> Vec<Tool> {
    defs()
        .into_iter()
        .map(|d| {
            let schema: JsonObject = d.schema.as_object().cloned().unwrap_or_default();
            let mut tool = Tool::new(d.name, d.description, Arc::new(schema));
            if let Some(out) = d.output_schema.as_ref().and_then(Value::as_object) {
                tool = tool.with_raw_output_schema(Arc::new(out.clone()));
            }
            tool.annotations = Some(
                ToolAnnotations::new()
                    .read_only(d.read_only)
                    .destructive(d.destructive)
                    .idempotent(d.read_only || d.idempotent)
                    .open_world(d.open_world),
            );
            tool
        })
        .collect()
}

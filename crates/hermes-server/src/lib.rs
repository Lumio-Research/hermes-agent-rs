#![allow(clippy::doc_lazy_continuation, clippy::field_reassign_with_default)]
//! HTTP and WebSocket API server for Hermes.
//!
//! Environment (see also `security` module):
//! - `HERMES_HTTP_MAX_BODY_BYTES` — max JSON body size for POST routes (default 2 MiB).
//! - `HERMES_HTTP_CORS_ORIGINS` — comma-separated browser Origins for CORS; empty → permissive.
//! - `HERMES_SERVE_WEB_STATIC` — set `0`/`false`/`off` to skip serving `apps/dashboard/dist` (API-only).
//! Policy HTTP routes are intentionally omitted (Hermes Python does not expose them).

pub mod dashboard;
mod security;

pub use security::parse_allowed_ips;
pub use security::PolicyGuardConfig;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ws::{Message as WsMessage, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::header;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use futures::StreamExt;
use hermes_transport::{
    CreateSessionRequest, ListSessionsResponse, ProtocolMessage, RenameSessionRequest,
    SendMessageRequest, SendMessageResponse, SessionMessagesResponse, SessionSummary, StreamEvent,
    WsEnvelope,
};
use hermes_config::GatewayConfig;
use hermes_core::{AgentError, Message, MessageRole, StreamChunk};
use hermes_tools::ToolRegistry;
use serde::{Deserialize, Serialize};
use tower::service_fn;
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer};
use tower_http::services::ServeDir;

pub const HTTP_PLATFORM: &str = "http";

#[derive(Clone)]
pub struct HttpServerState {
    pub config: Arc<GatewayConfig>,
    pub tool_registry: Arc<ToolRegistry>,
    pub hermes_home: std::path::PathBuf,
    pub session_persistence: Arc<hermes_agent::session_persistence::SessionPersistence>,
    pub cron_scheduler: Option<Arc<hermes_cron::CronScheduler>>,
    pub skill_store: Option<Arc<dyn hermes_skills::SkillStore>>,
    pub runtime_gateway_running: Option<Arc<AtomicBool>>,
    agent_service: Arc<dyn hermes_core::traits::AgentService>,
}

impl HttpServerState {
    /// Build dashboard state using the default [`hermes_config::hermes_home`] resolution.
    pub async fn build(config: GatewayConfig) -> Result<Self, AgentError> {
        Self::build_with_hermes_home(config, hermes_config::hermes_home()).await
    }

    /// Build dashboard state with an explicit Hermes data directory (sessions DB, cron dir,
    /// `dashboard_plugins.json`, etc.). Useful for tests and alternate install layouts.
    pub async fn build_with_hermes_home(
        config: GatewayConfig,
        hermes_home: std::path::PathBuf,
    ) -> Result<Self, AgentError> {
        let tool_registry = Arc::new(ToolRegistry::new());
        // Register builtin tools (file, shell, browser, etc.)
        let terminal_backend: Arc<dyn hermes_core::TerminalBackend> =
            Arc::new(hermes_environments::LocalBackend::default());
        let skill_store = Arc::new(hermes_skills::FileSkillStore::new(
            hermes_skills::FileSkillStore::default_dir(),
        ));
        let skill_provider: Arc<dyn hermes_core::SkillProvider> =
            Arc::new(hermes_skills::SkillManager::new(skill_store));
        hermes_tools::register_builtin_tools(&tool_registry, terminal_backend, skill_provider);

        let session_persistence = Arc::new(
            hermes_agent::session_persistence::SessionPersistence::new(&hermes_home),
        );
        let _ = session_persistence.ensure_db();

        // Create LocalAgentService
        let agent_service: Arc<dyn hermes_core::traits::AgentService> =
            Arc::new(hermes_agent::LocalAgentService::new(
                Arc::new(config.clone()),
                tool_registry.clone(),
                session_persistence.clone(),
            ));

        // Initialize cron scheduler backed by `$hermes_home/cron`
        let cron_dir = hermes_home.join("cron");
        let cron_scheduler = Arc::new(hermes_cron::cli_support::cron_scheduler_for_data_dir(
            cron_dir,
        ));

        // Initialize skill store
        let skill_store: Arc<dyn hermes_skills::SkillStore> = Arc::new(
            hermes_skills::FileSkillStore::new(hermes_skills::FileSkillStore::default_dir()),
        );

        Ok(Self {
            config: Arc::new(config),
            tool_registry,
            hermes_home,
            session_persistence,
            cron_scheduler: Some(cron_scheduler),
            skill_store: Some(skill_store),
            runtime_gateway_running: None,
            agent_service,
        })
    }

    pub async fn build_with_agent_service(
        config: GatewayConfig,
        agent_service: Arc<dyn hermes_core::traits::AgentService>,
    ) -> Result<Self, AgentError> {
        let tool_registry = Arc::new(ToolRegistry::new());
        // Register builtin tools (file, shell, browser, etc.)
        let terminal_backend: Arc<dyn hermes_core::TerminalBackend> =
            Arc::new(hermes_environments::LocalBackend::default());
        let skill_store = Arc::new(hermes_skills::FileSkillStore::new(
            hermes_skills::FileSkillStore::default_dir(),
        ));
        let skill_provider: Arc<dyn hermes_core::SkillProvider> =
            Arc::new(hermes_skills::SkillManager::new(skill_store));
        hermes_tools::register_builtin_tools(&tool_registry, terminal_backend, skill_provider);

        let hermes_home = hermes_config::hermes_home();
        let session_persistence = Arc::new(
            hermes_agent::session_persistence::SessionPersistence::new(&hermes_home),
        );
        let _ = session_persistence.ensure_db();

        // Initialize cron scheduler backed by $HERMES_HOME/cron
        let cron_dir = hermes_home.join("cron");
        let cron_scheduler = Arc::new(hermes_cron::cli_support::cron_scheduler_for_data_dir(
            cron_dir,
        ));

        // Initialize skill store
        let skill_store: Arc<dyn hermes_skills::SkillStore> = Arc::new(
            hermes_skills::FileSkillStore::new(hermes_skills::FileSkillStore::default_dir()),
        );

        Ok(Self {
            config: Arc::new(config),
            tool_registry,
            hermes_home,
            session_persistence,
            cron_scheduler: Some(cron_scheduler),
            skill_store: Some(skill_store),
            runtime_gateway_running: None,
            agent_service,
        })
    }

    pub fn with_runtime_gateway_running(
        mut self,
        runtime_gateway_running: Arc<AtomicBool>,
    ) -> Self {
        self.runtime_gateway_running = Some(runtime_gateway_running);
        self
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub timestamp: String,
}

#[derive(Debug, Deserialize)]
pub struct CommandRequest {
    pub command: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CommandResponse {
    pub accepted: bool,
    pub output: String,
}

#[derive(Debug, Deserialize)]
pub struct ClientTelemetryEvent {
    pub runtime: String,
    pub level: String,
    pub message: String,
    #[serde(default)]
    pub app_version: Option<String>,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub tags: Option<serde_json::Value>,
}

fn max_request_body_bytes() -> usize {
    const DEFAULT: usize = 2 * 1024 * 1024;
    std::env::var("HERMES_HTTP_MAX_BODY_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT)
}

/// CORS: comma-separated allowlist in `HERMES_HTTP_CORS_ORIGINS` (e.g. `https://app.example.com`).
/// Empty / invalid → permissive (local dev / same-origin split defaults).
fn http_cors_layer() -> CorsLayer {
    let raw = std::env::var("HERMES_HTTP_CORS_ORIGINS").unwrap_or_default();
    let origins: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if origins.is_empty() {
        tracing::debug!("HERMES_HTTP_CORS_ORIGINS unset — using permissive CORS");
        return CorsLayer::permissive();
    }
    let mut header_values: Vec<HeaderValue> = Vec::new();
    for o in origins {
        match o.parse::<HeaderValue>() {
            Ok(v) => header_values.push(v),
            Err(_) => tracing::warn!(origin = %o, "invalid CORS origin, skipped"),
        }
    }
    if header_values.is_empty() {
        tracing::warn!("HERMES_HTTP_CORS_ORIGINS had no valid origins — permissive CORS");
        return CorsLayer::permissive();
    }
    tracing::info!(count = header_values.len(), "CORS allowlist enabled");
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(header_values))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(AllowHeaders::list([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
        ]))
}

pub fn router(state: HttpServerState) -> Router {
    let security = Arc::new(security::HttpSecurity::from_env());
    let rate = Arc::new(security::RateLimiter::new(security.rate_limit_per_minute));
    let sec_guard = security.clone();
    let rate_guard = rate.clone();
    let body_limit = max_request_body_bytes();

    let plugin_dir = state.hermes_home.join("dashboard-plugins");
    if let Err(e) = std::fs::create_dir_all(&plugin_dir) {
        tracing::warn!(
            error = %e,
            dir = %plugin_dir.display(),
            "could not create dashboard-plugins directory"
        );
    }

    let mut app = Router::new()
        .route("/health", get(health))
        .route("/v1/protocol/schema", get(protocol_schema))
        .route("/v1/telemetry/client-event", post(client_telemetry_event))
        .route("/metrics", get(metrics_prometheus))
        .route("/v1/sessions", get(list_sessions).post(create_session))
        .route(
            "/v1/sessions/{session_id}",
            post(rename_session).delete(delete_session),
        )
        .route(
            "/v1/sessions/{session_id}/messages",
            get(get_session_messages).post(send_message),
        )
        .route("/v1/commands", post(exec_command))
        .route("/v1/ws/{session_id}", get(ws_upgrade))
        .route("/v1/ws-stream/{session_id}", get(ws_stream_upgrade))
        // Dashboard management API
        .merge(dashboard::router())
        .nest_service("/dashboard-plugins", ServeDir::new(plugin_dir))
        .with_state(state);

    // Optional SPA static hosting (`HERMES_SERVE_WEB_STATIC=0` disables for Vercel-split deploy).
    let serve_web_static = !matches!(
        std::env::var("HERMES_SERVE_WEB_STATIC").as_deref(),
        Ok("0") | Ok("false") | Ok("no") | Ok("off")
    );

    if serve_web_static {
        // Serve the dashboard SPA from the `apps/dashboard/dist` directory if it exists.
        // Checks HERMES_WEB_DIST env var first, then common relative paths.
        let web_dist = std::env::var("HERMES_WEB_DIST")
            .ok()
            .map(std::path::PathBuf::from)
            .or_else(|| {
                let candidates = [
                    std::path::PathBuf::from("apps/dashboard/dist"),
                    std::path::PathBuf::from("../apps/dashboard/dist"),
                    std::path::PathBuf::from("../../apps/dashboard/dist"),
                ];
                candidates
                    .into_iter()
                    .find(|p| p.join("index.html").exists())
            });

        if let Some(dist_dir) = web_dist {
            let index_html = dist_dir.join("index.html");
            if index_html.exists() {
                tracing::info!("Serving web dashboard from {}", dist_dir.display());
                // SPA: static files from `dist/`, and missing paths fall back to `index.html`.
                // Use `fallback`, not `not_found_service` — the latter wraps responses in
                // `SetStatus(404)` per tower-http, which breaks client-side routes (browser shows
                // "404" even when HTML is present).
                let index_nf = Arc::new(index_html);
                let spa = ServeDir::new(&dist_dir).fallback(service_fn(
                    move |_req: axum::http::Request<Body>| {
                        let p = index_nf.clone();
                        async move {
                            let bytes = tokio::fs::read(&*p).await.unwrap_or_default();
                            let res = Response::builder()
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                                .body(Body::from(bytes))
                                .unwrap();
                            Ok::<_, Infallible>(res)
                        }
                    },
                ));
                app = app.fallback_service(spa);
            }
        }
    } else {
        tracing::info!("HERMES_SERVE_WEB_STATIC disables bundled SPA static hosting");
    }

    app.layer(middleware::from_fn(move |req, next| {
        let sec = sec_guard.clone();
        let rl = rate_guard.clone();
        async move { security::request_guard(sec, rl, req, next).await }
    }))
    .layer(tower_http::limit::RequestBodyLimitLayer::new(body_limit))
    .layer(tower_http::trace::TraceLayer::new_for_http())
    .layer(http_cors_layer())
}

pub async fn run_server(addr: SocketAddr, config: GatewayConfig) -> Result<(), AgentError> {
    let state = HttpServerState::build(config).await?;
    run_server_with_state(addr, state).await
}

pub async fn run_server_with_state(
    addr: SocketAddr,
    state: HttpServerState,
) -> Result<(), AgentError> {
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| AgentError::Io(e.to_string()))?;
    tracing::info!("hermes-server listening on {}", addr);
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("hermes-server graceful shutdown");
    };
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
    .map_err(|e| AgentError::Io(e.to_string()))
}

async fn health() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok",
        timestamp: Utc::now().to_rfc3339(),
    })
}

async fn metrics_prometheus() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        hermes_telemetry::prometheus_text(),
    )
}

async fn protocol_schema() -> impl IntoResponse {
    Json(serde_json::json!({
        "version": 1,
        "ws_envelope": hermes_transport::ws_schema_json()
    }))
}

async fn client_telemetry_event(
    Json(event): Json<ClientTelemetryEvent>,
) -> Result<Json<serde_json::Value>, HttpError> {
    match event.level.as_str() {
        "error" => tracing::error!(
            runtime = %event.runtime,
            message = %event.message,
            app_version = ?event.app_version,
            trace_id = ?event.trace_id,
            tags = ?event.tags,
            "client telemetry event"
        ),
        "warn" => tracing::warn!(
            runtime = %event.runtime,
            message = %event.message,
            app_version = ?event.app_version,
            trace_id = ?event.trace_id,
            tags = ?event.tags,
            "client telemetry event"
        ),
        _ => tracing::info!(
            runtime = %event.runtime,
            message = %event.message,
            app_version = ?event.app_version,
            trace_id = ?event.trace_id,
            tags = ?event.tags,
            "client telemetry event"
        ),
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn list_sessions(State(state): State<HttpServerState>) -> Json<ListSessionsResponse> {
    let db_path = state.hermes_home.join("sessions.db");
    let conn = match rusqlite::Connection::open(db_path) {
        Ok(conn) => conn,
        Err(_) => {
            return Json(ListSessionsResponse {
                sessions: vec![],
                total: 0,
            });
        }
    };
    let mut stmt = match conn.prepare(
        "SELECT id, title, created_at, updated_at, message_count FROM sessions ORDER BY updated_at DESC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => {
            return Json(ListSessionsResponse {
                sessions: vec![],
                total: 0,
            });
        }
    };
    let sessions: Vec<SessionSummary> = stmt
        .query_map([], |row| {
            Ok(SessionSummary {
                id: row.get::<_, String>(0)?,
                title: row
                    .get::<_, Option<String>>(1)?
                    .unwrap_or_else(|| "New chat".to_string()),
                created_at: row.get::<_, String>(2)?,
                updated_at: row.get::<_, String>(3)?,
                message_count: row.get::<_, i64>(4).unwrap_or(0) as usize,
            })
        })
        .map(|rows| rows.filter_map(Result::ok).collect())
        .unwrap_or_default();
    Json(ListSessionsResponse {
        total: sessions.len(),
        sessions,
    })
}

async fn create_session(
    State(state): State<HttpServerState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<SessionSummary>, HttpError> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let title = if req.title.trim().is_empty() {
        "New chat".to_string()
    } else {
        req.title
    };
    state
        .session_persistence
        .persist_session(
            &session_id,
            &[] as &[Message],
            state.config.model.as_deref(),
            Some(HTTP_PLATFORM),
            Some(&title),
            None,
        )
        .map_err(HttpError::from)?;
    Ok(Json(SessionSummary {
        id: session_id,
        title,
        created_at: now.clone(),
        updated_at: now,
        message_count: 0,
    }))
}

async fn rename_session(
    Path(session_id): Path<String>,
    State(state): State<HttpServerState>,
    Json(req): Json<RenameSessionRequest>,
) -> Result<Json<serde_json::Value>, HttpError> {
    let messages = state
        .session_persistence
        .load_session(&session_id)
        .map_err(HttpError::from)?;
    state
        .session_persistence
        .persist_session(
            &session_id,
            &messages,
            state.config.model.as_deref(),
            Some(HTTP_PLATFORM),
            Some(req.title.trim()),
            None,
        )
        .map_err(HttpError::from)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn delete_session(
    Path(session_id): Path<String>,
    State(state): State<HttpServerState>,
) -> Result<Json<serde_json::Value>, HttpError> {
    let db_path = state.hermes_home.join("sessions.db");
    let conn = rusqlite::Connection::open(db_path).map_err(|e| HttpError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: e.to_string(),
    })?;
    conn.execute(
        "DELETE FROM messages WHERE session_id = ?1",
        rusqlite::params![session_id],
    )
    .map_err(|e| HttpError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: e.to_string(),
    })?;
    conn.execute(
        "DELETE FROM sessions WHERE id = ?1",
        rusqlite::params![session_id],
    )
    .map_err(|e| HttpError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: e.to_string(),
    })?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn get_session_messages(
    Path(session_id): Path<String>,
    State(state): State<HttpServerState>,
) -> Result<Json<SessionMessagesResponse>, HttpError> {
    let messages = state
        .session_persistence
        .load_session(&session_id)
        .map_err(HttpError::from)?
        .into_iter()
        .enumerate()
        .map(|(idx, m)| ProtocolMessage {
            id: format!("{}-{}", session_id, idx),
            role: match m.role {
                MessageRole::User => hermes_transport::Role::User,
                MessageRole::Assistant => hermes_transport::Role::Assistant,
                MessageRole::System => hermes_transport::Role::System,
                MessageRole::Tool => hermes_transport::Role::Tool,
            },
            content: m.content.unwrap_or_default(),
            timestamp: Utc::now().to_rfc3339(),
            model: None,
        })
        .collect();
    Ok(Json(SessionMessagesResponse {
        session_id,
        messages,
    }))
}

async fn send_message(
    Path(session_id): Path<String>,
    State(state): State<HttpServerState>,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, HttpError> {
    hermes_telemetry::record_http_request();
    let trace_id = hermes_transport::new_trace_id();

    // Build AgentOverrides from request
    let overrides = hermes_core::traits::AgentOverrides {
        model: req.model.clone(),
        personality: req.personality.clone(),
    };

    // Call agent service
    let reply = state
        .agent_service
        .send_message(&session_id, &req.text, overrides)
        .await
        .map_err(|e| HttpError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        })?;

    Ok(Json(SendMessageResponse {
        session_id,
        reply: reply.text,
        message_count: reply.message_count,
        trace_id,
    }))
}

async fn exec_command(
    State(state): State<HttpServerState>,
    Json(req): Json<CommandRequest>,
) -> Result<Json<CommandResponse>, HttpError> {
    hermes_telemetry::record_http_request();
    let trimmed = req.command.trim();
    if trimmed.is_empty() {
        return Ok(Json(CommandResponse {
            accepted: false,
            output: "empty command".to_string(),
        }));
    }

    let cmd = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    };

    let session_id = req
        .session_id
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "default".to_string());

    // For now, we'll just send the command as a regular message
    // In a real implementation, we'd need to handle slash commands specially
    let overrides = hermes_core::traits::AgentOverrides::default();
    let reply = state
        .agent_service
        .send_message(&session_id, &cmd, overrides)
        .await
        .map_err(|e| HttpError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        })?;

    Ok(Json(CommandResponse {
        accepted: true,
        output: reply.text,
    }))
}

async fn ws_upgrade(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    State(state): State<HttpServerState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws(socket, state, session_id))
}

async fn handle_ws(mut socket: WebSocket, state: HttpServerState, session_id: String) {
    let _ = socket
        .send(WsMessage::Text(
            format!("connected session={}", session_id).into(),
        ))
        .await;
    while let Some(Ok(msg)) = socket.next().await {
        match msg {
            WsMessage::Text(text) => {
                let parsed: Option<SendMessageRequest> = serde_json::from_str(&text).ok();
                let request = parsed.unwrap_or_else(|| SendMessageRequest {
                    text: text.to_string(),
                    model: None,
                    provider: None,
                    personality: None,
                    user_id: None,
                    request_id: None,
                });

                // Build AgentOverrides from request
                let overrides = hermes_core::traits::AgentOverrides {
                    model: request.model.clone(),
                    personality: request.personality.clone(),
                };

                // Call agent service
                match state
                    .agent_service
                    .send_message(&session_id, &request.text, overrides)
                    .await
                {
                    Ok(reply) => {
                        let _ = socket.send(WsMessage::Text(reply.text.into())).await;
                    }
                    Err(err) => {
                        let _ = socket.send(WsMessage::Text(err.to_string().into())).await;
                    }
                }
            }
            WsMessage::Close(_) => break,
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Streaming WebSocket endpoint — real-time Agent events
// ---------------------------------------------------------------------------

/// WebSocket upgrade for streaming endpoint.
async fn ws_stream_upgrade(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    State(state): State<HttpServerState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws_stream(socket, state, session_id))
}

/// Streaming WebSocket handler.
async fn handle_ws_stream(mut socket: WebSocket, state: HttpServerState, session_id: String) {
    use tokio::sync::mpsc;
    let trace_id_base = hermes_transport::new_trace_id();
    let connected = WsEnvelope {
        version: 1,
        request_id: None,
        trace_id: trace_id_base.clone(),
        event: StreamEvent::Connected {
            session_id: session_id.clone(),
        },
    };
    let _ = socket
        .send(WsMessage::Text(
            serde_json::to_string(&connected).unwrap_or_default().into(),
        ))
        .await;

    while let Some(Ok(msg)) = socket.next().await {
        match msg {
            WsMessage::Text(text) => {
                let trace_id = trace_id_base.clone();
                let parsed: Option<SendMessageRequest> = serde_json::from_str(&text).ok();
                let request = parsed.unwrap_or_else(|| SendMessageRequest {
                    text: text.to_string(),
                    model: None,
                    provider: None,
                    personality: None,
                    user_id: None,
                    request_id: None,
                });

                // Build AgentOverrides from request
                let overrides = hermes_core::traits::AgentOverrides {
                    model: request.model.clone(),
                    personality: request.personality.clone(),
                };

                // Create a bounded queue to avoid unbounded memory usage under slow clients.
                let queue_cap = std::env::var("HERMES_WS_STREAM_BUFFER")
                    .ok()
                    .and_then(|v| v.parse::<usize>().ok())
                    .filter(|v| *v > 0)
                    .unwrap_or(128);
                let (tx, mut rx) = mpsc::channel::<WsEnvelope>(queue_cap);
                let tx_clone = tx.clone();
                let request_id = request.request_id.clone();
                let trace_id_for_stream = trace_id.clone();
                let on_chunk = Arc::new(move |chunk: StreamChunk| {
                    if let Some(delta) = &chunk.delta {
                        if let Some(text) = &delta.content {
                            if !text.is_empty() {
                                let _ = tx_clone.try_send(WsEnvelope {
                                    version: 1,
                                    request_id: request_id.clone(),
                                    trace_id: trace_id_for_stream.clone(),
                                    event: StreamEvent::Text {
                                        content: redact_sensitive(text),
                                    },
                                });
                            }
                        }
                    }
                });

                // Call agent service with streaming
                let agent_service = state.agent_service.clone();
                let session_id_clone = session_id.clone();
                let request_text = request.text.clone();

                tokio::spawn(async move {
                    match agent_service
                        .send_message_stream(&session_id_clone, &request_text, overrides, on_chunk)
                        .await
                    {
                        Ok(reply) => {
                            let _ = tx
                                .send(WsEnvelope {
                                    version: 1,
                                    request_id: request.request_id.clone(),
                                    trace_id: trace_id.clone(),
                                    event: StreamEvent::Done {
                                        content: redact_sensitive(&reply.text),
                                    },
                                })
                                .await;
                        }
                        Err(e) => {
                            let _ = tx
                                .send(WsEnvelope {
                                    version: 1,
                                    request_id: request.request_id.clone(),
                                    trace_id: trace_id.clone(),
                                    event: StreamEvent::Error {
                                        code: "agent_error".to_string(),
                                        message: redact_sensitive(&e.to_string()),
                                    },
                                })
                                .await;
                        }
                    }
                });

                // Forward channel events to WebSocket
                while let Some(event_json) = rx.recv().await {
                    if socket
                        .send(WsMessage::Text(
                            serde_json::to_string(&event_json)
                                .unwrap_or_default()
                                .into(),
                        ))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
            WsMessage::Close(_) => break,
            _ => {}
        }
    }
}

fn redact_sensitive(input: &str) -> String {
    let mut out = input.to_string();
    let patterns = [
        ("sk-", "sk-***"),
        ("ghp_", "ghp_***"),
        ("xoxb-", "xoxb-***"),
    ];
    for (needle, replacement) in patterns {
        if out.contains(needle) {
            out = out.replace(needle, replacement);
        }
    }
    out
}

#[derive(Debug)]
pub struct HttpError {
    pub status: StatusCode,
    pub message: String,
}

impl From<AgentError> for HttpError {
    fn from(value: AgentError) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: value.to_string(),
        }
    }
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.status, self.message)
    }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}

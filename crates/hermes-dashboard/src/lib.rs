#![allow(clippy::doc_lazy_continuation, clippy::field_reassign_with_default)]
//! HTTP and WebSocket API server for Hermes.
//!
//! Environment (see also `security` module):
//! - `HERMES_HTTP_MAX_BODY_BYTES` — max JSON body size for POST routes (default 2 MiB).
//! Policy HTTP routes are intentionally omitted (Hermes Python does not expose them).

mod security;
pub mod dashboard;

pub use security::parse_allowed_ips;
pub use security::PolicyGuardConfig;

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use axum::extract::ws::{Message as WsMessage, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::header;
use axum::http::StatusCode;
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use futures::StreamExt;
use hermes_config::GatewayConfig;
use hermes_core::{AgentError, StreamChunk};
use hermes_tools::ToolRegistry;
use serde::{Deserialize, Serialize};

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
    pub async fn build(config: GatewayConfig) -> Result<Self, AgentError> {
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

        // Create LocalAgentService
        let agent_service: Arc<dyn hermes_core::traits::AgentService> = Arc::new(
            hermes_agent::LocalAgentService::new(
                Arc::new(config.clone()),
                tool_registry.clone(),
                session_persistence.clone(),
            ),
        );

        // Initialize cron scheduler backed by $HERMES_HOME/cron
        let cron_dir = hermes_home.join("cron");
        let cron_scheduler = Arc::new(
            hermes_cron::cli_support::cron_scheduler_for_data_dir(cron_dir),
        );

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
        let cron_scheduler = Arc::new(
            hermes_cron::cli_support::cron_scheduler_for_data_dir(cron_dir),
        );

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

    pub fn with_runtime_gateway_running(mut self, runtime_gateway_running: Arc<AtomicBool>) -> Self {
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
pub struct SendMessageRequest {
    pub text: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub personality: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub session_id: String,
    pub reply: String,
    pub message_count: usize,
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

fn max_request_body_bytes() -> usize {
    const DEFAULT: usize = 2 * 1024 * 1024;
    std::env::var("HERMES_HTTP_MAX_BODY_BYTES")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT)
}

pub fn router(state: HttpServerState) -> Router {
    let security = Arc::new(security::HttpSecurity::from_env());
    let rate = Arc::new(security::RateLimiter::new(security.rate_limit_per_minute));
    let sec_guard = security.clone();
    let rate_guard = rate.clone();
    let body_limit = max_request_body_bytes();

    let mut app = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics_prometheus))
        .route("/v1/sessions/{session_id}/messages", post(send_message))
        .route("/v1/commands", post(exec_command))
        .route("/v1/ws/{session_id}", get(ws_upgrade))
        .route("/v1/ws-stream/{session_id}", get(ws_stream_upgrade))
        // Dashboard management API
        .merge(dashboard::router())
        .with_state(state);

    // Serve the web dashboard SPA from the `apps/web/dist` directory if it exists.
    // Checks HERMES_WEB_DIST env var first, then common relative paths.
    let web_dist = std::env::var("HERMES_WEB_DIST")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            // Try relative to the binary location
            let candidates = [
                std::path::PathBuf::from("apps/web/dist"),
                std::path::PathBuf::from("../apps/web/dist"),
                std::path::PathBuf::from("../../apps/web/dist"),
            ];
            candidates.into_iter().find(|p| p.join("index.html").exists())
        });

    if let Some(dist_dir) = web_dist {
        if dist_dir.join("index.html").exists() {
            tracing::info!("Serving web dashboard from {}", dist_dir.display());
            // Serve static assets from dist/.  For SPA client-side routing, any path
            // that doesn't match a real file should return index.html with 200.
            let index_html = dist_dir.join("index.html");
            let index_bytes: &'static [u8] = Box::leak(
                std::fs::read(&index_html).unwrap_or_default().into_boxed_slice(),
            );
            app = app.fallback_service(
                tower_http::services::ServeDir::new(&dist_dir)
                    .fallback(get(move || async move {
                        (
                            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                            index_bytes,
                        )
                    })),
            );
        }
    }

    app.layer(middleware::from_fn(move |req, next| {
            let sec = sec_guard.clone();
            let rl = rate_guard.clone();
            async move { security::request_guard(sec, rl, req, next).await }
        }))
        .layer(tower_http::limit::RequestBodyLimitLayer::new(body_limit))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(tower_http::cors::CorsLayer::permissive())
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
    tracing::info!("hermes-dashboard listening on {}", addr);
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("hermes-dashboard graceful shutdown");
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

async fn send_message(
    Path(session_id): Path<String>,
    State(state): State<HttpServerState>,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, HttpError> {
    hermes_telemetry::record_http_request();

    // Build AgentOverrides from request
    let overrides = hermes_core::traits::AgentOverrides {
        model: req.model.clone(),
        personality: req.personality.clone(),
    };

    // Call agent service
    let reply = state.agent_service.send_message(&session_id, &req.text, overrides)
        .await
        .map_err(|e| HttpError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        })?;

    Ok(Json(SendMessageResponse {
        session_id,
        reply: reply.text,
        message_count: reply.message_count,
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
    let reply = state.agent_service.send_message(&session_id, &cmd, overrides)
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
                });
                
                // Build AgentOverrides from request
                let overrides = hermes_core::traits::AgentOverrides {
                    model: request.model.clone(),
                    personality: request.personality.clone(),
                };
                
                // Call agent service
                match state.agent_service.send_message(&session_id, &request.text, overrides).await {
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
///
/// Protocol:
/// - Client sends: `{"text": "...", "user_id": "..."}` (same as SendMessageRequest)
/// - Server pushes JSON events:
///   `{"type": "text", "content": "token..."}`
///   `{"type": "thinking", "content": "reasoning..."}`
///   `{"type": "tool_start", "tool": "name", "content": "description"}`
///   `{"type": "tool_complete", "tool": "name", "content": "result"}`
///   `{"type": "status", "content": "message"}`
///   `{"type": "activity", "content": "…"}` (at most once per silence for “still waiting”, then optionally once more if stalled)
///   `{"type": "done", "content": "full_reply"}`
///   `{"type": "error", "content": "error message"}`
async fn handle_ws_stream(
    mut socket: WebSocket,
    state: HttpServerState,
    session_id: String,
) {
    use tokio::sync::mpsc;

    // Send connected event
    let _ = socket
        .send(WsMessage::Text(
            serde_json::json!({"type": "connected", "session_id": session_id}).to_string().into(),
        ))
        .await;

    while let Some(Ok(msg)) = socket.next().await {
        match msg {
            WsMessage::Text(text) => {
                let parsed: Option<SendMessageRequest> =
                    serde_json::from_str(&text).ok();
                let request = parsed.unwrap_or_else(|| SendMessageRequest {
                    text: text.to_string(),
                    model: None,
                    provider: None,
                    personality: None,
                    user_id: None,
                });

                // Build AgentOverrides from request
                let overrides = hermes_core::traits::AgentOverrides {
                    model: request.model.clone(),
                    personality: request.personality.clone(),
                };

                // Create a simple streaming callback that forwards text chunks
                let (tx, mut rx) = mpsc::unbounded_channel::<String>();
                let tx_clone = tx.clone();
                let on_chunk = Arc::new(move |chunk: StreamChunk| {
                    if let Some(delta) = &chunk.delta {
                        if let Some(text) = &delta.content {
                            if !text.is_empty() {
                                let _ = tx_clone.send(
                                    serde_json::json!({"type": "text", "content": text}).to_string(),
                                );
                            }
                        }
                    }
                });

                // Call agent service with streaming
                let agent_service = state.agent_service.clone();
                let session_id_clone = session_id.clone();
                let request_text = request.text.clone();
                
                tokio::spawn(async move {
                    match agent_service.send_message_stream(&session_id_clone, &request_text, overrides, on_chunk).await {
                        Ok(reply) => {
                            let _ = tx.send(
                                serde_json::json!({"type": "done", "content": reply.text}).to_string(),
                            );
                        }
                        Err(e) => {
                            let _ = tx.send(
                                serde_json::json!({"type": "error", "content": e.to_string()}).to_string(),
                            );
                        }
                    }
                });

                // Forward channel events to WebSocket
                while let Some(event_json) = rx.recv().await {
                    if socket
                        .send(WsMessage::Text(event_json.into()))
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










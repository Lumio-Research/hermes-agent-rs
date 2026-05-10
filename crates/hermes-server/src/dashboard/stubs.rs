//! Stub endpoints for APIs the frontend expects but are not yet fully implemented.
//!
//! These return valid JSON so the SPA doesn't choke on HTML fallback responses.

use std::path::Path;

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::types::OkResponse;
use crate::HttpServerState;

// ── Dashboard plugins ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardPluginSettings {
    pub mcp_filesystem: bool,
    pub mcp_terminal: bool,
    pub mcp_browser: bool,
    pub mcp_database: bool,
    pub tool_code_exec: bool,
}

impl Default for DashboardPluginSettings {
    fn default() -> Self {
        Self {
            mcp_filesystem: true,
            mcp_terminal: true,
            mcp_browser: false,
            mcp_database: false,
            tool_code_exec: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct DashboardPluginState {
    plugins: DashboardPluginSettings,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardPluginResponse {
    plugins: DashboardPluginSettings,
    persisted: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DashboardPluginUpdate {
    pub plugins: DashboardPluginSettings,
}

fn plugin_state_path(state: &HttpServerState) -> std::path::PathBuf {
    state.hermes_home.join("dashboard_plugins.json")
}

fn load_plugin_state(state: &HttpServerState) -> (DashboardPluginState, bool) {
    let path = plugin_state_path(state);
    let data = match std::fs::read_to_string(path) {
        Ok(v) => v,
        Err(_) => return (DashboardPluginState::default(), false),
    };
    match serde_json::from_str::<DashboardPluginState>(&data) {
        Ok(v) => (v, true),
        Err(_) => (DashboardPluginState::default(), false),
    }
}

fn save_plugin_state(state: &HttpServerState, body: &DashboardPluginState) -> Result<(), String> {
    let path = plugin_state_path(state);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(body).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// GET /api/dashboard/plugins — return persisted plugin settings.
pub async fn get_plugins(State(state): State<HttpServerState>) -> Json<DashboardPluginResponse> {
    let (plugin_state, persisted) = load_plugin_state(&state);
    Json(DashboardPluginResponse {
        plugins: plugin_state.plugins,
        persisted,
    })
}

/// PUT /api/dashboard/plugins — persist plugin settings for remote mode.
pub async fn set_plugins(
    State(state): State<HttpServerState>,
    Json(body): Json<DashboardPluginUpdate>,
) -> Json<serde_json::Value> {
    let payload = DashboardPluginState {
        plugins: body.plugins,
    };
    match save_plugin_state(&state, &payload) {
        Ok(_) => Json(serde_json::json!({
            "ok": true,
            "plugins": payload.plugins,
            "persisted": true
        })),
        Err(err) => Json(serde_json::json!({
            "ok": false,
            "error": err
        })),
    }
}

/// `$HERMES_HOME/dashboard-plugins` — each subdir may contain `manifest.json` (SPA contract).
fn plugin_bundles_root(hermes_home: &Path) -> std::path::PathBuf {
    hermes_home.join("dashboard-plugins")
}

/// Scan `dashboard-plugins/<id>/manifest.json` and return JSON values (with `name` defaulted to dir id).
pub fn discover_plugin_manifests(hermes_home: &Path) -> Vec<Value> {
    let dir = plugin_bundles_root(hermes_home);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest_path = path.join("manifest.json");
        if !manifest_path.is_file() {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(mut v) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let plugin_id = entry.file_name().to_string_lossy().to_string();
        if let Value::Object(ref mut m) = v {
            m.entry("name".to_string())
                .or_insert(Value::String(plugin_id.clone()));
        }
        out.push(v);
    }
    out.sort_by(|a, b| {
        let sa = a.get("name").and_then(|x| x.as_str()).unwrap_or("");
        let sb = b.get("name").and_then(|x| x.as_str()).unwrap_or("");
        sa.cmp(sb)
    });
    out
}

/// POST /api/dashboard/plugins/rescan — returns count of discovered local plugin manifests.
pub async fn rescan_plugins(State(state): State<HttpServerState>) -> Json<Value> {
    let count = discover_plugin_manifests(&state.hermes_home).len();
    Json(json!({ "ok": true, "count": count }))
}

/// GET /api/dashboard/plugin-manifests — JSON array from `dashboard-plugins/*/manifest.json`.
pub async fn get_plugin_manifests(State(state): State<HttpServerState>) -> Json<Vec<Value>> {
    Json(discover_plugin_manifests(&state.hermes_home))
}

// ── Dashboard themes ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DashboardThemeState {
    /// Active theme id; must match a built-in `name` below.
    active: String,
}

impl Default for DashboardThemeState {
    fn default() -> Self {
        Self {
            active: "default".to_string(),
        }
    }
}

fn theme_state_path(state: &HttpServerState) -> std::path::PathBuf {
    state.hermes_home.join("dashboard_theme.json")
}

fn builtin_theme_summaries() -> Vec<serde_json::Value> {
    vec![
        json!({ "name": "default",      "label": "Default",     "description": "Pure black with white text — industrial tech aesthetic" }),
        json!({ "name": "hermes-teal",  "label": "Hermes Teal", "description": "Classic dark teal — the canonical Hermes look" }),
        json!({ "name": "midnight",     "label": "Midnight",    "description": "Deep blue-violet with cool accents" }),
        json!({ "name": "ember",        "label": "Ember",       "description": "Warm crimson and bronze — forge vibes" }),
        json!({ "name": "mono",         "label": "Mono",        "description": "Clean grayscale — minimal and focused" }),
        json!({ "name": "cyberpunk",    "label": "Cyberpunk",   "description": "Neon green on black — matrix terminal" }),
        json!({ "name": "rose",         "label": "Rosé",        "description": "Soft pink and warm ivory — easy on the eyes" }),
    ]
}

fn builtin_theme_names() -> &'static [&'static str] {
    &[
        "default",
        "hermes-teal",
        "midnight",
        "ember",
        "mono",
        "cyberpunk",
        "rose",
    ]
}

fn is_builtin_theme(name: &str) -> bool {
    builtin_theme_names().contains(&name)
}

fn load_theme_state(state: &HttpServerState) -> (DashboardThemeState, bool) {
    let path = theme_state_path(state);
    let data = match std::fs::read_to_string(path) {
        Ok(v) => v,
        Err(_) => return (DashboardThemeState::default(), false),
    };
    match serde_json::from_str::<DashboardThemeState>(&data) {
        Ok(mut v) => {
            if !is_builtin_theme(v.active.as_str()) {
                v.active = "default".to_string();
            }
            (v, true)
        }
        Err(_) => (DashboardThemeState::default(), false),
    }
}

fn save_theme_state(state: &HttpServerState, body: &DashboardThemeState) -> Result<(), String> {
    let path = theme_state_path(state);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(body).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// GET /api/dashboard/themes — built-in theme list + persisted `active` (Python parity surface).
pub async fn get_themes(State(state): State<HttpServerState>) -> Json<serde_json::Value> {
    let (theme_state, persisted) = load_theme_state(&state);
    Json(json!({
        "active": theme_state.active,
        "persisted": persisted,
        "themes": builtin_theme_summaries(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct SetThemeBody {
    pub name: String,
}

/// PUT /api/dashboard/theme — persist active theme under `HERMES_HOME/dashboard_theme.json`.
pub async fn set_theme(
    State(state): State<HttpServerState>,
    Json(body): Json<SetThemeBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let name = body.name.trim();
    let name = if name.is_empty() { "default" } else { name };
    if !is_builtin_theme(name) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "ok": false,
                "error": format!("unknown theme '{name}'"),
            })),
        ));
    }
    let payload = DashboardThemeState {
        active: name.to_string(),
    };
    match save_theme_state(&state, &payload) {
        Ok(_) => Ok(Json(json!({ "ok": true, "theme": name }))),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": err })),
        )),
    }
}

// ── OAuth providers ─────────────────────────────────────────────────

/// GET /api/providers/oauth — return empty OAuth provider list.
pub async fn get_oauth_providers(State(_state): State<HttpServerState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "providers": [],
        "connected": []
    }))
}

/// POST /api/providers/oauth/{provider_id}/disconnect — legacy no-op disconnect.
pub async fn disconnect_oauth_provider(State(_state): State<HttpServerState>) -> Json<OkResponse> {
    Json(OkResponse { ok: true })
}

/// DELETE /api/providers/oauth/{provider_id} — SPA uses this path (Python parity).
pub async fn delete_oauth_provider(
    AxumPath(provider_id): AxumPath<String>,
) -> Json<serde_json::Value> {
    Json(json!({
        "ok": true,
        "provider": provider_id
    }))
}

/// POST /api/providers/oauth/{provider_id}/start — stub device-code session (not a real OAuth server).
pub async fn oauth_start(AxumPath(provider_id): AxumPath<String>) -> Json<serde_json::Value> {
    Json(json!({
        "session_id": format!("stub-{provider_id}"),
        "flow": "device_code",
        "user_code": "NOT-IMPL",
        "verification_url": "https://hermes.invalid/oauth-not-implemented",
        "expires_in": 300,
        "poll_interval": 2
    }))
}

#[derive(Debug, Deserialize)]
pub struct OAuthSubmitBody {
    pub session_id: String,
    pub code: String,
}

/// POST /api/providers/oauth/{provider_id}/submit — stub; real token exchange not implemented here.
pub async fn oauth_submit(
    AxumPath(_provider_id): AxumPath<String>,
    Json(body): Json<OAuthSubmitBody>,
) -> Json<serde_json::Value> {
    Json(json!({
        "ok": false,
        "status": "error",
        "message": format!(
            "OAuth code exchange is not implemented on the Rust dashboard server (session_id={}, code_len={}).",
            body.session_id,
            body.code.len()
        )
    }))
}

/// GET /api/providers/oauth/{provider_id}/poll/{session_id}
pub async fn oauth_poll(
    AxumPath((_provider_id, session_id)): AxumPath<(String, String)>,
) -> Json<serde_json::Value> {
    Json(json!({
        "session_id": session_id,
        "status": "error",
        "error_message": "OAuth polling is not implemented on the Rust dashboard server.",
        "expires_at": null
    }))
}

/// DELETE /api/providers/oauth/sessions/{session_id}
pub async fn oauth_cancel_session(AxumPath(_session_id): AxumPath<String>) -> Json<OkResponse> {
    Json(OkResponse { ok: true })
}

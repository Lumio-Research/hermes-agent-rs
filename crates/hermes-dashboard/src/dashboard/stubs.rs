//! Stub endpoints for APIs the frontend expects but are not yet fully implemented.
//!
//! These return valid JSON so the SPA doesn't choke on HTML fallback responses.

use axum::extract::State;
use axum::Json;

use crate::HttpServerState;
use super::types::OkResponse;

// ── Dashboard plugins ───────────────────────────────────────────────

/// GET /api/dashboard/plugins — return empty plugin list.
pub async fn get_plugins(
    State(_state): State<HttpServerState>,
) -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

/// POST /api/dashboard/plugins/rescan — no-op rescan.
pub async fn rescan_plugins(
    State(_state): State<HttpServerState>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true, "count": 0 }))
}

// ── Dashboard themes ────────────────────────────────────────────────

/// GET /api/dashboard/themes — return built-in theme list.
///
/// The frontend has its own built-in presets; this endpoint lets the backend
/// override the active theme. For now we return the built-in names and let
/// the frontend's localStorage pick win.
pub async fn get_themes(
    State(_state): State<HttpServerState>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "active": "default",
        "themes": [
            { "name": "default",      "label": "Default",     "description": "Pure black with white text — industrial tech aesthetic" },
            { "name": "hermes-teal",  "label": "Hermes Teal", "description": "Classic dark teal — the canonical Hermes look" },
            { "name": "midnight",     "label": "Midnight",    "description": "Deep blue-violet with cool accents" },
            { "name": "ember",        "label": "Ember",       "description": "Warm crimson and bronze — forge vibes" },
            { "name": "mono",         "label": "Mono",        "description": "Clean grayscale — minimal and focused" },
            { "name": "cyberpunk",    "label": "Cyberpunk",   "description": "Neon green on black — matrix terminal" },
            { "name": "rose",         "label": "Rosé",        "description": "Soft pink and warm ivory — easy on the eyes" },
        ]
    }))
}

/// PUT /api/dashboard/theme — accept theme change (persisted client-side).
pub async fn set_theme(
    State(_state): State<HttpServerState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let name = body["name"].as_str().unwrap_or("default");
    Json(serde_json::json!({ "ok": true, "theme": name }))
}

// ── OAuth providers ─────────────────────────────────────────────────

/// GET /api/providers/oauth — return empty OAuth provider list.
pub async fn get_oauth_providers(
    State(_state): State<HttpServerState>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "providers": [],
        "connected": []
    }))
}

/// POST /api/providers/oauth/{provider_id}/disconnect — no-op disconnect.
pub async fn disconnect_oauth_provider(
    State(_state): State<HttpServerState>,
) -> Json<OkResponse> {
    Json(OkResponse { ok: true })
}

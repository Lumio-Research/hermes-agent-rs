//! Dashboard management API endpoints.
//!
//! These endpoints mirror the Python `hermes_cli/web_server.py` API surface,
//! providing the `/api/*` routes consumed by the React web dashboard.

pub mod types;
pub mod status;
pub mod sessions;
pub mod config;
pub mod env;
pub mod logs;
pub mod cron;
pub mod skills;
pub mod analytics;
mod stubs;

use axum::routing::{delete, get, post, put};
use axum::Router;

use crate::HttpServerState;

/// Build the dashboard API router.
///
/// All routes are prefixed with `/api/` and share the same `HttpServerState`.
pub fn router() -> Router<HttpServerState> {
    Router::new()
        // Status
        .route("/api/status", get(status::get_status))
        // Sessions
        .route("/api/sessions", get(sessions::list_sessions))
        .route("/api/sessions/search", get(sessions::search_sessions))
        .route("/api/sessions/{session_id}", get(sessions::get_session_detail).delete(sessions::delete_session))
        .route("/api/sessions/{session_id}/messages", get(sessions::get_session_messages))
        // Config
        .route("/api/config", get(config::get_config).put(config::update_config))
        .route("/api/config/defaults", get(config::get_defaults))
        .route("/api/config/schema", get(config::get_schema))
        .route("/api/config/raw", get(config::get_config_raw).put(config::update_config_raw))
        // Model info
        .route("/api/model/info", get(config::get_model_info))
        // Env
        .route("/api/env", get(env::get_env_vars).put(env::set_env_var).delete(env::delete_env_var))
        .route("/api/env/reveal", post(env::reveal_env_var))
        // Logs
        .route("/api/logs", get(logs::get_logs))
        // Cron
        .route("/api/cron/jobs", get(cron::list_jobs).post(cron::create_job))
        .route("/api/cron/jobs/{job_id}", delete(cron::delete_job))
        .route("/api/cron/jobs/{job_id}/pause", post(cron::pause_job))
        .route("/api/cron/jobs/{job_id}/resume", post(cron::resume_job))
        .route("/api/cron/jobs/{job_id}/trigger", post(cron::trigger_job))
        // Skills & Tools
        .route("/api/skills", get(skills::list_skills))
        .route("/api/skills/toggle", put(skills::toggle_skill))
        .route("/api/tools/toolsets", get(skills::list_toolsets))
        // Analytics
        .route("/api/analytics/usage", get(analytics::get_usage))
        // Dashboard plugins & themes (stubs — not yet fully implemented)
        .route("/api/dashboard/plugins", get(stubs::get_plugins))
        .route("/api/dashboard/plugins/rescan", post(stubs::rescan_plugins))
        .route("/api/dashboard/themes", get(stubs::get_themes))
        .route("/api/dashboard/theme", put(stubs::set_theme))
        // OAuth providers (stub)
        .route("/api/providers/oauth", get(stubs::get_oauth_providers))
        .route("/api/providers/oauth/{provider_id}/disconnect", post(stubs::disconnect_oauth_provider))
}

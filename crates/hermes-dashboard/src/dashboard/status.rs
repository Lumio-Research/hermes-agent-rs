//! GET /api/status — agent status overview.

use axum::extract::State;
use axum::Json;
use std::collections::HashMap;
use std::fs;

use crate::HttpServerState;
use super::types::StatusResponse;

pub async fn get_status(
    State(state): State<HttpServerState>,
) -> Json<StatusResponse> {
    let hermes_home = state.hermes_home.display().to_string();
    let config_path = state.hermes_home.join("config.yaml").display().to_string();
    let env_path = state.hermes_home.join(".env").display().to_string();

    // Check if gateway is running via PID file
    let pid_file_path = state.hermes_home.join("gateway.pid");
    let (gateway_running, gateway_pid) = if pid_file_path.exists() {
        if let Ok(pid_str) = fs::read_to_string(&pid_file_path) {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                // Check if process is alive (Unix-specific)
                #[cfg(unix)]
                let is_alive = unsafe { libc::kill(pid as i32, 0) == 0 };
                #[cfg(not(unix))]
                let is_alive = false; // On non-Unix, assume not running
                
                if is_alive {
                    (true, Some(pid))
                } else {
                    (false, Some(pid))
                }
            } else {
                (false, None)
            }
        } else {
            (false, None)
        }
    } else {
        (false, None)
    };

    // Query active session count from SQLite directly
    let db_path = state.hermes_home.join("sessions.db");
    let active_sessions = if db_path.exists() {
        if let Ok(conn) = rusqlite::Connection::open(&db_path) {
            conn.query_row("SELECT COUNT(DISTINCT id) FROM sessions", [], |row| {
                row.get::<_, u32>(0)
            }).unwrap_or(0)
        } else {
            0
        }
    } else {
        0
    };

    // Gateway platforms - empty for now since we're decoupled
    let gateway_platforms = HashMap::new();

    Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        release_date: String::new(),
        hermes_home,
        config_path,
        env_path,
        config_version: 1,
        latest_config_version: 1,
        active_sessions,
        gateway_running,
        gateway_pid,
        gateway_state: if gateway_running { Some("running".to_string()) } else { None },
        gateway_health_url: None,
        gateway_exit_reason: None,
        gateway_updated_at: None,
        gateway_platforms,
    })
}

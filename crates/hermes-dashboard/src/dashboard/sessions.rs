//! Session management endpoints.
//!
//! Queries the SQLite `sessions.db` created by `SessionPersistence`.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

use crate::{HttpError, HttpServerState};
use super::types::*;

#[derive(Debug, Deserialize)]
pub struct ListParams {
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 { 20 }

/// GET /api/sessions
pub async fn list_sessions(
    State(state): State<HttpServerState>,
    Query(params): Query<ListParams>,
) -> Json<PaginatedSessions> {
    let db_path = state.hermes_home.join("sessions.db");
    let (sessions, total) = query_sessions(&db_path, params.limit, params.offset).unwrap_or_default();

    Json(PaginatedSessions {
        sessions,
        total,
        limit: params.limit,
        offset: params.offset,
    })
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    #[serde(default)]
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

/// GET /api/sessions/search
pub async fn search_sessions(
    State(state): State<HttpServerState>,
    Query(params): Query<SearchParams>,
) -> Json<SessionSearchResponse> {
    let db_path = state.hermes_home.join("sessions.db");
    let results = query_search(&db_path, &params.q, params.limit).unwrap_or_default();
    Json(SessionSearchResponse { results })
}

/// GET /api/sessions/{session_id}
pub async fn get_session_detail(
    State(state): State<HttpServerState>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionInfo>, HttpError> {
    let db_path = state.hermes_home.join("sessions.db");
    query_session_by_id(&db_path, &session_id)
        .map(Json)
        .map_err(|e| HttpError {
            status: axum::http::StatusCode::NOT_FOUND,
            message: e,
        })
}

/// GET /api/sessions/{session_id}/messages
pub async fn get_session_messages(
    State(state): State<HttpServerState>,
    Path(session_id): Path<String>,
) -> Json<SessionMessagesResponse> {
    let messages = state.session_persistence.load_session(&session_id)
        .unwrap_or_default()
        .into_iter()
        .map(|m| {
            let tool_calls = m.tool_calls.map(|tcs| {
                tcs.into_iter().map(|tc| ToolCallInfo {
                    id: tc.id.clone(),
                    function: ToolCallFunction {
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                    },
                }).collect()
            });
            SessionMessage {
                role: format!("{:?}", m.role).to_lowercase(),
                content: m.content,
                tool_calls,
                tool_name: m.name.clone(),
                tool_call_id: m.tool_call_id,
                timestamp: None,
            }
        })
        .collect();

    Json(SessionMessagesResponse { session_id, messages })
}

/// DELETE /api/sessions/{session_id}
pub async fn delete_session(
    State(state): State<HttpServerState>,
    Path(session_id): Path<String>,
) -> Json<OkResponse> {
    let db_path = state.hermes_home.join("sessions.db");
    let ok = delete_session_from_db(&db_path, &session_id).is_ok();
    Json(OkResponse { ok })
}

// ── SQLite helpers ──────────────────────────────────────────────────

fn query_sessions(
    db_path: &std::path::Path,
    limit: u32,
    offset: u32,
) -> Result<(Vec<SessionInfo>, u32), String> {
    let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;

    let total: u32 = conn
        .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
        .unwrap_or(0);

    let mut stmt = conn
        .prepare(
            "SELECT id, model, platform, created_at, updated_at, title, message_count
             FROM sessions
             ORDER BY updated_at DESC
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(|e| e.to_string())?;

    let sessions = stmt
        .query_map(rusqlite::params![limit, offset], |row| {
            let id: String = row.get(0)?;
            let model: Option<String> = row.get(1)?;
            let platform: Option<String> = row.get(2)?;
            let created_at: String = row.get(3)?;
            let updated_at: String = row.get(4)?;
            let title: Option<String> = row.get(5)?;
            let message_count: u32 = row.get::<_, i64>(6).unwrap_or(0) as u32;

            let started_ts = chrono::DateTime::parse_from_rfc3339(&created_at)
                .map(|d| d.timestamp() as f64)
                .unwrap_or(0.0);
            let updated_ts = chrono::DateTime::parse_from_rfc3339(&updated_at)
                .map(|d| d.timestamp() as f64)
                .unwrap_or(0.0);

            Ok(SessionInfo {
                id,
                source: platform,
                model,
                title,
                started_at: started_ts,
                ended_at: None,
                last_active: updated_ts,
                is_active: false,
                message_count,
                tool_call_count: 0,
                input_tokens: 0,
                output_tokens: 0,
                preview: None,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok((sessions, total))
}

fn query_session_by_id(db_path: &std::path::Path, session_id: &str) -> Result<SessionInfo, String> {
    let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;

    conn.query_row(
        "SELECT id, model, platform, created_at, updated_at, title, message_count
         FROM sessions WHERE id = ?1",
        rusqlite::params![session_id],
        |row| {
            let id: String = row.get(0)?;
            let model: Option<String> = row.get(1)?;
            let platform: Option<String> = row.get(2)?;
            let created_at: String = row.get(3)?;
            let updated_at: String = row.get(4)?;
            let title: Option<String> = row.get(5)?;
            let message_count: u32 = row.get::<_, i64>(6).unwrap_or(0) as u32;

            let started_ts = chrono::DateTime::parse_from_rfc3339(&created_at)
                .map(|d| d.timestamp() as f64)
                .unwrap_or(0.0);
            let updated_ts = chrono::DateTime::parse_from_rfc3339(&updated_at)
                .map(|d| d.timestamp() as f64)
                .unwrap_or(0.0);

            Ok(SessionInfo {
                id,
                source: platform,
                model,
                title,
                started_at: started_ts,
                ended_at: None,
                last_active: updated_ts,
                is_active: false,
                message_count,
                tool_call_count: 0,
                input_tokens: 0,
                output_tokens: 0,
                preview: None,
            })
        },
    )
    .map_err(|e| format!("session not found: {}", e))
}

fn query_search(
    db_path: &std::path::Path,
    query: &str,
    limit: u32,
) -> Result<Vec<SessionSearchResult>, String> {
    let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;

    // Use FTS5 for full-text search
    let mut stmt = conn
        .prepare(
            "SELECT m.session_id, snippet(messages_fts, 0, '>>>', '<<<', '...', 48) as snip,
                    m.role, s.platform, s.model, s.created_at
             FROM messages_fts AS mf
             JOIN messages AS m ON m.id = mf.rowid
             JOIN sessions AS s ON s.id = m.session_id
             WHERE messages_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;

    let results = stmt
        .query_map(rusqlite::params![query, limit], |row| {
            let session_id: String = row.get(0)?;
            let snippet: String = row.get(1)?;
            let role: Option<String> = row.get(2)?;
            let source: Option<String> = row.get(3)?;
            let model: Option<String> = row.get(4)?;
            let created_at: Option<String> = row.get(5)?;
            let session_started = created_at.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .map(|d| d.timestamp() as f64)
                    .ok()
            });

            Ok(SessionSearchResult {
                session_id,
                snippet,
                role,
                source,
                model,
                session_started,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}

fn delete_session_from_db(db_path: &std::path::Path, session_id: &str) -> Result<(), String> {
    let conn = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM messages WHERE session_id = ?1", rusqlite::params![session_id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM sessions WHERE id = ?1", rusqlite::params![session_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

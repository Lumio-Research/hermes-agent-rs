//! Smoke tests for the dashboard management API endpoints.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

/// Helper: build a default test app.
async fn test_app() -> axum::Router {
    let _ = tracing_subscriber::fmt::try_init();
    let cfg = hermes_config::GatewayConfig::default();
    let state = hermes_dashboard::HttpServerState::build(cfg).await.unwrap();
    hermes_dashboard::router(state)
}

/// Helper: GET request and return (status, body_json).
async fn get_json(app: axum::Router, uri: &str) -> (StatusCode, serde_json::Value) {
    let res = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value =
        serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, json)
}

// ── /api/status ─────────────────────────────────────────────────────

#[tokio::test]
async fn status_returns_version() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/status").await;
    assert_eq!(status, StatusCode::OK);
    // Must contain version field matching Cargo.toml
    let version = body["version"].as_str().unwrap();
    assert!(!version.is_empty(), "version should not be empty");
    // Must contain hermes_home path
    assert!(body["hermes_home"].is_string());
    assert!(body["config_path"].is_string());
    assert!(body["env_path"].is_string());
    // gateway_running should be a boolean
    assert!(body["gateway_running"].is_boolean());
}

// ── /api/sessions ───────────────────────────────────────────────────

#[tokio::test]
async fn sessions_list_returns_paginated() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/sessions").await;
    assert_eq!(status, StatusCode::OK);
    // Should have pagination fields
    assert!(body["sessions"].is_array());
    assert!(body["total"].is_number());
    assert!(body["limit"].is_number());
    assert!(body["offset"].is_number());
}

#[tokio::test]
async fn sessions_list_respects_limit_offset() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/sessions?limit=5&offset=0").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["limit"].as_u64().unwrap(), 5);
    assert_eq!(body["offset"].as_u64().unwrap(), 0);
}

#[tokio::test]
async fn session_detail_not_found() {
    let app = test_app().await;
    let (status, _body) = get_json(app, "/api/sessions/nonexistent-id-12345").await;
    // Should return 404 for a session that doesn't exist
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn session_messages_returns_empty_for_unknown() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/sessions/unknown-session/messages").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["session_id"].as_str().unwrap(), "unknown-session");
    assert!(body["messages"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn session_search_returns_results_array() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/sessions/search?q=hello").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["results"].is_array());
}

// ── /api/config ─────────────────────────────────────────────────────

#[tokio::test]
async fn config_get_returns_json() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/config").await;
    assert_eq!(status, StatusCode::OK);
    // Should be a JSON object (the serialized GatewayConfig)
    assert!(body.is_object());
}

#[tokio::test]
async fn config_defaults_returns_json() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/config/defaults").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_object());
}

#[tokio::test]
async fn config_schema_returns_fields() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/config/schema").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["category_order"].is_array());
}

#[tokio::test]
async fn config_raw_returns_yaml() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/config/raw").await;
    assert_eq!(status, StatusCode::OK);
    // Should have a "yaml" field (may be empty string if no config file)
    assert!(body["yaml"].is_string());
}

// ── /api/model/info ─────────────────────────────────────────────────

#[tokio::test]
async fn model_info_returns_provider_and_model() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/model/info").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["model"].is_string());
    assert!(body["provider"].is_string());
    assert!(body["effective_context_length"].is_number());
    assert!(body["capabilities"].is_object());
}

// ── /api/env ────────────────────────────────────────────────────────

#[tokio::test]
async fn env_vars_returns_known_keys() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/env").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_object());
    // Should contain at least OPENAI_API_KEY
    assert!(body["OPENAI_API_KEY"].is_object());
    assert!(body["ANTHROPIC_API_KEY"].is_object());
    // Each entry should have expected fields
    let openai = &body["OPENAI_API_KEY"];
    assert!(openai["is_set"].is_boolean());
    assert!(openai["description"].is_string());
    assert!(openai["category"].is_string());
}

// ── /api/logs ───────────────────────────────────────────────────────

#[tokio::test]
async fn logs_returns_lines_array() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/logs").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["file"].is_string());
    assert!(body["lines"].is_array());
}

#[tokio::test]
async fn logs_accepts_params() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/logs?file=agent&lines=10&level=ERROR").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["file"].as_str().unwrap(), "agent");
}

// ── /api/cron/jobs ──────────────────────────────────────────────────

#[tokio::test]
async fn cron_jobs_unavailable_without_scheduler() {
    let app = test_app().await;
    let (status, _body) = get_json(app, "/api/cron/jobs").await;
    assert_eq!(status, StatusCode::OK);
}

// ── /api/skills ─────────────────────────────────────────────────────

#[tokio::test]
async fn skills_list_returns_array() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/skills").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_array());
}

// ── /api/tools/toolsets ─────────────────────────────────────────────

#[tokio::test]
async fn toolsets_list_returns_array() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/tools/toolsets").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_array());
    // Should have registered builtin tools
    let arr = body.as_array().unwrap();
    assert!(!arr.is_empty(), "toolsets should not be empty after builtin registration");
}

// ── /api/analytics/usage ────────────────────────────────────────────

#[tokio::test]
async fn analytics_usage_returns_structure() {
    let app = test_app().await;
    let (status, body) = get_json(app, "/api/analytics/usage").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["daily"].is_array());
    assert!(body["by_model"].is_array());
    assert!(body["totals"].is_object());
    assert!(body["skills"].is_object());
}

// ── DELETE /api/sessions/{id} ───────────────────────────────────────

#[tokio::test]
async fn delete_session_returns_ok() {
    let app = test_app().await;
    let res = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/sessions/nonexistent-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // Even for non-existent session, delete should succeed gracefully
    // (the DB may not exist, but the handler should not panic)
    let status = res.status();
    assert!(
        status == StatusCode::OK || status == StatusCode::NOT_FOUND,
        "unexpected status: {}",
        status
    );
}

// ── PUT /api/config/raw ─────────────────────────────────────────────

#[tokio::test]
async fn update_config_raw_rejects_invalid_yaml() {
    let app = test_app().await;
    let payload = serde_json::json!({ "yaml_text": "{{invalid yaml" });
    let res = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/config/raw")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

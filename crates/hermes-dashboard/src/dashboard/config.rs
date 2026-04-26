//! Configuration management endpoints.

use axum::extract::State;
use axum::Json;

use crate::{HttpError, HttpServerState};
use super::types::*;

/// GET /api/config — return current config as JSON.
pub async fn get_config(
    State(state): State<HttpServerState>,
) -> Json<serde_json::Value> {
    let config = hermes_config::load_config(Some(state.hermes_home.to_str().unwrap_or_default()))
        .map(|c| serde_json::to_value(c).unwrap_or_default())
        .unwrap_or_default();
    Json(config)
}

/// GET /api/config/defaults — return default config values.
pub async fn get_defaults(
    State(state): State<HttpServerState>,
) -> Json<serde_json::Value> {
    let _ = &state;
    let defaults = serde_json::to_value(hermes_config::GatewayConfig::default())
        .unwrap_or_default();
    Json(defaults)
}

/// GET /api/config/schema — return config field schema for the form editor.
pub async fn get_schema(
    State(state): State<HttpServerState>,
) -> Json<serde_json::Value> {
    let _ = &state;
    // Build a basic schema from the GatewayConfig struct.
    // A full implementation would introspect the config fields.
    Json(serde_json::json!({
        "fields": {},
        "category_order": ["general", "agent", "terminal", "display", "security"]
    }))
}

/// PUT /api/config — update config from JSON.
pub async fn update_config(
    State(state): State<HttpServerState>,
    Json(body): Json<ConfigUpdate>,
) -> Result<Json<OkResponse>, HttpError> {
    let config_path = state.hermes_home.join("config.yaml");
    let yaml = serde_yaml::to_string(&body.config)
        .map_err(|e| HttpError {
            status: axum::http::StatusCode::BAD_REQUEST,
            message: format!("invalid config: {}", e),
        })?;
    std::fs::write(&config_path, yaml)
        .map_err(|e| HttpError {
            status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("failed to write config: {}", e),
        })?;
    Ok(Json(OkResponse { ok: true }))
}

/// GET /api/config/raw — return raw YAML config.
pub async fn get_config_raw(
    State(state): State<HttpServerState>,
) -> Result<Json<serde_json::Value>, HttpError> {
    let config_path = state.hermes_home.join("config.yaml");
    let yaml = std::fs::read_to_string(&config_path).unwrap_or_default();
    Ok(Json(serde_json::json!({ "yaml": yaml })))
}

/// PUT /api/config/raw — save raw YAML config.
pub async fn update_config_raw(
    State(state): State<HttpServerState>,
    Json(body): Json<RawConfigUpdate>,
) -> Result<Json<OkResponse>, HttpError> {
    // Validate YAML before saving
    let _: serde_yaml::Value = serde_yaml::from_str(&body.yaml_text)
        .map_err(|e| HttpError {
            status: axum::http::StatusCode::BAD_REQUEST,
            message: format!("invalid YAML: {}", e),
        })?;

    let config_path = state.hermes_home.join("config.yaml");
    std::fs::write(&config_path, &body.yaml_text)
        .map_err(|e| HttpError {
            status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("failed to write config: {}", e),
        })?;
    Ok(Json(OkResponse { ok: true }))
}

/// GET /api/model/info — return current model information.
pub async fn get_model_info(
    State(state): State<HttpServerState>,
) -> Json<ModelInfoResponse> {
    let model = state.config.model.clone().unwrap_or_else(|| "openai:gpt-4o".to_string());
    let (provider, model_name) = model.split_once(':').unwrap_or(("openai", &model));

    Json(ModelInfoResponse {
        model: model_name.to_string(),
        provider: provider.to_string(),
        auto_context_length: 128000,
        config_context_length: 0,
        effective_context_length: 128000,
        capabilities: serde_json::json!({
            "supports_tools": true,
            "supports_vision": true,
            "context_window": 128000,
        }),
    })
}

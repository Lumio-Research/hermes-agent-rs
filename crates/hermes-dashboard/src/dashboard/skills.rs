//! Skills and toolset management endpoints.

use axum::extract::State;
use axum::Json;

use crate::{HttpError, HttpServerState};
use super::types::*;

/// GET /api/skills
pub async fn list_skills(
    State(state): State<HttpServerState>,
) -> Json<Vec<SkillInfo>> {
    let Some(store) = state.skill_store.as_ref() else {
        return Json(vec![]);
    };

    let metas = store.list().await.unwrap_or_default();
    Json(metas.into_iter().map(|m| SkillInfo {
        name: m.name,
        description: m.description.unwrap_or_default(),
        category: m.category.unwrap_or_default(),
        enabled: true, // file-based skills are always enabled if present
    }).collect())
}

/// PUT /api/skills/toggle
pub async fn toggle_skill(
    State(state): State<HttpServerState>,
    Json(body): Json<SkillToggle>,
) -> Result<Json<OkResponse>, HttpError> {
    let Some(store) = state.skill_store.as_ref() else {
        return Err(HttpError {
            status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
            message: "skill store not configured".to_string(),
        });
    };

    if !body.enabled {
        // "Disable" by deleting the skill file
        store.delete(&body.name).await.map_err(|e| HttpError {
            status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        })?;
    }
    // Enabling a deleted skill would require re-downloading — not implemented yet

    Ok(Json(OkResponse { ok: true }))
}

/// GET /api/tools/toolsets
pub async fn list_toolsets(
    State(state): State<HttpServerState>,
) -> Json<Vec<ToolsetInfo>> {
    let defs = state.tool_registry.get_definitions();
    let toolsets: Vec<ToolsetInfo> = defs.iter().map(|d| ToolsetInfo {
        name: d.name.clone(),
        label: d.name.clone(),
        description: d.description.clone(),
        enabled: true,
        configured: true,
        tools: vec![d.name.clone()],
    }).collect();
    Json(toolsets)
}

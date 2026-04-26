//! Usage analytics endpoint.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::HttpServerState;
use super::types::*;

#[derive(Debug, Deserialize)]
pub struct AnalyticsParams {
    #[serde(default = "default_days")]
    pub days: u32,
}

fn default_days() -> u32 { 30 }

/// GET /api/analytics/usage
pub async fn get_usage(
    State(_state): State<HttpServerState>,
    Query(_params): Query<AnalyticsParams>,
) -> Json<AnalyticsResponse> {
    // TODO: aggregate from session persistence store
    Json(AnalyticsResponse {
        daily: vec![],
        by_model: vec![],
        totals: AnalyticsTotals {
            total_input: 0,
            total_output: 0,
            total_cache_read: 0,
            total_reasoning: 0,
            total_estimated_cost: 0.0,
            total_actual_cost: 0.0,
            total_sessions: 0,
        },
        skills: AnalyticsSkillsResponse {
            summary: AnalyticsSkillsSummary {
                total_skill_loads: 0,
                total_skill_edits: 0,
                total_skill_actions: 0,
                distinct_skills_used: 0,
            },
            top_skills: vec![],
        },
    })
}

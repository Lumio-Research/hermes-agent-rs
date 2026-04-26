//! Log viewing endpoint.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use std::io::{BufRead, BufReader};

use crate::HttpServerState;
use super::types::LogsResponse;

#[derive(Debug, Deserialize)]
pub struct LogParams {
    #[serde(default = "default_file")]
    pub file: String,
    #[serde(default = "default_lines")]
    pub lines: usize,
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default)]
    pub component: Option<String>,
}

fn default_file() -> String { "agent".to_string() }
fn default_lines() -> usize { 100 }

/// GET /api/logs
pub async fn get_logs(
    State(state): State<HttpServerState>,
    Query(params): Query<LogParams>,
) -> Json<LogsResponse> {
    let log_dir = state.hermes_home.join("logs");
    let log_file = log_dir.join(format!("{}.log", params.file));

    let lines = if log_file.exists() {
        read_tail(&log_file, params.lines, params.level.as_deref(), params.component.as_deref())
    } else {
        vec![]
    };

    Json(LogsResponse {
        file: params.file,
        lines,
    })
}

/// Read the last N lines from a file, optionally filtering by level/component.
fn read_tail(
    path: &std::path::Path,
    max_lines: usize,
    level: Option<&str>,
    component: Option<&str>,
) -> Vec<String> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return vec![],
    };

    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().map_while(Result::ok).collect();

    let filtered: Vec<String> = all_lines
        .into_iter()
        .filter(|line| {
            if let Some(lvl) = level {
                if lvl != "ALL" {
                    let upper = line.to_uppercase();
                    if !upper.contains(lvl) {
                        return false;
                    }
                }
            }
            if let Some(comp) = component {
                if comp != "all" && !line.to_lowercase().contains(comp) {
                    return false;
                }
            }
            true
        })
        .collect();

    // Return last N lines
    let start = filtered.len().saturating_sub(max_lines);
    filtered[start..].to_vec()
}

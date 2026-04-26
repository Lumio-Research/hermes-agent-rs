//! Shared request/response types for the dashboard API.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Status ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct PlatformStatus {
    pub state: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub version: String,
    pub release_date: String,
    pub hermes_home: String,
    pub config_path: String,
    pub env_path: String,
    pub config_version: u32,
    pub latest_config_version: u32,
    pub active_sessions: u32,
    pub gateway_running: bool,
    pub gateway_pid: Option<u32>,
    pub gateway_state: Option<String>,
    pub gateway_health_url: Option<String>,
    pub gateway_exit_reason: Option<String>,
    pub gateway_updated_at: Option<String>,
    pub gateway_platforms: HashMap<String, PlatformStatus>,
}

// ── Sessions ────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub source: Option<String>,
    pub model: Option<String>,
    pub title: Option<String>,
    pub started_at: f64,
    pub ended_at: Option<f64>,
    pub last_active: f64,
    pub is_active: bool,
    pub message_count: u32,
    pub tool_call_count: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub preview: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedSessions {
    pub sessions: Vec<SessionInfo>,
    pub total: u32,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Serialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct ToolCallInfo {
    pub id: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Serialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Serialize)]
pub struct SessionMessagesResponse {
    pub session_id: String,
    pub messages: Vec<SessionMessage>,
}

#[derive(Debug, Serialize)]
pub struct SessionSearchResult {
    pub session_id: String,
    pub snippet: String,
    pub role: Option<String>,
    pub source: Option<String>,
    pub model: Option<String>,
    pub session_started: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct SessionSearchResponse {
    pub results: Vec<SessionSearchResult>,
}

// ── Config ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ConfigUpdate {
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct RawConfigUpdate {
    pub yaml_text: String,
}

#[derive(Debug, Serialize)]
pub struct ModelInfoResponse {
    pub model: String,
    pub provider: String,
    pub auto_context_length: u32,
    pub config_context_length: u32,
    pub effective_context_length: u32,
    pub capabilities: serde_json::Value,
}

// ── Env ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct EnvVarInfo {
    pub is_set: bool,
    pub redacted_value: Option<String>,
    pub description: String,
    pub url: Option<String>,
    pub category: String,
    pub is_password: bool,
    pub tools: Vec<String>,
    pub advanced: bool,
}

#[derive(Debug, Deserialize)]
pub struct EnvVarUpdate {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct EnvVarDelete {
    pub key: String,
}

#[derive(Debug, Deserialize)]
pub struct EnvVarReveal {
    pub key: String,
}

// ── Logs ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct LogsResponse {
    pub file: String,
    pub lines: Vec<String>,
}

// ── Cron ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct CronJobResponse {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub prompt: String,
    pub schedule: CronScheduleInfo,
    pub schedule_display: String,
    pub enabled: bool,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deliver: Option<String>,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CronScheduleInfo {
    pub kind: String,
    pub expr: String,
    pub display: String,
}

#[derive(Debug, Deserialize)]
pub struct CronJobCreate {
    pub prompt: String,
    pub schedule: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub deliver: Option<String>,
}

// ── Skills ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub category: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct SkillToggle {
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct ToolsetInfo {
    pub name: String,
    pub label: String,
    pub description: String,
    pub enabled: bool,
    pub configured: bool,
    pub tools: Vec<String>,
}

// ── Analytics ───────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AnalyticsDailyEntry {
    pub day: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub reasoning_tokens: u64,
    pub estimated_cost: f64,
    pub actual_cost: f64,
    pub sessions: u32,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsModelEntry {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub estimated_cost: f64,
    pub sessions: u32,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsTotals {
    pub total_input: u64,
    pub total_output: u64,
    pub total_cache_read: u64,
    pub total_reasoning: u64,
    pub total_estimated_cost: f64,
    pub total_actual_cost: f64,
    pub total_sessions: u32,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsSkillEntry {
    pub skill: String,
    pub view_count: u32,
    pub manage_count: u32,
    pub total_count: u32,
    pub percentage: f64,
    pub last_used_at: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsSkillsSummary {
    pub total_skill_loads: u32,
    pub total_skill_edits: u32,
    pub total_skill_actions: u32,
    pub distinct_skills_used: u32,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsResponse {
    pub daily: Vec<AnalyticsDailyEntry>,
    pub by_model: Vec<AnalyticsModelEntry>,
    pub totals: AnalyticsTotals,
    pub skills: AnalyticsSkillsResponse,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsSkillsResponse {
    pub summary: AnalyticsSkillsSummary,
    pub top_skills: Vec<AnalyticsSkillEntry>,
}

// ── Common ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct OkResponse {
    pub ok: bool,
}

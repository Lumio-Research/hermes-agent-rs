//! Cron job management endpoints.

use axum::extract::{Path, State};
use axum::Json;

use crate::{HttpError, HttpServerState};
use super::types::*;

fn no_scheduler() -> HttpError {
    HttpError {
        status: axum::http::StatusCode::SERVICE_UNAVAILABLE,
        message: "cron scheduler not configured".to_string(),
    }
}

/// GET /api/cron/jobs
pub async fn list_jobs(
    State(state): State<HttpServerState>,
) -> Result<Json<Vec<CronJobResponse>>, HttpError> {
    let sched = state.cron_scheduler.as_ref().ok_or_else(no_scheduler)?;
    let jobs = sched.list_jobs().await;
    Ok(Json(jobs.into_iter().map(into_cron_response).collect()))
}

/// POST /api/cron/jobs
pub async fn create_job(
    State(state): State<HttpServerState>,
    Json(body): Json<CronJobCreate>,
) -> Result<Json<CronJobResponse>, HttpError> {
    let sched = state.cron_scheduler.as_ref().ok_or_else(no_scheduler)?;
    let mut job = hermes_cron::CronJob::new(&body.schedule, &body.prompt);
    job.name = body.name;
    if let Some(deliver) = body.deliver {
        let target = match deliver.as_str() {
            "local" => hermes_cron::DeliverTarget::Local,
            "telegram" => hermes_cron::DeliverTarget::Telegram,
            "discord" => hermes_cron::DeliverTarget::Discord,
            "slack" => hermes_cron::DeliverTarget::Slack,
            "email" => hermes_cron::DeliverTarget::Email,
            _ => hermes_cron::DeliverTarget::Local,
        };
        job.deliver = Some(hermes_cron::DeliverConfig {
            target,
            platform: None,
        });
    }
    let _id = sched.create_job(job.clone()).await.map_err(|e| HttpError {
        status: axum::http::StatusCode::BAD_REQUEST,
        message: e.to_string(),
    })?;
    Ok(Json(into_cron_response(job)))
}

/// POST /api/cron/jobs/{job_id}/pause
pub async fn pause_job(
    State(state): State<HttpServerState>,
    Path(job_id): Path<String>,
) -> Result<Json<OkResponse>, HttpError> {
    let sched = state.cron_scheduler.as_ref().ok_or_else(no_scheduler)?;
    sched.pause_job(&job_id).await.map_err(|e| HttpError {
        status: axum::http::StatusCode::NOT_FOUND,
        message: e.to_string(),
    })?;
    Ok(Json(OkResponse { ok: true }))
}

/// POST /api/cron/jobs/{job_id}/resume
pub async fn resume_job(
    State(state): State<HttpServerState>,
    Path(job_id): Path<String>,
) -> Result<Json<OkResponse>, HttpError> {
    let sched = state.cron_scheduler.as_ref().ok_or_else(no_scheduler)?;
    sched.resume_job(&job_id).await.map_err(|e| HttpError {
        status: axum::http::StatusCode::NOT_FOUND,
        message: e.to_string(),
    })?;
    Ok(Json(OkResponse { ok: true }))
}

/// POST /api/cron/jobs/{job_id}/trigger
pub async fn trigger_job(
    State(state): State<HttpServerState>,
    Path(job_id): Path<String>,
) -> Result<Json<OkResponse>, HttpError> {
    let sched = state.cron_scheduler.as_ref().ok_or_else(no_scheduler)?;
    sched.run_job(&job_id).await.map_err(|e| HttpError {
        status: axum::http::StatusCode::NOT_FOUND,
        message: e.to_string(),
    })?;
    Ok(Json(OkResponse { ok: true }))
}

/// DELETE /api/cron/jobs/{job_id}
pub async fn delete_job(
    State(state): State<HttpServerState>,
    Path(job_id): Path<String>,
) -> Result<Json<OkResponse>, HttpError> {
    let sched = state.cron_scheduler.as_ref().ok_or_else(no_scheduler)?;
    sched.remove_job(&job_id).await.map_err(|e| HttpError {
        status: axum::http::StatusCode::NOT_FOUND,
        message: e.to_string(),
    })?;
    Ok(Json(OkResponse { ok: true }))
}

fn into_cron_response(job: hermes_cron::CronJob) -> CronJobResponse {
    let deliver_str = job.deliver.as_ref().map(|d| format!("{:?}", d.target).to_lowercase());

    CronJobResponse {
        id: job.id.clone(),
        name: job.name.clone(),
        prompt: job.prompt.clone(),
        schedule: CronScheduleInfo {
            kind: "cron".to_string(),
            expr: job.schedule.clone(),
            display: job.schedule.clone(),
        },
        schedule_display: job.schedule.clone(),
        enabled: job.status == hermes_cron::JobStatus::Active,
        state: job.status.to_string(),
        deliver: deliver_str,
        last_run_at: job.last_run.map(|t| t.to_rfc3339()),
        next_run_at: job.next_run.map(|t| t.to_rfc3339()),
        last_error: None,
    }
}

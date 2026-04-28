//! Video generation backend: SeedDance2 API.
//!
//! SeedDance2 is an async video generation service — submit a task, then
//! poll for completion. This backend handles both the submission and
//! status-check flows.
//!
//! Supports:
//! - Text-to-video: generate from a text prompt
//! - Image-to-video: generate from a reference image + prompt
//!
//! Environment variables:
//! - `SEEDDANCE_API_KEY` — API key for SeedDance2
//! - `SEEDDANCE_BASE_URL` — API base URL (default: https://api.seeddance.com/v2)

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use crate::tools::video_gen::VideoGenBackend;
use hermes_core::ToolError;

const DEFAULT_BASE_URL: &str = "https://api.seeddance.com/v2";
const DEFAULT_DURATION: f32 = 5.0;
const DEFAULT_FPS: u32 = 24;
const MAX_POLL_ATTEMPTS: u32 = 120;
const POLL_INTERVAL_SECS: u64 = 5;

/// SeedDance2 video generation backend.
pub struct SeedDance2Backend {
    client: Client,
    api_key: String,
    base_url: String,
}

impl SeedDance2Backend {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url,
        }
    }

    pub fn from_env() -> Result<Self, ToolError> {
        let api_key = std::env::var("SEEDDANCE_API_KEY").map_err(|_| {
            ToolError::ExecutionFailed(
                "SEEDDANCE_API_KEY not set. Get an API key from SeedDance2.".into(),
            )
        })?;
        let trimmed = api_key.trim();
        if trimmed.is_empty() {
            return Err(ToolError::ExecutionFailed(
                "SEEDDANCE_API_KEY is empty".into(),
            ));
        }
        let base_url =
            std::env::var("SEEDDANCE_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into());
        Ok(Self::new(trimmed.to_string(), base_url))
    }

    /// Submit a text-to-video generation task.
    async fn submit_text_to_video(
        &self,
        prompt: &str,
        duration: f32,
        resolution: &str,
        fps: u32,
        aspect_ratio: &str,
        seed: Option<i64>,
    ) -> Result<Value, ToolError> {
        let url = format!(
            "{}/generation/text-to-video",
            self.base_url.trim_end_matches('/')
        );

        let mut body = json!({
            "prompt": prompt,
            "duration": duration,
            "resolution": resolution,
            "fps": fps,
            "aspect_ratio": aspect_ratio,
        });

        if let Some(s) = seed {
            body.as_object_mut()
                .unwrap()
                .insert("seed".into(), json!(s));
        }

        self.post_task(&url, &body).await
    }

    /// Submit an image-to-video generation task.
    async fn submit_image_to_video(
        &self,
        prompt: &str,
        input_image: &str,
        duration: f32,
        resolution: &str,
        fps: u32,
        aspect_ratio: &str,
        seed: Option<i64>,
    ) -> Result<Value, ToolError> {
        let url = format!(
            "{}/generation/image-to-video",
            self.base_url.trim_end_matches('/')
        );

        // Encode image if local
        let image_data =
            if input_image.starts_with("http://") || input_image.starts_with("https://") {
                json!({"url": input_image})
            } else {
                let bytes = tokio::fs::read(input_image).await.map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to read image '{input_image}': {e}"))
                })?;
                use base64::Engine;
                let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
                let mime = if input_image.ends_with(".png") {
                    "image/png"
                } else if input_image.ends_with(".webp") {
                    "image/webp"
                } else {
                    "image/jpeg"
                };
                json!({"base64": encoded, "mime_type": mime})
            };

        let mut body = json!({
            "prompt": prompt,
            "input_image": image_data,
            "duration": duration,
            "resolution": resolution,
            "fps": fps,
            "aspect_ratio": aspect_ratio,
        });

        if let Some(s) = seed {
            body.as_object_mut()
                .unwrap()
                .insert("seed".into(), json!(s));
        }

        self.post_task(&url, &body).await
    }

    /// POST a task and return the parsed response.
    async fn post_task(&self, url: &str, body: &Value) -> Result<Value, ToolError> {
        let resp = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!("SeedDance2 API request failed: {e}"))
            })?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionFailed(format!(
                "SeedDance2 error ({status}): {text}"
            )));
        }

        resp.json::<Value>()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse response: {e}")))
    }

    /// Poll for task completion, downloading the video when ready.
    async fn poll_until_complete(&self, task_id: &str) -> Result<String, ToolError> {
        let status_url = format!("{}/tasks/{}", self.base_url.trim_end_matches('/'), task_id);

        for attempt in 0..MAX_POLL_ATTEMPTS {
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;

            let resp = self
                .client
                .get(&status_url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .send()
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Status check failed: {e}")))?;

            let result: Value = resp
                .json()
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Parse error: {e}")))?;

            let status = result
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("unknown");

            match status {
                "completed" | "succeeded" => {
                    // Download the video
                    if let Some(video_url) = result.get("output").and_then(|o| {
                        o.get("video_url")
                            .or_else(|| o.get("url"))
                            .and_then(|u| u.as_str())
                    }) {
                        return self.download_video(task_id, video_url).await;
                    }
                    return Ok(result.to_string());
                }
                "failed" | "error" => {
                    let msg = result
                        .get("error")
                        .and_then(|e| e.as_str())
                        .unwrap_or("Unknown error");
                    return Err(ToolError::ExecutionFailed(format!(
                        "Video generation failed: {msg}"
                    )));
                }
                _ => {
                    let progress = result
                        .get("progress")
                        .and_then(|p| p.as_f64())
                        .unwrap_or(0.0);
                    tracing::debug!(
                        "Video task {task_id}: {status} ({:.0}%) — attempt {}/{}",
                        progress * 100.0,
                        attempt + 1,
                        MAX_POLL_ATTEMPTS
                    );
                }
            }
        }

        Err(ToolError::Timeout(format!(
            "Video generation timed out after {} seconds",
            MAX_POLL_ATTEMPTS as u64 * POLL_INTERVAL_SECS
        )))
    }

    /// Download a completed video to a temp file.
    async fn download_video(&self, task_id: &str, video_url: &str) -> Result<String, ToolError> {
        let bytes = self
            .client
            .get(video_url)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Video download failed: {e}")))?
            .bytes()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read video: {e}")))?;

        let output_path =
            std::env::temp_dir().join(format!("hermes_video_{}.mp4", uuid::Uuid::new_v4()));
        tokio::fs::write(&output_path, &bytes)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write video: {e}")))?;

        Ok(json!({
            "task_id": task_id,
            "status": "completed",
            "file": output_path.display().to_string(),
            "bytes": bytes.len(),
            "video_url": video_url,
        })
        .to_string())
    }
}

#[async_trait]
impl VideoGenBackend for SeedDance2Backend {
    async fn generate(
        &self,
        prompt: &str,
        input_image: Option<&str>,
        duration: Option<f32>,
        resolution: Option<&str>,
        fps: Option<u32>,
        aspect_ratio: Option<&str>,
        seed: Option<i64>,
    ) -> Result<String, ToolError> {
        let duration = duration.unwrap_or(DEFAULT_DURATION);
        let resolution = resolution.unwrap_or("1080p");
        let fps = fps.unwrap_or(DEFAULT_FPS);
        let aspect_ratio = aspect_ratio.unwrap_or("16:9");

        let submit_result = if let Some(img) = input_image {
            self.submit_image_to_video(prompt, img, duration, resolution, fps, aspect_ratio, seed)
                .await?
        } else {
            self.submit_text_to_video(prompt, duration, resolution, fps, aspect_ratio, seed)
                .await?
        };

        let task_id = submit_result
            .get("task_id")
            .or_else(|| submit_result.get("id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::ExecutionFailed(format!("No task_id in response: {}", submit_result))
            })?;

        // Poll until complete (blocking the tool call, which is fine for agent loop)
        self.poll_until_complete(task_id).await
    }

    async fn check_status(&self, task_id: &str) -> Result<String, ToolError> {
        let url = format!("{}/tasks/{}", self.base_url.trim_end_matches('/'), task_id);

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Status check failed: {e}")))?;

        let status_code = resp.status();
        if !status_code.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionFailed(format!(
                "Status check error ({status_code}): {text}"
            )));
        }

        let result: Value = resp
            .json()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Parse error: {e}")))?;

        Ok(result.to_string())
    }
}

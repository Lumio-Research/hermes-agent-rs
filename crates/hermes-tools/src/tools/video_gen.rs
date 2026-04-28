//! Video generation tool (SeedDance2 and extensible backends)
//!
//! Supports text-to-video and image-to-video generation with async polling
//! for long-running generation tasks.

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};

use hermes_core::{tool_schema, JsonSchema, ToolError, ToolHandler, ToolSchema};

use std::sync::Arc;

// ---------------------------------------------------------------------------
// VideoGenBackend trait
// ---------------------------------------------------------------------------

/// Backend for video generation operations.
#[async_trait]
pub trait VideoGenBackend: Send + Sync {
    /// Generate a video from a text prompt, optionally with a reference image.
    async fn generate(
        &self,
        prompt: &str,
        input_image: Option<&str>,
        duration: Option<f32>,
        resolution: Option<&str>,
        fps: Option<u32>,
        aspect_ratio: Option<&str>,
        seed: Option<i64>,
    ) -> Result<String, ToolError>;

    /// Check the status of a pending video generation task.
    async fn check_status(&self, task_id: &str) -> Result<String, ToolError>;
}

// ---------------------------------------------------------------------------
// VideoGenerateHandler
// ---------------------------------------------------------------------------

/// Tool for generating videos from text/image prompts.
pub struct VideoGenerateHandler {
    backend: Arc<dyn VideoGenBackend>,
}

impl VideoGenerateHandler {
    pub fn new(backend: Arc<dyn VideoGenBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl ToolHandler for VideoGenerateHandler {
    async fn execute(&self, params: Value) -> Result<String, ToolError> {
        // Check if this is a status check
        if let Some(task_id) = params.get("task_id").and_then(|v| v.as_str()) {
            return self.backend.check_status(task_id).await;
        }

        let prompt = params
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParams(
                    "Missing 'prompt' parameter (or 'task_id' for status check)".into(),
                )
            })?;

        let input_image = params.get("input_image").and_then(|v| v.as_str());
        let duration = params
            .get("duration")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32);
        let resolution = params.get("resolution").and_then(|v| v.as_str());
        let fps = params.get("fps").and_then(|v| v.as_u64()).map(|v| v as u32);
        let aspect_ratio = params.get("aspect_ratio").and_then(|v| v.as_str());
        let seed = params.get("seed").and_then(|v| v.as_i64());

        self.backend
            .generate(
                prompt,
                input_image,
                duration,
                resolution,
                fps,
                aspect_ratio,
                seed,
            )
            .await
    }

    fn schema(&self) -> ToolSchema {
        let mut props = IndexMap::new();
        props.insert(
            "prompt".into(),
            json!({
                "type": "string",
                "description": "Text description of the video to generate"
            }),
        );
        props.insert(
            "input_image".into(),
            json!({
                "type": "string",
                "description": "Path or URL to a reference image for image-to-video generation (optional)"
            }),
        );
        props.insert(
            "duration".into(),
            json!({
                "type": "number",
                "description": "Video duration in seconds (default: 5.0, max varies by backend)",
                "default": 5.0
            }),
        );
        props.insert(
            "resolution".into(),
            json!({
                "type": "string",
                "description": "Video resolution: '720p', '1080p', '4k'",
                "enum": ["720p", "1080p", "4k"],
                "default": "1080p"
            }),
        );
        props.insert(
            "fps".into(),
            json!({
                "type": "integer",
                "description": "Frames per second (default: 24)",
                "default": 24
            }),
        );
        props.insert(
            "aspect_ratio".into(),
            json!({
                "type": "string",
                "description": "Aspect ratio: '16:9', '9:16', '1:1'",
                "enum": ["16:9", "9:16", "1:1"],
                "default": "16:9"
            }),
        );
        props.insert(
            "seed".into(),
            json!({
                "type": "integer",
                "description": "Random seed for reproducible generation (optional)"
            }),
        );
        props.insert(
            "task_id".into(),
            json!({
                "type": "string",
                "description": "Task ID to check status of a pending generation (use instead of prompt)"
            }),
        );

        tool_schema(
            "video_generate",
            "Generate videos from text descriptions or reference images using AI video generation models (SeedDance2). \
             Returns a task_id for async polling when generation takes time.",
            JsonSchema::object(props, vec![]),
        )
    }
}

// ---------------------------------------------------------------------------
// VideoStatusHandler (convenience tool for status checks)
// ---------------------------------------------------------------------------

/// Dedicated tool for checking video generation task status.
pub struct VideoStatusHandler {
    backend: Arc<dyn VideoGenBackend>,
}

impl VideoStatusHandler {
    pub fn new(backend: Arc<dyn VideoGenBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl ToolHandler for VideoStatusHandler {
    async fn execute(&self, params: Value) -> Result<String, ToolError> {
        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'task_id' parameter".into()))?;

        self.backend.check_status(task_id).await
    }

    fn schema(&self) -> ToolSchema {
        let mut props = IndexMap::new();
        props.insert(
            "task_id".into(),
            json!({
                "type": "string",
                "description": "Task ID returned by video_generate"
            }),
        );

        tool_schema(
            "video_status",
            "Check the status of a pending video generation task. Returns progress, status, and download URL when complete.",
            JsonSchema::object(props, vec!["task_id".into()]),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockVideoGenBackend;
    #[async_trait]
    impl VideoGenBackend for MockVideoGenBackend {
        async fn generate(
            &self,
            prompt: &str,
            _input_image: Option<&str>,
            _duration: Option<f32>,
            _resolution: Option<&str>,
            _fps: Option<u32>,
            _aspect_ratio: Option<&str>,
            _seed: Option<i64>,
        ) -> Result<String, ToolError> {
            Ok(json!({
                "task_id": "task_abc123",
                "status": "processing",
                "prompt": prompt,
            })
            .to_string())
        }

        async fn check_status(&self, task_id: &str) -> Result<String, ToolError> {
            Ok(json!({
                "task_id": task_id,
                "status": "completed",
                "video_url": "https://example.com/video.mp4",
            })
            .to_string())
        }
    }

    #[tokio::test]
    async fn test_video_generate_schema() {
        let handler = VideoGenerateHandler::new(Arc::new(MockVideoGenBackend));
        assert_eq!(handler.schema().name, "video_generate");
    }

    #[tokio::test]
    async fn test_video_generate_execute() {
        let handler = VideoGenerateHandler::new(Arc::new(MockVideoGenBackend));
        let result = handler
            .execute(json!({"prompt": "a dancing robot"}))
            .await
            .unwrap();
        assert!(result.contains("task_abc123"));
    }

    #[tokio::test]
    async fn test_video_status() {
        let handler = VideoStatusHandler::new(Arc::new(MockVideoGenBackend));
        let result = handler
            .execute(json!({"task_id": "task_abc123"}))
            .await
            .unwrap();
        assert!(result.contains("completed"));
    }
}

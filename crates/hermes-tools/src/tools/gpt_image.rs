//! GPT Image generation tool (ChatGPT gpt-image-1 model)
//!
//! Uses OpenAI's image generation API with the `gpt-image-1` model,
//! supporting text-to-image and image editing with natural language.

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};

use hermes_core::{tool_schema, JsonSchema, ToolError, ToolHandler, ToolSchema};

use std::sync::Arc;

// ---------------------------------------------------------------------------
// GptImageBackend trait
// ---------------------------------------------------------------------------

/// Backend for GPT image generation operations.
#[async_trait]
pub trait GptImageBackend: Send + Sync {
    /// Generate an image from a text prompt, optionally with a reference image
    /// for editing.
    async fn generate(
        &self,
        prompt: &str,
        size: Option<&str>,
        quality: Option<&str>,
        background: Option<&str>,
        input_image: Option<&str>,
        n: Option<u32>,
    ) -> Result<String, ToolError>;
}

// ---------------------------------------------------------------------------
// GptImageHandler
// ---------------------------------------------------------------------------

/// Tool for generating / editing images using OpenAI's gpt-image-1 model.
pub struct GptImageHandler {
    backend: Arc<dyn GptImageBackend>,
}

impl GptImageHandler {
    pub fn new(backend: Arc<dyn GptImageBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl ToolHandler for GptImageHandler {
    async fn execute(&self, params: Value) -> Result<String, ToolError> {
        let prompt = params
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'prompt' parameter".into()))?;

        let size = params.get("size").and_then(|v| v.as_str());
        let quality = params.get("quality").and_then(|v| v.as_str());
        let background = params.get("background").and_then(|v| v.as_str());
        let input_image = params.get("input_image").and_then(|v| v.as_str());
        let n = params.get("n").and_then(|v| v.as_u64()).map(|v| v as u32);

        self.backend
            .generate(prompt, size, quality, background, input_image, n)
            .await
    }

    fn schema(&self) -> ToolSchema {
        let mut props = IndexMap::new();
        props.insert(
            "prompt".into(),
            json!({
                "type": "string",
                "description": "Text description of the image to generate or edit instruction"
            }),
        );
        props.insert(
            "size".into(),
            json!({
                "type": "string",
                "description": "Output size: '1024x1024' (square), '1536x1024' (landscape), '1024x1536' (portrait), 'auto'",
                "enum": ["1024x1024", "1536x1024", "1024x1536", "auto"],
                "default": "auto"
            }),
        );
        props.insert(
            "quality".into(),
            json!({
                "type": "string",
                "description": "Image quality: 'low', 'medium', 'high'",
                "enum": ["low", "medium", "high"],
                "default": "high"
            }),
        );
        props.insert(
            "background".into(),
            json!({
                "type": "string",
                "description": "Background handling: 'auto', 'transparent', 'opaque'",
                "enum": ["auto", "transparent", "opaque"],
                "default": "auto"
            }),
        );
        props.insert(
            "input_image".into(),
            json!({
                "type": "string",
                "description": "Path or URL to a reference image for editing (optional)"
            }),
        );
        props.insert(
            "n".into(),
            json!({
                "type": "integer",
                "description": "Number of images to generate (1-4, default: 1)",
                "default": 1,
                "minimum": 1,
                "maximum": 4
            }),
        );

        tool_schema(
            "gpt_image_generate",
            "Generate or edit images using OpenAI's gpt-image-1 model. Supports text-to-image generation and image editing with natural language instructions.",
            JsonSchema::object(props, vec!["prompt".into()]),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockGptImageBackend;
    #[async_trait]
    impl GptImageBackend for MockGptImageBackend {
        async fn generate(
            &self,
            prompt: &str,
            _size: Option<&str>,
            _quality: Option<&str>,
            _background: Option<&str>,
            _input_image: Option<&str>,
            _n: Option<u32>,
        ) -> Result<String, ToolError> {
            Ok(json!({
                "model": "gpt-image-1",
                "images": [{"url": "https://example.com/img.png"}],
                "prompt": prompt,
            })
            .to_string())
        }
    }

    #[tokio::test]
    async fn test_gpt_image_schema() {
        let handler = GptImageHandler::new(Arc::new(MockGptImageBackend));
        assert_eq!(handler.schema().name, "gpt_image_generate");
    }

    #[tokio::test]
    async fn test_gpt_image_execute() {
        let handler = GptImageHandler::new(Arc::new(MockGptImageBackend));
        let result = handler
            .execute(json!({"prompt": "a cat wearing a hat"}))
            .await
            .unwrap();
        assert!(result.contains("gpt-image-1"));
    }
}

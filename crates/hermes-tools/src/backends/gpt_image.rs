//! GPT Image backend: OpenAI gpt-image-1 API.
//!
//! Calls `POST /v1/images/generations` (text-to-image) or
//! `POST /v1/images/edits` (image editing with reference).
//! Supports both direct API key and managed gateway transport.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use crate::tools::gpt_image::GptImageBackend;
use hermes_config::managed_gateway::{
    prefers_gateway, resolve_managed_tool_gateway, ManagedToolGatewayConfig, ResolveOptions,
};
use hermes_core::ToolError;

const DEFAULT_MODEL: &str = "gpt-image-1";

/// OpenAI gpt-image-1 backend.
pub struct OpenAiGptImageBackend {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAiGptImageBackend {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url,
            model: DEFAULT_MODEL.into(),
        }
    }

    /// Resolve from environment.
    ///
    /// Priority: direct `OPENAI_API_KEY` unless `gpt_image.use_gateway: true`,
    /// then Nous-managed gateway → `Err` with a hint.
    pub fn from_env_or_managed() -> Result<Self, ToolError> {
        let force_gateway = prefers_gateway("gpt_image");
        if !force_gateway {
            if let Ok(key) = std::env::var("OPENAI_API_KEY") {
                let trimmed = key.trim();
                if !trimmed.is_empty() {
                    let base = std::env::var("OPENAI_BASE_URL")
                        .unwrap_or_else(|_| "https://api.openai.com/v1".into());
                    return Ok(Self::new(trimmed.to_string(), base));
                }
            }
        }

        // Try managed gateway
        let opts = ResolveOptions::default();
        if let Some(cfg) = resolve_managed_tool_gateway("openai", opts) {
            return Ok(Self::from_managed(&cfg));
        }

        Err(ToolError::ExecutionFailed(
            "GPT Image requires OPENAI_API_KEY or a managed gateway. \
             Set the key in ~/.hermes/.env or enable managed tools."
                .into(),
        ))
    }

    fn from_managed(cfg: &ManagedToolGatewayConfig) -> Self {
        Self {
            client: Client::new(),
            api_key: cfg.nous_user_token.clone(),
            base_url: cfg.gateway_origin.clone(),
            model: DEFAULT_MODEL.into(),
        }
    }

    /// Text-to-image generation via `/v1/images/generations`.
    async fn generate_from_text(
        &self,
        prompt: &str,
        size: &str,
        quality: &str,
        background: &str,
        n: u32,
    ) -> Result<String, ToolError> {
        let url = format!("{}/images/generations", self.base_url.trim_end_matches('/'));

        let body = json!({
            "model": self.model,
            "prompt": prompt,
            "size": size,
            "quality": quality,
            "background": background,
            "n": n,
            "output_format": "png",
        });

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("OpenAI API request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionFailed(format!(
                "OpenAI image generation error ({status}): {text}"
            )));
        }

        let result: Value = resp
            .json()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse response: {e}")))?;

        // Save images to temp files and return paths
        self.save_images(&result, prompt).await
    }

    /// Image editing via `/v1/images/edits` with a reference image.
    async fn edit_image(
        &self,
        prompt: &str,
        input_image: &str,
        size: &str,
        quality: &str,
        n: u32,
    ) -> Result<String, ToolError> {
        let url = format!("{}/images/edits", self.base_url.trim_end_matches('/'));

        // Read the input image
        let image_bytes =
            if input_image.starts_with("http://") || input_image.starts_with("https://") {
                self.client
                    .get(input_image)
                    .send()
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(format!("Failed to fetch image: {e}")))?
                    .bytes()
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read image: {e}")))?
                    .to_vec()
            } else {
                tokio::fs::read(input_image).await.map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to read '{input_image}': {e}"))
                })?
            };

        let file_part = reqwest::multipart::Part::bytes(image_bytes)
            .file_name("image.png")
            .mime_str("image/png")
            .map_err(|e| ToolError::ExecutionFailed(format!("Multipart error: {e}")))?;

        let form = reqwest::multipart::Form::new()
            .text("model", self.model.clone())
            .text("prompt", prompt.to_string())
            .text("size", size.to_string())
            .text("quality", quality.to_string())
            .text("n", n.to_string())
            .part("image", file_part);

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("OpenAI edit API failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionFailed(format!(
                "OpenAI image edit error ({status}): {text}"
            )));
        }

        let result: Value = resp
            .json()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to parse response: {e}")))?;

        self.save_images(&result, prompt).await
    }

    /// Download and save generated images to temp directory, return JSON summary.
    async fn save_images(&self, api_response: &Value, prompt: &str) -> Result<String, ToolError> {
        let data = api_response
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| ToolError::ExecutionFailed("No image data in response".into()))?;

        let mut saved = Vec::new();
        for (i, item) in data.iter().enumerate() {
            let output_path = std::env::temp_dir().join(format!(
                "hermes_gpt_image_{}_{}.png",
                uuid::Uuid::new_v4(),
                i
            ));

            // gpt-image-1 returns base64 by default
            if let Some(b64) = item.get("b64_json").and_then(|v| v.as_str()) {
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|e| ToolError::ExecutionFailed(format!("Base64 decode error: {e}")))?;
                tokio::fs::write(&output_path, &bytes).await.map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to write image: {e}"))
                })?;
                saved.push(json!({
                    "file": output_path.display().to_string(),
                    "bytes": bytes.len(),
                }));
            } else if let Some(url) = item.get("url").and_then(|v| v.as_str()) {
                // Fallback: download from URL
                let bytes = self
                    .client
                    .get(url)
                    .send()
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(format!("Download failed: {e}")))?
                    .bytes()
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(format!("Read failed: {e}")))?;
                tokio::fs::write(&output_path, &bytes).await.map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to write image: {e}"))
                })?;
                saved.push(json!({
                    "file": output_path.display().to_string(),
                    "url": url,
                    "bytes": bytes.len(),
                }));
            }
        }

        Ok(json!({
            "model": self.model,
            "prompt": prompt,
            "images": saved,
            "count": saved.len(),
        })
        .to_string())
    }
}

#[async_trait]
impl GptImageBackend for OpenAiGptImageBackend {
    async fn generate(
        &self,
        prompt: &str,
        size: Option<&str>,
        quality: Option<&str>,
        background: Option<&str>,
        input_image: Option<&str>,
        n: Option<u32>,
    ) -> Result<String, ToolError> {
        let size = size.unwrap_or("auto");
        let quality = quality.unwrap_or("high");
        let background = background.unwrap_or("auto");
        let n = n.unwrap_or(1).min(4).max(1);

        if let Some(img) = input_image {
            self.edit_image(prompt, img, size, quality, n).await
        } else {
            self.generate_from_text(prompt, size, quality, background, n)
                .await
        }
    }
}

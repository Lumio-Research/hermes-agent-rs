//! Enhanced audio generation backends.
//!
//! Supports multiple providers for music, SFX, and voice cloning:
//! - Fish Audio: voice cloning + TTS (FISH_AUDIO_API_KEY)
//! - Suno-compatible: music generation (SUNO_API_KEY)
//! - ElevenLabs: voice cloning + SFX (ELEVENLABS_API_KEY)
//!
//! Falls back gracefully when keys are missing.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

use crate::tools::audio_gen::AudioGenBackend;
use hermes_core::ToolError;

/// Multi-provider audio generation backend.
pub struct MultiAudioGenBackend {
    client: Client,
    fish_audio_key: Option<String>,
    suno_key: Option<String>,
    suno_base_url: String,
    elevenlabs_key: Option<String>,
}

impl MultiAudioGenBackend {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            fish_audio_key: std::env::var("FISH_AUDIO_API_KEY")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            suno_key: std::env::var("SUNO_API_KEY")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            suno_base_url: std::env::var("SUNO_BASE_URL")
                .unwrap_or_else(|_| "https://api.suno.ai/v1".into()),
            elevenlabs_key: std::env::var("ELEVENLABS_API_KEY")
                .ok()
                .filter(|v| !v.trim().is_empty()),
        }
    }

    fn available_providers(&self) -> Vec<&str> {
        let mut providers = Vec::new();
        if self.suno_key.is_some() {
            providers.push("suno");
        }
        if self.elevenlabs_key.is_some() {
            providers.push("elevenlabs");
        }
        if self.fish_audio_key.is_some() {
            providers.push("fish_audio");
        }
        providers
    }

    /// Music generation via Suno-compatible API.
    async fn suno_music(
        &self,
        prompt: &str,
        duration: Option<f32>,
        genre: Option<&str>,
        instrumental: bool,
    ) -> Result<String, ToolError> {
        let api_key = self.suno_key.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed(
                "SUNO_API_KEY not set. Set it in ~/.hermes/.env for music generation.".into(),
            )
        })?;

        let mut body = json!({
            "prompt": prompt,
            "make_instrumental": instrumental,
        });

        if let Some(d) = duration {
            body.as_object_mut()
                .unwrap()
                .insert("duration".into(), json!(d));
        }
        if let Some(g) = genre {
            body.as_object_mut()
                .unwrap()
                .insert("tags".into(), json!(g));
        }

        let url = format!("{}/generation", self.suno_base_url.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&body)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Suno API failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionFailed(format!(
                "Suno error ({status}): {text}"
            )));
        }

        let result: Value = resp
            .json()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Parse error: {e}")))?;

        // Download audio if URL is provided
        if let Some(audio_url) = result.get("audio_url").and_then(|u| u.as_str()) {
            let bytes = self
                .client
                .get(audio_url)
                .send()
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Download failed: {e}")))?
                .bytes()
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Read failed: {e}")))?;

            let path =
                std::env::temp_dir().join(format!("hermes_music_{}.mp3", uuid::Uuid::new_v4()));
            tokio::fs::write(&path, &bytes)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Write failed: {e}")))?;

            return Ok(json!({
                "provider": "suno",
                "mode": "music",
                "file": path.display().to_string(),
                "bytes": bytes.len(),
                "prompt": prompt,
            })
            .to_string());
        }

        Ok(json!({
            "provider": "suno",
            "mode": "music",
            "result": result,
            "prompt": prompt,
        })
        .to_string())
    }

    /// Sound effects via ElevenLabs Sound Generation API.
    async fn elevenlabs_sfx(
        &self,
        prompt: &str,
        duration: Option<f32>,
    ) -> Result<String, ToolError> {
        let api_key = self.elevenlabs_key.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed(
                "ELEVENLABS_API_KEY not set. Set it in ~/.hermes/.env for SFX generation.".into(),
            )
        })?;

        let mut body = json!({
            "text": prompt,
        });
        if let Some(d) = duration {
            body.as_object_mut()
                .unwrap()
                .insert("duration_seconds".into(), json!(d));
        }

        let resp = self
            .client
            .post("https://api.elevenlabs.io/v1/sound-generation")
            .header("xi-api-key", api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("ElevenLabs SFX failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionFailed(format!(
                "ElevenLabs SFX error ({status}): {text}"
            )));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Read failed: {e}")))?;

        let path = std::env::temp_dir().join(format!("hermes_sfx_{}.mp3", uuid::Uuid::new_v4()));
        tokio::fs::write(&path, &bytes)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Write failed: {e}")))?;

        Ok(json!({
            "provider": "elevenlabs",
            "mode": "sfx",
            "file": path.display().to_string(),
            "bytes": bytes.len(),
            "prompt": prompt,
        })
        .to_string())
    }

    /// Voice cloning via Fish Audio API.
    async fn fish_audio_clone(
        &self,
        reference_audio: &str,
        text: &str,
    ) -> Result<String, ToolError> {
        let api_key = self.fish_audio_key.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed(
                "FISH_AUDIO_API_KEY not set. Set it in ~/.hermes/.env for voice cloning.".into(),
            )
        })?;

        // Read reference audio
        let ref_bytes = if reference_audio.starts_with("http://")
            || reference_audio.starts_with("https://")
        {
            self.client
                .get(reference_audio)
                .send()
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Fetch reference failed: {e}")))?
                .bytes()
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Read reference failed: {e}")))?
                .to_vec()
        } else {
            tokio::fs::read(reference_audio).await.map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to read reference audio '{reference_audio}': {e}"
                ))
            })?
        };

        let ref_part = reqwest::multipart::Part::bytes(ref_bytes)
            .file_name("reference.mp3")
            .mime_str("audio/mpeg")
            .map_err(|e| ToolError::ExecutionFailed(format!("Multipart error: {e}")))?;

        let form = reqwest::multipart::Form::new()
            .text("text", text.to_string())
            .part("reference_audio", ref_part);

        let resp = self
            .client
            .post("https://api.fish.audio/v1/tts")
            .header("Authorization", format!("Bearer {api_key}"))
            .multipart(form)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Fish Audio API failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionFailed(format!(
                "Fish Audio error ({status}): {text}"
            )));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Read failed: {e}")))?;

        let path =
            std::env::temp_dir().join(format!("hermes_voice_clone_{}.mp3", uuid::Uuid::new_v4()));
        tokio::fs::write(&path, &bytes)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Write failed: {e}")))?;

        Ok(json!({
            "provider": "fish_audio",
            "mode": "voice_clone",
            "file": path.display().to_string(),
            "bytes": bytes.len(),
        })
        .to_string())
    }
}

impl Default for MultiAudioGenBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioGenBackend for MultiAudioGenBackend {
    async fn generate_music(
        &self,
        prompt: &str,
        duration: Option<f32>,
        genre: Option<&str>,
        instrumental: bool,
    ) -> Result<String, ToolError> {
        if self.suno_key.is_some() {
            return self.suno_music(prompt, duration, genre, instrumental).await;
        }

        Err(ToolError::ExecutionFailed(format!(
            "No music generation provider available. Set SUNO_API_KEY in ~/.hermes/.env. \
             Available providers: {:?}",
            self.available_providers()
        )))
    }

    async fn generate_sfx(&self, prompt: &str, duration: Option<f32>) -> Result<String, ToolError> {
        if self.elevenlabs_key.is_some() {
            return self.elevenlabs_sfx(prompt, duration).await;
        }

        Err(ToolError::ExecutionFailed(format!(
            "No SFX generation provider available. Set ELEVENLABS_API_KEY in ~/.hermes/.env. \
             Available providers: {:?}",
            self.available_providers()
        )))
    }

    async fn clone_voice(&self, reference_audio: &str, text: &str) -> Result<String, ToolError> {
        if self.fish_audio_key.is_some() {
            return self.fish_audio_clone(reference_audio, text).await;
        }

        // Fallback to ElevenLabs voice cloning if available
        if self.elevenlabs_key.is_some() {
            return Err(ToolError::ExecutionFailed(
                "ElevenLabs voice cloning requires a pre-created voice ID. \
                 Use Fish Audio (FISH_AUDIO_API_KEY) for zero-shot voice cloning."
                    .into(),
            ));
        }

        Err(ToolError::ExecutionFailed(format!(
            "No voice cloning provider available. Set FISH_AUDIO_API_KEY in ~/.hermes/.env. \
             Available providers: {:?}",
            self.available_providers()
        )))
    }
}

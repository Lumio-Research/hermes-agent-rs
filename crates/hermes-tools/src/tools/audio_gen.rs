//! Enhanced audio generation tool
//!
//! Supports multiple audio generation capabilities beyond basic TTS:
//! - Music generation (via Suno, Udio, or similar APIs)
//! - Sound effects generation
//! - Voice cloning
//! - Audio-to-audio style transfer
//!
//! Complements the existing `tts` tool for speech synthesis.

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};

use hermes_core::{tool_schema, JsonSchema, ToolError, ToolHandler, ToolSchema};

use std::sync::Arc;

// ---------------------------------------------------------------------------
// AudioGenBackend trait
// ---------------------------------------------------------------------------

/// Backend for advanced audio generation operations.
#[async_trait]
pub trait AudioGenBackend: Send + Sync {
    /// Generate music from a text prompt.
    async fn generate_music(
        &self,
        prompt: &str,
        duration: Option<f32>,
        genre: Option<&str>,
        instrumental: bool,
    ) -> Result<String, ToolError>;

    /// Generate sound effects from a text description.
    async fn generate_sfx(&self, prompt: &str, duration: Option<f32>) -> Result<String, ToolError>;

    /// Clone a voice from a reference audio sample.
    async fn clone_voice(&self, reference_audio: &str, text: &str) -> Result<String, ToolError>;
}

// ---------------------------------------------------------------------------
// AudioGenerateHandler
// ---------------------------------------------------------------------------

/// Tool for generating music, sound effects, and cloned voice audio.
pub struct AudioGenerateHandler {
    backend: Arc<dyn AudioGenBackend>,
}

impl AudioGenerateHandler {
    pub fn new(backend: Arc<dyn AudioGenBackend>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl ToolHandler for AudioGenerateHandler {
    async fn execute(&self, params: Value) -> Result<String, ToolError> {
        let mode = params
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("music");

        match mode {
            "music" => {
                let prompt = params
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidParams("Missing 'prompt' for music generation".into())
                    })?;
                let duration = params
                    .get("duration")
                    .and_then(|v| v.as_f64())
                    .map(|v| v as f32);
                let genre = params.get("genre").and_then(|v| v.as_str());
                let instrumental = params
                    .get("instrumental")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                self.backend
                    .generate_music(prompt, duration, genre, instrumental)
                    .await
            }
            "sfx" | "sound_effect" => {
                let prompt = params
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidParams("Missing 'prompt' for SFX generation".into())
                    })?;
                let duration = params
                    .get("duration")
                    .and_then(|v| v.as_f64())
                    .map(|v| v as f32);
                self.backend.generate_sfx(prompt, duration).await
            }
            "voice_clone" => {
                let reference = params
                    .get("reference_audio")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ToolError::InvalidParams(
                            "Missing 'reference_audio' for voice cloning".into(),
                        )
                    })?;
                let text = params.get("text").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::InvalidParams("Missing 'text' for voice cloning".into())
                })?;
                self.backend.clone_voice(reference, text).await
            }
            _ => Err(ToolError::InvalidParams(format!(
                "Unknown audio mode: '{mode}'. Use: music, sfx, voice_clone"
            ))),
        }
    }

    fn schema(&self) -> ToolSchema {
        let mut props = IndexMap::new();
        props.insert(
            "mode".into(),
            json!({
                "type": "string",
                "description": "Generation mode: 'music' (AI music), 'sfx' (sound effects), 'voice_clone' (voice cloning)",
                "enum": ["music", "sfx", "voice_clone"],
                "default": "music"
            }),
        );
        props.insert(
            "prompt".into(),
            json!({
                "type": "string",
                "description": "Text description of the audio to generate (for music and sfx modes)"
            }),
        );
        props.insert(
            "duration".into(),
            json!({
                "type": "number",
                "description": "Audio duration in seconds (default varies by mode)"
            }),
        );
        props.insert(
            "genre".into(),
            json!({
                "type": "string",
                "description": "Music genre hint (for music mode): 'pop', 'rock', 'classical', 'electronic', 'ambient', etc."
            }),
        );
        props.insert(
            "instrumental".into(),
            json!({
                "type": "boolean",
                "description": "Generate instrumental only, no vocals (for music mode)",
                "default": false
            }),
        );
        props.insert(
            "reference_audio".into(),
            json!({
                "type": "string",
                "description": "Path or URL to reference audio file (for voice_clone mode)"
            }),
        );
        props.insert(
            "text".into(),
            json!({
                "type": "string",
                "description": "Text to speak in the cloned voice (for voice_clone mode)"
            }),
        );

        tool_schema(
            "audio_generate",
            "Generate music, sound effects, or cloned voice audio using AI. \
             Supports music composition from text prompts, sound effect synthesis, \
             and voice cloning from reference samples.",
            JsonSchema::object(props, vec!["mode".into()]),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAudioGenBackend;
    #[async_trait]
    impl AudioGenBackend for MockAudioGenBackend {
        async fn generate_music(
            &self,
            prompt: &str,
            _duration: Option<f32>,
            genre: Option<&str>,
            instrumental: bool,
        ) -> Result<String, ToolError> {
            Ok(json!({
                "mode": "music",
                "prompt": prompt,
                "genre": genre,
                "instrumental": instrumental,
                "file": "/tmp/music.mp3",
            })
            .to_string())
        }

        async fn generate_sfx(
            &self,
            prompt: &str,
            _duration: Option<f32>,
        ) -> Result<String, ToolError> {
            Ok(json!({
                "mode": "sfx",
                "prompt": prompt,
                "file": "/tmp/sfx.wav",
            })
            .to_string())
        }

        async fn clone_voice(&self, _reference: &str, text: &str) -> Result<String, ToolError> {
            Ok(json!({
                "mode": "voice_clone",
                "text": text,
                "file": "/tmp/cloned.mp3",
            })
            .to_string())
        }
    }

    #[tokio::test]
    async fn test_music_generation() {
        let handler = AudioGenerateHandler::new(Arc::new(MockAudioGenBackend));
        let result = handler
            .execute(json!({
                "mode": "music",
                "prompt": "upbeat electronic dance track",
                "genre": "electronic",
                "instrumental": true
            }))
            .await
            .unwrap();
        assert!(result.contains("electronic"));
    }

    #[tokio::test]
    async fn test_sfx_generation() {
        let handler = AudioGenerateHandler::new(Arc::new(MockAudioGenBackend));
        let result = handler
            .execute(json!({
                "mode": "sfx",
                "prompt": "thunder and rain"
            }))
            .await
            .unwrap();
        assert!(result.contains("sfx"));
    }

    #[tokio::test]
    async fn test_schema() {
        let handler = AudioGenerateHandler::new(Arc::new(MockAudioGenBackend));
        assert_eq!(handler.schema().name, "audio_generate");
    }
}

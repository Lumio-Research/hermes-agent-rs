//! Voice mode — full STT→Agent→TTS loop management.
//!
//! This module provides:
//!
//! 1. A process-wide **voice mode flag** (atomic bool) that the CLI/TUI polls.
//! 2. A **VoicePipeline** that orchestrates:
//!    - Audio recording via configurable input sources
//!    - Voice Activity Detection (VAD) with energy-based + zero-crossing analysis
//!    - Streaming STT (Speech-to-Text) via Whisper / Deepgram / custom endpoints
//!    - TTS (Text-to-Speech) playback via the existing `MultiTtsBackend`
//!    - Audio chunk buffering with configurable silence thresholds
//! 3. A **VoiceModeHandler** tool that the LLM can invoke to toggle voice mode,
//!    configure STT/TTS providers, adjust VAD sensitivity, and query status.
//!
//! Architecture:
//! ```text
//!   Mic → [AudioCapture] → [VAD] → [STT] → agent_loop → [TTS] → Speaker
//!                                     ↑                     ↑
//!                              VoicePipeline          MultiTtsBackend
//! ```
//!
//! The pipeline is designed to be driven by the CLI event loop. The tool handler
//! only manages configuration and state; actual audio I/O is delegated to the
//! caller via the `AudioSource` / `AudioSink` traits.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{watch, Mutex};

use hermes_core::{tool_schema, JsonSchema, ToolError, ToolHandler, ToolSchema};

// ---------------------------------------------------------------------------
// Global voice mode flag
// ---------------------------------------------------------------------------

static VOICE_MODE_ENABLED: AtomicBool = AtomicBool::new(false);

/// Returns the current process-wide voice mode flag.
pub fn voice_mode_enabled() -> bool {
    VOICE_MODE_ENABLED.load(Ordering::Relaxed)
}

/// Force the flag (for tests / CLI command-line --voice).
pub fn set_voice_mode_enabled(value: bool) {
    VOICE_MODE_ENABLED.store(value, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// Voice Activity Detection (VAD)
// ---------------------------------------------------------------------------

/// Configuration for the energy-based Voice Activity Detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    /// RMS energy threshold (0.0–1.0 for f32 samples). Frames below this
    /// are considered silence.
    pub energy_threshold: f32,
    /// Minimum consecutive voiced frames before we consider speech started.
    pub min_speech_frames: usize,
    /// Silence duration (ms) after speech before we consider speech ended.
    pub silence_timeout_ms: u64,
    /// Frame size in samples (e.g. 480 for 30ms at 16kHz).
    pub frame_size: usize,
    /// Zero-crossing rate upper bound — very high ZCR with low energy
    /// indicates noise, not speech.
    pub max_zcr: f32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            energy_threshold: 0.02,
            min_speech_frames: 3,
            silence_timeout_ms: 800,
            frame_size: 480,
            max_zcr: 0.5,
        }
    }
}

/// Simple energy + zero-crossing VAD.
///
/// This is intentionally lightweight. For production use, consider Silero VAD
/// or WebRTC VAD via ONNX. This implementation is sufficient for filtering
/// obvious silence and noise.
#[derive(Debug, Clone)]
pub struct VoiceActivityDetector {
    config: VadConfig,
    consecutive_speech: usize,
    speech_active: bool,
    last_speech_time: Option<Instant>,
}

impl VoiceActivityDetector {
    pub fn new(config: VadConfig) -> Self {
        Self {
            config,
            consecutive_speech: 0,
            speech_active: false,
            last_speech_time: None,
        }
    }

    /// Process a frame of f32 PCM samples (mono, normalized to [-1, 1]).
    /// Returns `true` if the frame is considered speech.
    pub fn process_frame(&mut self, samples: &[f32]) -> bool {
        if samples.is_empty() {
            return false;
        }

        let rms = Self::compute_rms(samples);
        let zcr = Self::compute_zcr(samples);

        let is_voiced = rms >= self.config.energy_threshold && zcr <= self.config.max_zcr;

        if is_voiced {
            self.consecutive_speech += 1;
            self.last_speech_time = Some(Instant::now());

            if self.consecutive_speech >= self.config.min_speech_frames {
                self.speech_active = true;
            }
        } else {
            self.consecutive_speech = 0;

            if self.speech_active {
                // Check silence timeout
                if let Some(last) = self.last_speech_time {
                    if last.elapsed() > Duration::from_millis(self.config.silence_timeout_ms) {
                        self.speech_active = false;
                    }
                }
            }
        }

        self.speech_active
    }

    /// Reset the detector state (e.g. between utterances).
    pub fn reset(&mut self) {
        self.consecutive_speech = 0;
        self.speech_active = false;
        self.last_speech_time = None;
    }

    /// Whether speech is currently detected.
    pub fn is_speech_active(&self) -> bool {
        self.speech_active
    }

    /// Compute RMS energy of a sample buffer.
    fn compute_rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
        (sum_sq / samples.len() as f32).sqrt()
    }

    /// Compute zero-crossing rate (0.0–1.0).
    fn compute_zcr(samples: &[f32]) -> f32 {
        if samples.len() < 2 {
            return 0.0;
        }
        let crossings = samples
            .windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count();
        crossings as f32 / (samples.len() - 1) as f32
    }
}

// ---------------------------------------------------------------------------
// Audio I/O traits (implemented by CLI / TUI / gateway)
// ---------------------------------------------------------------------------

/// Trait for audio input sources. The CLI implements this with `cpal` or
/// platform-specific recording APIs.
#[async_trait]
pub trait AudioSource: Send + Sync {
    /// Read the next chunk of PCM audio (mono, 16kHz, f32).
    /// Returns `None` when the source is closed.
    async fn read_chunk(&self) -> Option<Vec<f32>>;

    /// Sample rate of the source.
    fn sample_rate(&self) -> u32;

    /// Number of channels (should be 1 for mono).
    fn channels(&self) -> u16;
}

/// Trait for audio output sinks. The CLI implements this with `cpal` or
/// platform-specific playback APIs.
#[async_trait]
pub trait AudioSink: Send + Sync {
    /// Play raw audio bytes (format depends on TTS provider, typically MP3).
    async fn play(&self, audio_data: &[u8], format: &str) -> Result<(), ToolError>;

    /// Stop any currently playing audio.
    async fn stop(&self) -> Result<(), ToolError>;
}

// ---------------------------------------------------------------------------
// STT Provider abstraction
// ---------------------------------------------------------------------------

/// STT provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttConfig {
    pub provider: SttProviderType,
    pub language: Option<String>,
    pub model: Option<String>,
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            provider: SttProviderType::Whisper,
            language: None,
            model: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SttProviderType {
    Whisper,
    Deepgram,
    Custom(String),
}

/// Transcribe raw PCM audio to text using the configured STT provider.
pub async fn transcribe_audio(
    config: &SttConfig,
    audio_pcm: &[f32],
    sample_rate: u32,
) -> Result<String, ToolError> {
    // Convert f32 PCM to 16-bit WAV in memory for API upload
    let wav_bytes = pcm_to_wav(audio_pcm, sample_rate, 1);

    match &config.provider {
        SttProviderType::Whisper => transcribe_whisper(&wav_bytes, config).await,
        SttProviderType::Deepgram => transcribe_deepgram(&wav_bytes, config).await,
        SttProviderType::Custom(url) => transcribe_custom(url, &wav_bytes, config).await,
    }
}

/// Convert f32 PCM samples to a WAV byte buffer.
fn pcm_to_wav(samples: &[f32], sample_rate: u32, channels: u16) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = channels * (bits_per_sample / 8);
    let data_size = samples.len() as u32 * (bits_per_sample as u32 / 8);
    let file_size = 36 + data_size;

    let mut buf = Vec::with_capacity(file_size as usize + 8);

    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&file_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // chunk size
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());

    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let i16_val = (clamped * i16::MAX as f32) as i16;
        buf.extend_from_slice(&i16_val.to_le_bytes());
    }

    buf
}

async fn transcribe_whisper(wav_bytes: &[u8], config: &SttConfig) -> Result<String, ToolError> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .or_else(|_| std::env::var("VOICE_TOOLS_OPENAI_KEY"))
        .map_err(|_| {
            ToolError::ExecutionFailed(
                "Whisper STT requires OPENAI_API_KEY or VOICE_TOOLS_OPENAI_KEY".into(),
            )
        })?;

    let client = reqwest::Client::new();
    let part = reqwest::multipart::Part::bytes(wav_bytes.to_vec())
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

    let model = config.model.as_deref().unwrap_or("whisper-1");

    let mut form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", model.to_string());

    if let Some(ref lang) = config.language {
        form = form.text("language", lang.clone());
    }

    let resp = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Whisper API: {e}")))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ToolError::ExecutionFailed(format!("Whisper error: {body}")));
    }

    let json: Value = resp
        .json()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Whisper JSON: {e}")))?;

    Ok(json
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string())
}

async fn transcribe_deepgram(wav_bytes: &[u8], config: &SttConfig) -> Result<String, ToolError> {
    let api_key = std::env::var("DEEPGRAM_API_KEY")
        .map_err(|_| ToolError::ExecutionFailed("Deepgram STT requires DEEPGRAM_API_KEY".into()))?;

    let model = config.model.as_deref().unwrap_or("nova-2");

    // Validate model name to prevent injection
    if !model
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ToolError::InvalidParams(
            "DEEPGRAM_MODEL must be alphanumeric (plus '-' or '_')".into(),
        ));
    }

    let mut url = format!("https://api.deepgram.com/v1/listen?model={model}");
    if let Some(ref lang) = config.language {
        url.push_str(&format!("&language={lang}"));
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Authorization", format!("Token {}", api_key))
        .header("Content-Type", "audio/wav")
        .body(wav_bytes.to_vec())
        .send()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Deepgram: {e}")))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ToolError::ExecutionFailed(format!(
            "Deepgram error: {body}"
        )));
    }

    let json: Value = resp
        .json()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Deepgram JSON: {e}")))?;

    Ok(json
        .pointer("/results/channels/0/alternatives/0/transcript")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string())
}

async fn transcribe_custom(
    endpoint: &str,
    wav_bytes: &[u8],
    _config: &SttConfig,
) -> Result<String, ToolError> {
    let client = reqwest::Client::new();
    let mut req = client
        .post(endpoint)
        .header("Content-Type", "audio/wav")
        .body(wav_bytes.to_vec());

    if let Ok(h) = std::env::var("HERMES_CUSTOM_STT_AUTH_HEADER") {
        req = req.header("Authorization", h);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Custom STT: {e}")))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(ToolError::ExecutionFailed(format!(
            "Custom STT error: {body}"
        )));
    }

    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ct.contains("json") {
        let json: Value = resp
            .json()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Custom STT JSON: {e}")))?;
        Ok(json
            .get("text")
            .or_else(|| json.get("transcript"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string())
    } else {
        resp.text()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Custom STT body: {e}")))
    }
}

// ---------------------------------------------------------------------------
// Voice Pipeline
// ---------------------------------------------------------------------------

/// Full voice pipeline state, shared between the tool handler and the CLI loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoicePipelineConfig {
    pub stt: SttConfig,
    pub tts_provider: String,
    pub tts_voice: Option<String>,
    pub vad: VadConfig,
    /// Whether to auto-play TTS for every assistant response.
    pub auto_speak: bool,
    /// Maximum recording duration per utterance (seconds).
    pub max_record_secs: u64,
}

impl Default for VoicePipelineConfig {
    fn default() -> Self {
        Self {
            stt: SttConfig::default(),
            tts_provider: "openai".into(),
            tts_voice: None,
            vad: VadConfig::default(),
            auto_speak: true,
            max_record_secs: 30,
        }
    }
}

/// Shared voice pipeline state that the CLI event loop reads.
pub struct VoicePipeline {
    config: Mutex<VoicePipelineConfig>,
    /// Watch channel so the CLI loop can react to config changes.
    config_tx: watch::Sender<VoicePipelineConfig>,
    config_rx: watch::Receiver<VoicePipelineConfig>,
}

impl VoicePipeline {
    pub fn new(config: VoicePipelineConfig) -> Self {
        let (tx, rx) = watch::channel(config.clone());
        Self {
            config: Mutex::new(config),
            config_tx: tx,
            config_rx: rx,
        }
    }

    /// Get a watch receiver for config changes.
    pub fn subscribe(&self) -> watch::Receiver<VoicePipelineConfig> {
        self.config_rx.clone()
    }

    /// Update the pipeline configuration.
    pub async fn update_config<F>(&self, f: F)
    where
        F: FnOnce(&mut VoicePipelineConfig),
    {
        let mut cfg = self.config.lock().await;
        f(&mut cfg);
        let _ = self.config_tx.send(cfg.clone());
    }

    /// Get a snapshot of the current config.
    pub async fn current_config(&self) -> VoicePipelineConfig {
        self.config.lock().await.clone()
    }

    /// Run the recording→VAD→STT pipeline for a single utterance.
    ///
    /// This collects audio from the source until VAD detects end-of-speech
    /// or the max duration is reached, then transcribes the collected audio.
    pub async fn record_and_transcribe(
        &self,
        source: &dyn AudioSource,
    ) -> Result<String, ToolError> {
        let cfg = self.config.lock().await.clone();
        let mut vad = VoiceActivityDetector::new(cfg.vad.clone());
        let mut audio_buffer: Vec<f32> = Vec::new();
        let max_samples = cfg.max_record_secs as usize * source.sample_rate() as usize;
        let mut recording = false;

        let deadline = Instant::now() + Duration::from_secs(cfg.max_record_secs);

        loop {
            if Instant::now() >= deadline {
                break;
            }

            let chunk = match source.read_chunk().await {
                Some(c) => c,
                None => break,
            };

            let is_speech = vad.process_frame(&chunk);

            if is_speech {
                recording = true;
                audio_buffer.extend_from_slice(&chunk);
            } else if recording {
                // Speech ended — we have a complete utterance
                break;
            }

            if audio_buffer.len() >= max_samples {
                break;
            }
        }

        if audio_buffer.is_empty() {
            return Ok(String::new());
        }

        tracing::debug!(
            samples = audio_buffer.len(),
            duration_ms = audio_buffer.len() as u64 * 1000 / source.sample_rate() as u64,
            "Voice recording complete, transcribing"
        );

        transcribe_audio(&cfg.stt, &audio_buffer, source.sample_rate()).await
    }
}

impl Default for VoicePipeline {
    fn default() -> Self {
        Self::new(VoicePipelineConfig::default())
    }
}

// ---------------------------------------------------------------------------
// VoiceModeHandler — tool the LLM invokes
// ---------------------------------------------------------------------------

/// Tool handler for voice mode. The LLM can:
/// - Toggle voice mode on/off
/// - Configure STT provider (whisper / deepgram / custom)
/// - Configure TTS provider and voice
/// - Adjust VAD sensitivity
/// - Query current voice mode status
pub struct VoiceModeHandler {
    pipeline: Arc<VoicePipeline>,
}

impl VoiceModeHandler {
    pub fn new(pipeline: Arc<VoicePipeline>) -> Self {
        Self { pipeline }
    }

    pub fn with_default_pipeline() -> Self {
        Self::new(Arc::new(VoicePipeline::default()))
    }

    /// Get a reference to the underlying pipeline (for CLI integration).
    pub fn pipeline(&self) -> &Arc<VoicePipeline> {
        &self.pipeline
    }
}

impl Default for VoiceModeHandler {
    fn default() -> Self {
        Self::with_default_pipeline()
    }
}

#[async_trait]
impl ToolHandler for VoiceModeHandler {
    async fn execute(&self, params: Value) -> Result<String, ToolError> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("toggle");

        match action {
            "enable" | "on" => {
                let previous = VOICE_MODE_ENABLED.swap(true, Ordering::Relaxed);
                // Apply any provider overrides from params
                self.apply_config_overrides(&params).await;
                let cfg = self.pipeline.current_config().await;
                Ok(json!({
                    "voice_mode": true,
                    "previous": previous,
                    "status": "enabled",
                    "stt_provider": format!("{:?}", cfg.stt.provider),
                    "tts_provider": cfg.tts_provider,
                    "auto_speak": cfg.auto_speak,
                    "note": "Voice mode on: CLI will record mic input, transcribe via STT, and speak replies via TTS."
                })
                .to_string())
            }
            "disable" | "off" => {
                let previous = VOICE_MODE_ENABLED.swap(false, Ordering::Relaxed);
                Ok(json!({
                    "voice_mode": false,
                    "previous": previous,
                    "status": "disabled",
                    "note": "Voice mode off: CLI will use text-only IO."
                })
                .to_string())
            }
            "toggle" => {
                let previous = VOICE_MODE_ENABLED.load(Ordering::Relaxed);
                let new_state = params
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(!previous);
                VOICE_MODE_ENABLED.store(new_state, Ordering::Relaxed);

                if new_state {
                    self.apply_config_overrides(&params).await;
                }

                let cfg = self.pipeline.current_config().await;
                Ok(json!({
                    "voice_mode": new_state,
                    "previous": previous,
                    "status": if new_state { "enabled" } else { "disabled" },
                    "stt_provider": format!("{:?}", cfg.stt.provider),
                    "tts_provider": cfg.tts_provider,
                    "note": if new_state {
                        "Voice mode on: CLI will record mic input, transcribe via STT, and speak replies via TTS."
                    } else {
                        "Voice mode off: CLI will use text-only IO."
                    }
                })
                .to_string())
            }
            "configure" => {
                self.apply_config_overrides(&params).await;
                let cfg = self.pipeline.current_config().await;
                Ok(json!({
                    "voice_mode": voice_mode_enabled(),
                    "status": "configured",
                    "stt_provider": format!("{:?}", cfg.stt.provider),
                    "stt_language": cfg.stt.language,
                    "stt_model": cfg.stt.model,
                    "tts_provider": cfg.tts_provider,
                    "tts_voice": cfg.tts_voice,
                    "vad_energy_threshold": cfg.vad.energy_threshold,
                    "vad_silence_timeout_ms": cfg.vad.silence_timeout_ms,
                    "auto_speak": cfg.auto_speak,
                    "max_record_secs": cfg.max_record_secs,
                })
                .to_string())
            }
            "status" => {
                let cfg = self.pipeline.current_config().await;
                Ok(json!({
                    "voice_mode": voice_mode_enabled(),
                    "stt_provider": format!("{:?}", cfg.stt.provider),
                    "stt_language": cfg.stt.language,
                    "tts_provider": cfg.tts_provider,
                    "tts_voice": cfg.tts_voice,
                    "auto_speak": cfg.auto_speak,
                    "vad_energy_threshold": cfg.vad.energy_threshold,
                    "max_record_secs": cfg.max_record_secs,
                })
                .to_string())
            }
            other => Err(ToolError::InvalidParams(format!(
                "Unknown voice_mode action: '{other}'. Use: enable, disable, toggle, configure, status"
            ))),
        }
    }

    fn schema(&self) -> ToolSchema {
        let mut props = IndexMap::new();
        props.insert(
            "action".into(),
            json!({
                "type": "string",
                "description": "Action to perform",
                "enum": ["enable", "disable", "toggle", "configure", "status"],
                "default": "toggle"
            }),
        );
        props.insert(
            "enabled".into(),
            json!({
                "type": "boolean",
                "description": "Enable (true) or disable (false) voice mode. Used with 'toggle' action."
            }),
        );
        props.insert(
            "stt_provider".into(),
            json!({
                "type": "string",
                "description": "STT provider: 'whisper', 'deepgram', or a custom endpoint URL",
                "enum": ["whisper", "deepgram"]
            }),
        );
        props.insert(
            "stt_language".into(),
            json!({
                "type": "string",
                "description": "Language code for STT (e.g. 'en', 'zh', 'ja')"
            }),
        );
        props.insert(
            "tts_provider".into(),
            json!({
                "type": "string",
                "description": "TTS provider: 'openai', 'elevenlabs', 'minimax'",
                "enum": ["openai", "elevenlabs", "minimax"]
            }),
        );
        props.insert(
            "tts_voice".into(),
            json!({
                "type": "string",
                "description": "Voice ID for TTS (provider-specific)"
            }),
        );
        props.insert(
            "auto_speak".into(),
            json!({
                "type": "boolean",
                "description": "Auto-play TTS for every assistant response"
            }),
        );
        props.insert(
            "vad_sensitivity".into(),
            json!({
                "type": "string",
                "description": "VAD sensitivity: 'low', 'medium', 'high'",
                "enum": ["low", "medium", "high"]
            }),
        );
        tool_schema(
            "voice_mode",
            "Manage voice mode (STT→Agent→TTS loop). Actions: enable/disable/toggle voice mode, \
             configure STT/TTS providers and VAD sensitivity, or query current status. \
             When enabled, the CLI records mic input, transcribes via STT, and speaks replies via TTS.",
            JsonSchema::object(props, vec![]),
        )
    }
}

impl VoiceModeHandler {
    /// Apply configuration overrides from tool params.
    async fn apply_config_overrides(&self, params: &Value) {
        self.pipeline
            .update_config(|cfg| {
                if let Some(provider) = params.get("stt_provider").and_then(|v| v.as_str()) {
                    cfg.stt.provider = match provider {
                        "whisper" => SttProviderType::Whisper,
                        "deepgram" => SttProviderType::Deepgram,
                        url => SttProviderType::Custom(url.to_string()),
                    };
                }
                if let Some(lang) = params.get("stt_language").and_then(|v| v.as_str()) {
                    cfg.stt.language = Some(lang.to_string());
                }
                if let Some(provider) = params.get("tts_provider").and_then(|v| v.as_str()) {
                    cfg.tts_provider = provider.to_string();
                }
                if let Some(voice) = params.get("tts_voice").and_then(|v| v.as_str()) {
                    cfg.tts_voice = Some(voice.to_string());
                }
                if let Some(auto) = params.get("auto_speak").and_then(|v| v.as_bool()) {
                    cfg.auto_speak = auto;
                }
                if let Some(sensitivity) = params.get("vad_sensitivity").and_then(|v| v.as_str()) {
                    match sensitivity {
                        "low" => {
                            cfg.vad.energy_threshold = 0.04;
                            cfg.vad.min_speech_frames = 5;
                        }
                        "medium" => {
                            cfg.vad.energy_threshold = 0.02;
                            cfg.vad.min_speech_frames = 3;
                        }
                        "high" => {
                            cfg.vad.energy_threshold = 0.01;
                            cfg.vad.min_speech_frames = 2;
                        }
                        _ => {}
                    }
                }
            })
            .await;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- VAD tests -----------------------------------------------------------

    #[test]
    fn vad_silence_not_detected_as_speech() {
        let mut vad = VoiceActivityDetector::new(VadConfig::default());
        let silence = vec![0.0f32; 480];
        assert!(!vad.process_frame(&silence));
        assert!(!vad.is_speech_active());
    }

    #[test]
    fn vad_loud_signal_detected_as_speech() {
        let mut vad = VoiceActivityDetector::new(VadConfig {
            min_speech_frames: 2,
            ..Default::default()
        });
        // Generate a simple sine wave (speech-like)
        let samples: Vec<f32> = (0..480).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        // First frame: not yet enough consecutive
        vad.process_frame(&samples);
        // Second frame: should trigger
        assert!(vad.process_frame(&samples));
        assert!(vad.is_speech_active());
    }

    #[test]
    fn vad_reset_clears_state() {
        let mut vad = VoiceActivityDetector::new(VadConfig {
            min_speech_frames: 1,
            ..Default::default()
        });
        let loud: Vec<f32> = (0..480).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        vad.process_frame(&loud);
        assert!(vad.is_speech_active());
        vad.reset();
        assert!(!vad.is_speech_active());
    }

    #[test]
    fn vad_empty_frame() {
        let mut vad = VoiceActivityDetector::new(VadConfig::default());
        assert!(!vad.process_frame(&[]));
    }

    #[test]
    fn compute_rms_known_values() {
        let samples = vec![0.5, -0.5, 0.5, -0.5];
        let rms = VoiceActivityDetector::compute_rms(&samples);
        assert!((rms - 0.5).abs() < 0.001);
    }

    #[test]
    fn compute_zcr_known_values() {
        // Alternating sign → max ZCR
        let samples = vec![1.0, -1.0, 1.0, -1.0, 1.0];
        let zcr = VoiceActivityDetector::compute_zcr(&samples);
        assert!((zcr - 1.0).abs() < 0.001);

        // Same sign → zero ZCR
        let samples = vec![1.0, 1.0, 1.0, 1.0];
        let zcr = VoiceActivityDetector::compute_zcr(&samples);
        assert!(zcr < 0.001);
    }

    // -- PCM to WAV tests ----------------------------------------------------

    #[test]
    fn pcm_to_wav_produces_valid_header() {
        let samples = vec![0.0f32; 160];
        let wav = pcm_to_wav(&samples, 16000, 1);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        // data chunk
        let data_pos = 36;
        assert_eq!(&wav[data_pos..data_pos + 4], b"data");
        // data size = 160 samples * 2 bytes
        let data_size = u32::from_le_bytes([
            wav[data_pos + 4],
            wav[data_pos + 5],
            wav[data_pos + 6],
            wav[data_pos + 7],
        ]);
        assert_eq!(data_size, 320);
    }

    // -- VoiceModeHandler tests ----------------------------------------------
    // NB: VOICE_MODE_ENABLED is a process-wide atomic, so all tests that
    // mutate it are folded into a single sequential test to avoid races.

    #[tokio::test]
    async fn handler_all_actions_sequential() {
        let handler = VoiceModeHandler::default();

        // 1. Enable
        let out = handler.execute(json!({"action": "enable"})).await.unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["voice_mode"], json!(true));
        assert_eq!(v["status"], "enabled");
        assert!(voice_mode_enabled());

        // 2. Status while enabled
        let out = handler.execute(json!({"action": "status"})).await.unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["voice_mode"], json!(true));
        assert!(v.get("stt_provider").is_some());
        assert!(v.get("tts_provider").is_some());

        // 3. Disable
        let out = handler.execute(json!({"action": "disable"})).await.unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["voice_mode"], json!(false));
        assert!(!voice_mode_enabled());

        // 4. Toggle (off → on)
        handler.execute(json!({})).await.unwrap();
        assert!(voice_mode_enabled());

        // 5. Toggle (on → off)
        handler.execute(json!({})).await.unwrap();
        assert!(!voice_mode_enabled());

        // 6. Explicit enabled param
        handler.execute(json!({"enabled": true})).await.unwrap();
        assert!(voice_mode_enabled());

        handler.execute(json!({"enabled": false})).await.unwrap();
        assert!(!voice_mode_enabled());

        // 7. Configure
        handler
            .execute(json!({
                "action": "configure",
                "stt_provider": "deepgram",
                "stt_language": "ja",
                "tts_provider": "elevenlabs",
                "auto_speak": false,
                "vad_sensitivity": "high"
            }))
            .await
            .unwrap();

        let cfg = handler.pipeline.current_config().await;
        assert_eq!(cfg.stt.provider, SttProviderType::Deepgram);
        assert_eq!(cfg.stt.language.as_deref(), Some("ja"));
        assert_eq!(cfg.tts_provider, "elevenlabs");
        assert!(!cfg.auto_speak);
        assert!((cfg.vad.energy_threshold - 0.01).abs() < 0.001);

        // 8. Invalid action
        let err = handler
            .execute(json!({"action": "bogus"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Unknown voice_mode action"));
    }

    // -- Pipeline config watch -----------------------------------------------

    #[tokio::test]
    async fn pipeline_config_watch() {
        let pipeline = VoicePipeline::default();
        let mut rx = pipeline.subscribe();

        pipeline
            .update_config(|cfg| {
                cfg.tts_provider = "elevenlabs".into();
            })
            .await;

        rx.changed().await.unwrap();
        let cfg = rx.borrow().clone();
        assert_eq!(cfg.tts_provider, "elevenlabs");
    }
}

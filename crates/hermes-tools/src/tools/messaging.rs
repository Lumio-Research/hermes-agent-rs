//! Messaging tool: send messages across platforms.
//!
//! This module provides the `send_message` tool that the LLM invokes to
//! deliver messages to users on any supported platform (Telegram, Discord,
//! Slack, WhatsApp, Signal, Email, SMS, etc.).
//!
//! Architecture:
//! ```text
//!   LLM → send_message tool → MessagingBackend trait → GatewayMessagingBackend
//!                                                          ↓
//!                                                    DeliveryRouter
//!                                                    ↓         ↓
//!                                              Telegram    Discord  ...
//! ```
//!
//! The tool handler resolves channel references (aliases, platform prefixes,
//! bare IDs), splits long messages at safe markdown boundaries, handles
//! media attachments, and implements retry with fallback.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use hermes_core::{tool_schema, JsonSchema, ToolError, ToolHandler, ToolSchema};

// ---------------------------------------------------------------------------
// Channel resolution
// ---------------------------------------------------------------------------

/// A resolved channel target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedChannel {
    /// Platform name (e.g. "telegram", "discord", "slack").
    pub platform: String,
    /// Chat/channel/user ID on that platform.
    pub chat_id: String,
    /// Human-readable display name (if known).
    pub display_name: Option<String>,
}

/// Parse a channel reference string into a `ResolvedChannel`.
///
/// Supported formats:
/// - `"telegram:12345"` — explicit platform:id
/// - `"discord:channel_id"` — explicit platform:id
/// - `"email:user@example.com"` — email address
/// - `"sms:+1234567890"` — phone number
/// - `"12345"` — bare ID (requires `default_platform`)
/// - `"@username"` — platform-specific username (requires `default_platform`)
pub fn resolve_channel(
    channel_ref: &str,
    default_platform: Option<&str>,
) -> Result<ResolvedChannel, ToolError> {
    let trimmed = channel_ref.trim();
    if trimmed.is_empty() {
        return Err(ToolError::InvalidParams(
            "Empty channel reference".into(),
        ));
    }

    // Try platform:id format
    if let Some((platform, id)) = trimmed.split_once(':') {
        let platform = platform.trim().to_lowercase();
        let id = id.trim();
        if !id.is_empty() && is_known_platform(&platform) {
            return Ok(ResolvedChannel {
                platform,
                chat_id: id.to_string(),
                display_name: None,
            });
        }
    }

    // Email detection
    if trimmed.contains('@') && trimmed.contains('.') && !trimmed.starts_with('@') {
        return Ok(ResolvedChannel {
            platform: "email".into(),
            chat_id: trimmed.to_string(),
            display_name: None,
        });
    }

    // Phone number detection
    if trimmed.starts_with('+') && trimmed[1..].chars().all(|c| c.is_ascii_digit() || c == '-' || c == ' ') {
        return Ok(ResolvedChannel {
            platform: "sms".into(),
            chat_id: trimmed.to_string(),
            display_name: None,
        });
    }

    // Bare ID or @username — use default platform
    if let Some(platform) = default_platform {
        return Ok(ResolvedChannel {
            platform: platform.to_lowercase(),
            chat_id: trimmed.to_string(),
            display_name: None,
        });
    }

    Err(ToolError::InvalidParams(format!(
        "Cannot resolve channel '{}': no platform prefix and no default platform configured. \
         Use format 'platform:id' (e.g. 'telegram:12345', 'discord:channel_id').",
        trimmed
    )))
}

fn is_known_platform(name: &str) -> bool {
    matches!(
        name,
        "telegram"
            | "discord"
            | "slack"
            | "whatsapp"
            | "signal"
            | "email"
            | "sms"
            | "matrix"
            | "mattermost"
            | "weixin"
            | "wechat"
            | "wecom"
            | "dingtalk"
            | "feishu"
            | "qqbot"
            | "bluebubbles"
            | "homeassistant"
            | "webhook"
            | "api"
    )
}

// ---------------------------------------------------------------------------
// MessagingBackend trait
// ---------------------------------------------------------------------------

/// Delivery result for a single message send.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryResult {
    pub platform: String,
    pub chat_id: String,
    pub status: DeliveryStatus,
    /// Number of message chunks sent (for split messages).
    pub chunks_sent: usize,
    /// Error message if failed.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    Delivered,
    Queued,
    Failed,
    PartiallyDelivered,
}

/// Backend for sending messages across platforms.
///
/// The gateway layer provides a real implementation backed by `DeliveryRouter`
/// and platform adapters. The tool layer only sees this trait.
#[async_trait]
pub trait MessagingBackend: Send + Sync {
    /// Send a text message to a recipient on a platform.
    async fn send(
        &self,
        platform: &str,
        recipient: &str,
        message: &str,
    ) -> Result<String, ToolError>;

    /// Send a text message with full delivery tracking.
    async fn send_tracked(
        &self,
        channel: &ResolvedChannel,
        message: &str,
        _split_long: bool,
        _max_chunk_size: usize,
    ) -> Result<DeliveryResult, ToolError> {
        // Default implementation: delegate to simple send
        let result = self
            .send(&channel.platform, &channel.chat_id, message)
            .await?;
        let status = if result.contains("\"status\":\"pending\"") {
            DeliveryStatus::Queued
        } else {
            DeliveryStatus::Delivered
        };
        Ok(DeliveryResult {
            platform: channel.platform.clone(),
            chat_id: channel.chat_id.clone(),
            status,
            chunks_sent: 1,
            error: None,
        })
    }

    /// Send a file/media attachment.
    async fn send_file(
        &self,
        platform: &str,
        _recipient: &str,
        _file_path: &str,
        _caption: Option<&str>,
    ) -> Result<String, ToolError> {
        // Default: not supported
        Err(ToolError::ExecutionFailed(format!(
            "File sending not supported for platform '{platform}'"
        )))
    }

    /// List available/registered platforms.
    async fn available_platforms(&self) -> Vec<String> {
        vec![]
    }
}

// ---------------------------------------------------------------------------
// Message splitting helper
// ---------------------------------------------------------------------------

/// Platform-specific message length limits.
fn platform_max_length(platform: &str) -> usize {
    match platform {
        "telegram" => 4096,
        "discord" => 2000,
        "slack" => 40000,
        "whatsapp" => 65536,
        "signal" => 65536,
        "email" => 1_000_000,
        "sms" => 1600,
        "matrix" => 65536,
        "mattermost" => 16383,
        _ => 4096, // conservative default
    }
}

/// Simple markdown-safe split. For the full implementation, see
/// `hermes_gateway::markdown_split::split_markdown`.
fn split_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_len {
            chunks.push(remaining.to_string());
            break;
        }

        // Find a safe split point
        let search = &remaining[..max_len.min(remaining.len())];
        let split_at = search
            .rfind("\n\n")
            .map(|p| p + 1)
            .or_else(|| search.rfind('\n').map(|p| p + 1))
            .or_else(|| search.rfind(' ').map(|p| p + 1))
            .unwrap_or(max_len.min(remaining.len()));

        let (chunk, rest) = remaining.split_at(split_at);
        chunks.push(chunk.to_string());
        remaining = rest;
    }

    chunks
}

// ---------------------------------------------------------------------------
// SendMessageHandler — the tool the LLM invokes
// ---------------------------------------------------------------------------

/// Configuration for the send_message tool.
#[derive(Debug, Clone)]
pub struct SendMessageConfig {
    /// Default platform when recipient has no prefix.
    pub default_platform: Option<String>,
    /// Fallback platform if primary delivery fails.
    pub fallback_platform: Option<String>,
    /// Maximum retry attempts per delivery.
    pub max_retries: u32,
    /// Delay between retries.
    pub retry_delay: Duration,
    /// Whether to auto-split long messages.
    pub auto_split: bool,
}

impl Default for SendMessageConfig {
    fn default() -> Self {
        Self {
            default_platform: None,
            fallback_platform: None,
            max_retries: 2,
            retry_delay: Duration::from_secs(1),
            auto_split: true,
        }
    }
}

/// Tool for sending messages across platforms.
pub struct SendMessageHandler {
    backend: Arc<dyn MessagingBackend>,
    config: SendMessageConfig,
}

impl SendMessageHandler {
    pub fn new(backend: Arc<dyn MessagingBackend>) -> Self {
        Self {
            backend,
            config: SendMessageConfig::default(),
        }
    }

    pub fn with_config(backend: Arc<dyn MessagingBackend>, config: SendMessageConfig) -> Self {
        Self { backend, config }
    }

    /// Deliver a message with retry and optional fallback.
    async fn deliver_with_retry(
        &self,
        channel: &ResolvedChannel,
        message: &str,
    ) -> Result<DeliveryResult, ToolError> {
        let max_len = platform_max_length(&channel.platform);
        let chunks = if self.config.auto_split {
            split_message(message, max_len)
        } else {
            vec![message.to_string()]
        };

        let mut delivered = 0;
        let mut last_error: Option<String> = None;

        for (i, chunk) in chunks.iter().enumerate() {
            let mut attempts = 0;
            loop {
                attempts += 1;
                match self
                    .backend
                    .send(&channel.platform, &channel.chat_id, chunk)
                    .await
                {
                    Ok(_) => {
                        delivered += 1;
                        break;
                    }
                    Err(e) => {
                        last_error = Some(e.to_string());
                        if attempts > self.config.max_retries {
                            tracing::warn!(
                                platform = %channel.platform,
                                chat_id = %channel.chat_id,
                                chunk = i,
                                attempts = attempts,
                                error = %e,
                                "Message delivery failed after retries"
                            );
                            break;
                        }
                        tracing::debug!(
                            platform = %channel.platform,
                            attempt = attempts,
                            error = %e,
                            "Retrying message delivery"
                        );
                        tokio::time::sleep(self.config.retry_delay).await;
                    }
                }
            }
        }

        let status = if delivered == chunks.len() {
            DeliveryStatus::Delivered
        } else if delivered > 0 {
            DeliveryStatus::PartiallyDelivered
        } else {
            DeliveryStatus::Failed
        };

        Ok(DeliveryResult {
            platform: channel.platform.clone(),
            chat_id: channel.chat_id.clone(),
            status,
            chunks_sent: delivered,
            error: if status != DeliveryStatus::Delivered {
                last_error
            } else {
                None
            },
        })
    }

    /// Try fallback platform if primary delivery failed.
    async fn try_fallback(
        &self,
        original_channel: &ResolvedChannel,
        message: &str,
        primary_result: &DeliveryResult,
    ) -> Option<DeliveryResult> {
        if primary_result.status == DeliveryStatus::Delivered {
            return None;
        }

        let fallback_platform = self.config.fallback_platform.as_deref()?;
        if fallback_platform == original_channel.platform {
            return None;
        }

        tracing::info!(
            primary = %original_channel.platform,
            fallback = %fallback_platform,
            "Primary delivery failed, trying fallback platform"
        );

        let fallback_channel = ResolvedChannel {
            platform: fallback_platform.to_string(),
            chat_id: original_channel.chat_id.clone(),
            display_name: original_channel.display_name.clone(),
        };

        match self.deliver_with_retry(&fallback_channel, message).await {
            Ok(result) => Some(result),
            Err(e) => {
                tracing::warn!(
                    fallback = %fallback_platform,
                    error = %e,
                    "Fallback delivery also failed"
                );
                None
            }
        }
    }
}

#[async_trait]
impl ToolHandler for SendMessageHandler {
    async fn execute(&self, params: Value) -> Result<String, ToolError> {
        let message = params
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'message' parameter".into()))?;

        if message.trim().is_empty() {
            return Err(ToolError::InvalidParams("Message cannot be empty".into()));
        }

        // Resolve the channel
        let channel = if let Some(platform) = params.get("platform").and_then(|v| v.as_str()) {
            let recipient = params
                .get("recipient")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::InvalidParams("Missing 'recipient' parameter".into())
                })?;
            ResolvedChannel {
                platform: platform.to_lowercase(),
                chat_id: recipient.to_string(),
                display_name: None,
            }
        } else if let Some(channel_ref) = params.get("channel").and_then(|v| v.as_str()) {
            resolve_channel(channel_ref, self.config.default_platform.as_deref())?
        } else if let Some(recipient) = params.get("recipient").and_then(|v| v.as_str()) {
            resolve_channel(recipient, self.config.default_platform.as_deref())?
        } else {
            return Err(ToolError::InvalidParams(
                "Must provide either 'platform'+'recipient' or 'channel' parameter".into(),
            ));
        };

        // Handle file attachment
        if let Some(file_path) = params.get("file").and_then(|v| v.as_str()) {
            let caption = params.get("caption").and_then(|v| v.as_str());
            let file_result = self
                .backend
                .send_file(&channel.platform, &channel.chat_id, file_path, caption)
                .await;

            return match file_result {
                Ok(result) => Ok(json!({
                    "status": "delivered",
                    "platform": channel.platform,
                    "recipient": channel.chat_id,
                    "type": "file",
                    "file": file_path,
                    "result": result,
                })
                .to_string()),
                Err(e) => Ok(json!({
                    "status": "failed",
                    "platform": channel.platform,
                    "recipient": channel.chat_id,
                    "type": "file",
                    "error": e.to_string(),
                })
                .to_string()),
            };
        }

        // Deliver text message with retry
        let result = self.deliver_with_retry(&channel, message).await?;

        // Try fallback if needed
        let fallback_result = self.try_fallback(&channel, message, &result).await;

        let final_status = if result.status == DeliveryStatus::Delivered {
            &result
        } else if let Some(ref fb) = fallback_result {
            if fb.status == DeliveryStatus::Delivered {
                fb
            } else {
                &result
            }
        } else {
            &result
        };

        Ok(json!({
            "status": format!("{:?}", final_status.status).to_lowercase(),
            "platform": final_status.platform,
            "recipient": final_status.chat_id,
            "chunks_sent": final_status.chunks_sent,
            "error": final_status.error,
            "fallback_used": fallback_result.as_ref().map(|fb| fb.status == DeliveryStatus::Delivered).unwrap_or(false),
            "fallback_platform": fallback_result.as_ref().map(|fb| &fb.platform),
        })
        .to_string())
    }

    fn schema(&self) -> ToolSchema {
        let mut props = IndexMap::new();
        props.insert(
            "platform".into(),
            json!({
                "type": "string",
                "description": "Platform to send the message on (e.g. 'telegram', 'discord', 'slack'). \
                                Can be omitted if 'channel' uses platform:id format.",
                "enum": ["telegram", "discord", "slack", "whatsapp", "signal", "email", "sms",
                         "matrix", "mattermost", "weixin", "dingtalk", "feishu", "qqbot",
                         "bluebubbles", "webhook"]
            }),
        );
        props.insert(
            "recipient".into(),
            json!({
                "type": "string",
                "description": "Recipient identifier (chat ID, user ID, email, phone number). \
                                Can also use 'platform:id' format (e.g. 'telegram:12345')."
            }),
        );
        props.insert(
            "channel".into(),
            json!({
                "type": "string",
                "description": "Channel reference in 'platform:id' format (e.g. 'telegram:12345', \
                                'discord:channel_id'). Alternative to separate platform+recipient."
            }),
        );
        props.insert(
            "message".into(),
            json!({
                "type": "string",
                "description": "Message content to send. Supports Markdown formatting; \
                                automatically converted to platform-specific format."
            }),
        );
        props.insert(
            "file".into(),
            json!({
                "type": "string",
                "description": "Path to a file to send as attachment (image, audio, document)."
            }),
        );
        props.insert(
            "caption".into(),
            json!({
                "type": "string",
                "description": "Caption for file attachments."
            }),
        );

        tool_schema(
            "send_message",
            "Send a message or file to a recipient on any supported platform. \
             Supports Telegram, Discord, Slack, WhatsApp, Signal, Email, SMS, Matrix, \
             and more. Long messages are automatically split at safe markdown boundaries. \
             Failed deliveries are retried and can fall back to an alternate platform.",
            JsonSchema::object(
                props,
                vec!["message".into()],
            ),
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    // -- Channel resolution tests --------------------------------------------

    #[test]
    fn resolve_platform_prefix() {
        let ch = resolve_channel("telegram:12345", None).unwrap();
        assert_eq!(ch.platform, "telegram");
        assert_eq!(ch.chat_id, "12345");
    }

    #[test]
    fn resolve_discord_prefix() {
        let ch = resolve_channel("discord:channel_abc", None).unwrap();
        assert_eq!(ch.platform, "discord");
        assert_eq!(ch.chat_id, "channel_abc");
    }

    #[test]
    fn resolve_email() {
        let ch = resolve_channel("user@example.com", None).unwrap();
        assert_eq!(ch.platform, "email");
        assert_eq!(ch.chat_id, "user@example.com");
    }

    #[test]
    fn resolve_phone() {
        let ch = resolve_channel("+1-234-567-8900", None).unwrap();
        assert_eq!(ch.platform, "sms");
    }

    #[test]
    fn resolve_bare_id_with_default() {
        let ch = resolve_channel("12345", Some("telegram")).unwrap();
        assert_eq!(ch.platform, "telegram");
        assert_eq!(ch.chat_id, "12345");
    }

    #[test]
    fn resolve_bare_id_without_default_fails() {
        let err = resolve_channel("12345", None).unwrap_err();
        assert!(err.to_string().contains("Cannot resolve"));
    }

    #[test]
    fn resolve_empty_fails() {
        let err = resolve_channel("", None).unwrap_err();
        assert!(err.to_string().contains("Empty"));
    }

    #[test]
    fn resolve_at_username_with_default() {
        let ch = resolve_channel("@username", Some("telegram")).unwrap();
        assert_eq!(ch.platform, "telegram");
        assert_eq!(ch.chat_id, "@username");
    }

    // -- Message splitting tests ---------------------------------------------

    #[test]
    fn split_short_message() {
        let chunks = split_message("hello", 100);
        assert_eq!(chunks, vec!["hello"]);
    }

    #[test]
    fn split_at_paragraph() {
        let text = "paragraph one\n\nparagraph two is here";
        let chunks = split_message(text, 20);
        assert!(chunks.len() >= 2);
        assert!(chunks[0].contains("paragraph one"));
    }

    #[test]
    fn split_at_newline() {
        let text = "line one\nline two is longer than limit";
        let chunks = split_message(text, 15);
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn split_at_space() {
        let text = "word1 word2 word3 word4 word5";
        let chunks = split_message(text, 12);
        assert!(chunks.len() >= 2);
        // Should not split mid-word
        for chunk in &chunks {
            assert!(!chunk.starts_with(' '));
        }
    }

    #[test]
    fn platform_limits() {
        assert_eq!(platform_max_length("telegram"), 4096);
        assert_eq!(platform_max_length("discord"), 2000);
        assert_eq!(platform_max_length("sms"), 1600);
        assert_eq!(platform_max_length("unknown"), 4096);
    }

    // -- Mock backends -------------------------------------------------------

    struct MockMessagingBackend {
        send_count: AtomicU32,
    }

    impl MockMessagingBackend {
        fn new() -> Self {
            Self {
                send_count: AtomicU32::new(0),
            }
        }
    }

    #[async_trait]
    impl MessagingBackend for MockMessagingBackend {
        async fn send(
            &self,
            platform: &str,
            recipient: &str,
            message: &str,
        ) -> Result<String, ToolError> {
            self.send_count.fetch_add(1, Ordering::Relaxed);
            Ok(json!({
                "status": "delivered",
                "platform": platform,
                "recipient": recipient,
                "length": message.len(),
            })
            .to_string())
        }

        async fn send_file(
            &self,
            platform: &str,
            recipient: &str,
            file_path: &str,
            caption: Option<&str>,
        ) -> Result<String, ToolError> {
            Ok(json!({
                "status": "delivered",
                "platform": platform,
                "recipient": recipient,
                "file": file_path,
                "caption": caption,
            })
            .to_string())
        }

        async fn available_platforms(&self) -> Vec<String> {
            vec!["telegram".into(), "discord".into(), "slack".into()]
        }
    }

    /// Backend that fails N times then succeeds.
    struct RetryMockBackend {
        fail_count: AtomicU32,
        max_failures: u32,
    }

    impl RetryMockBackend {
        fn new(max_failures: u32) -> Self {
            Self {
                fail_count: AtomicU32::new(0),
                max_failures,
            }
        }
    }

    #[async_trait]
    impl MessagingBackend for RetryMockBackend {
        async fn send(
            &self,
            platform: &str,
            recipient: &str,
            _message: &str,
        ) -> Result<String, ToolError> {
            let n = self.fail_count.fetch_add(1, Ordering::Relaxed);
            if n < self.max_failures {
                return Err(ToolError::ExecutionFailed(format!(
                    "Simulated failure #{n}"
                )));
            }
            Ok(json!({"status": "delivered", "platform": platform, "recipient": recipient}).to_string())
        }
    }

    // -- Handler tests -------------------------------------------------------

    #[tokio::test]
    async fn handler_send_with_platform_and_recipient() {
        let handler = SendMessageHandler::new(Arc::new(MockMessagingBackend::new()));
        let result = handler
            .execute(json!({
                "platform": "telegram",
                "recipient": "12345",
                "message": "Hello!"
            }))
            .await
            .unwrap();
        let v: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["status"], "delivered");
        assert_eq!(v["platform"], "telegram");
        assert_eq!(v["recipient"], "12345");
    }

    #[tokio::test]
    async fn handler_send_with_channel_ref() {
        let handler = SendMessageHandler::new(Arc::new(MockMessagingBackend::new()));
        let result = handler
            .execute(json!({
                "channel": "discord:channel_abc",
                "message": "Hello Discord!"
            }))
            .await
            .unwrap();
        let v: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["status"], "delivered");
        assert_eq!(v["platform"], "discord");
        assert_eq!(v["recipient"], "channel_abc");
    }

    #[tokio::test]
    async fn handler_send_file() {
        let handler = SendMessageHandler::new(Arc::new(MockMessagingBackend::new()));
        let result = handler
            .execute(json!({
                "platform": "telegram",
                "recipient": "12345",
                "message": "See attached",
                "file": "/tmp/image.png",
                "caption": "A photo"
            }))
            .await
            .unwrap();
        let v: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["status"], "delivered");
        assert_eq!(v["type"], "file");
    }

    #[tokio::test]
    async fn handler_missing_message() {
        let handler = SendMessageHandler::new(Arc::new(MockMessagingBackend::new()));
        let err = handler
            .execute(json!({"platform": "telegram", "recipient": "12345"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Missing 'message'"));
    }

    #[tokio::test]
    async fn handler_empty_message() {
        let handler = SendMessageHandler::new(Arc::new(MockMessagingBackend::new()));
        let err = handler
            .execute(json!({"platform": "telegram", "recipient": "12345", "message": "  "}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[tokio::test]
    async fn handler_missing_recipient() {
        let handler = SendMessageHandler::new(Arc::new(MockMessagingBackend::new()));
        let err = handler
            .execute(json!({"message": "hello"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("platform") || err.to_string().contains("recipient") || err.to_string().contains("channel"));
    }

    #[tokio::test]
    async fn handler_retry_succeeds() {
        let backend = Arc::new(RetryMockBackend::new(1)); // fail once, then succeed
        let handler = SendMessageHandler::with_config(
            backend,
            SendMessageConfig {
                max_retries: 2,
                retry_delay: Duration::from_millis(1),
                ..Default::default()
            },
        );
        let result = handler
            .execute(json!({
                "platform": "telegram",
                "recipient": "12345",
                "message": "Hello!"
            }))
            .await
            .unwrap();
        let v: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["status"], "delivered");
    }

    #[tokio::test]
    async fn handler_retry_exhausted() {
        let backend = Arc::new(RetryMockBackend::new(100)); // always fail
        let handler = SendMessageHandler::with_config(
            backend,
            SendMessageConfig {
                max_retries: 1,
                retry_delay: Duration::from_millis(1),
                ..Default::default()
            },
        );
        let result = handler
            .execute(json!({
                "platform": "telegram",
                "recipient": "12345",
                "message": "Hello!"
            }))
            .await
            .unwrap();
        let v: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["status"], "failed");
        assert!(v["error"].as_str().unwrap().contains("Simulated"));
    }

    #[tokio::test]
    async fn handler_auto_split_long_message() {
        let backend = Arc::new(MockMessagingBackend::new());
        let handler = SendMessageHandler::new(backend.clone());
        // Create a message longer than SMS limit (1600)
        let long_msg = "x".repeat(3000);
        let result = handler
            .execute(json!({
                "platform": "sms",
                "recipient": "+1234567890",
                "message": long_msg
            }))
            .await
            .unwrap();
        let v: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["status"], "delivered");
        // Should have sent multiple chunks
        assert!(v["chunks_sent"].as_u64().unwrap() >= 2);
    }

    #[tokio::test]
    async fn handler_schema() {
        let handler = SendMessageHandler::new(Arc::new(MockMessagingBackend::new()));
        let schema = handler.schema();
        assert_eq!(schema.name, "send_message");
        assert!(schema.description.contains("platform"));
    }

    #[tokio::test]
    async fn handler_fallback_on_failure() {
        let backend = Arc::new(RetryMockBackend::new(100)); // always fail
        let handler = SendMessageHandler::with_config(
            backend,
            SendMessageConfig {
                max_retries: 0,
                retry_delay: Duration::from_millis(1),
                fallback_platform: Some("email".into()),
                ..Default::default()
            },
        );
        // Both primary and fallback will fail (same backend), but fallback should be attempted
        let result = handler
            .execute(json!({
                "platform": "telegram",
                "recipient": "12345",
                "message": "Hello!"
            }))
            .await
            .unwrap();
        let v: Value = serde_json::from_str(&result).unwrap();
        // Primary failed, fallback also failed (same mock), but fallback was attempted
        assert_eq!(v["status"], "failed");
    }
}

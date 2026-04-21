//! Real messaging backend: delegates to hermes-gateway for cross-platform sending.
//!
//! Two implementations:
//!
//! 1. **`SignalMessagingBackend`** — lightweight stub that returns a JSON envelope
//!    with `status: pending`. Used when no gateway is running (e.g. CLI-only mode).
//!    The gateway picks up these envelopes from the delivery queue.
//!
//! 2. **`GatewayMessagingBackend`** — holds an `Arc<DeliveryRouter>` and dispatches
//!    directly to platform adapters. Used when the gateway is running in-process.
//!    This is the "real" backend that achieves parity with Python's `send_message_tool`.

use async_trait::async_trait;
use serde_json::json;

use crate::tools::messaging::{DeliveryResult, DeliveryStatus, MessagingBackend, ResolvedChannel};
use hermes_core::ToolError;

// ---------------------------------------------------------------------------
// SignalMessagingBackend (queued / offline mode)
// ---------------------------------------------------------------------------

/// Messaging backend that enqueues a delivery request as JSON.
///
/// The gateway's delivery loop picks these up and routes them to the
/// appropriate platform adapter. This is the default backend when the
/// gateway is not running in-process.
pub struct SignalMessagingBackend;

impl SignalMessagingBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SignalMessagingBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessagingBackend for SignalMessagingBackend {
    async fn send(
        &self,
        platform: &str,
        recipient: &str,
        message: &str,
    ) -> Result<String, ToolError> {
        Ok(json!({
            "type": "messaging_request",
            "platform": platform,
            "recipient": recipient,
            "message": message,
            "status": "pending",
            "note": "Message queued for delivery. The gateway will route it to the platform adapter."
        })
        .to_string())
    }

    async fn send_tracked(
        &self,
        channel: &ResolvedChannel,
        message: &str,
        _split_long: bool,
        _max_chunk_size: usize,
    ) -> Result<DeliveryResult, ToolError> {
        // In signal mode, we just queue — actual delivery happens later
        self.send(&channel.platform, &channel.chat_id, message)
            .await?;
        Ok(DeliveryResult {
            platform: channel.platform.clone(),
            chat_id: channel.chat_id.clone(),
            status: DeliveryStatus::Queued,
            chunks_sent: 1,
            error: None,
        })
    }

    async fn send_file(
        &self,
        platform: &str,
        recipient: &str,
        file_path: &str,
        caption: Option<&str>,
    ) -> Result<String, ToolError> {
        Ok(json!({
            "type": "file_delivery_request",
            "platform": platform,
            "recipient": recipient,
            "file_path": file_path,
            "caption": caption,
            "status": "pending",
        })
        .to_string())
    }
}

// ---------------------------------------------------------------------------
// GatewayMessagingBackend (real delivery via DeliveryRouter)
// ---------------------------------------------------------------------------

/// Callback-based messaging backend that delegates to a delivery function.
///
/// The gateway layer constructs this with a closure that calls
/// `DeliveryRouter::send_to_platform`. This avoids a direct dependency
/// from `hermes-tools` to `hermes-gateway`.
pub struct GatewayMessagingBackend {
    /// Async function: (platform, chat_id, message) -> Result
    send_fn: Box<
        dyn Fn(String, String, String) -> futures::future::BoxFuture<'static, Result<(), String>>
            + Send
            + Sync,
    >,
    /// Async function: (platform, chat_id, file_path, caption) -> Result
    send_file_fn: Option<
        Box<
            dyn Fn(
                    String,
                    String,
                    String,
                    Option<String>,
                ) -> futures::future::BoxFuture<'static, Result<(), String>>
                + Send
                + Sync,
        >,
    >,
    /// List of registered platform names.
    platforms: Vec<String>,
}

impl GatewayMessagingBackend {
    pub fn new<F>(send_fn: F) -> Self
    where
        F: Fn(String, String, String) -> futures::future::BoxFuture<'static, Result<(), String>>
            + Send
            + Sync
            + 'static,
    {
        Self {
            send_fn: Box::new(send_fn),
            send_file_fn: None,
            platforms: Vec::new(),
        }
    }

    pub fn with_file_support<F>(mut self, send_file_fn: F) -> Self
    where
        F: Fn(
                String,
                String,
                String,
                Option<String>,
            ) -> futures::future::BoxFuture<'static, Result<(), String>>
            + Send
            + Sync
            + 'static,
    {
        self.send_file_fn = Some(Box::new(send_file_fn));
        self
    }

    pub fn with_platforms(mut self, platforms: Vec<String>) -> Self {
        self.platforms = platforms;
        self
    }
}

#[async_trait]
impl MessagingBackend for GatewayMessagingBackend {
    async fn send(
        &self,
        platform: &str,
        recipient: &str,
        message: &str,
    ) -> Result<String, ToolError> {
        (self.send_fn)(
            platform.to_string(),
            recipient.to_string(),
            message.to_string(),
        )
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Delivery failed: {e}")))?;

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
        let send_file_fn = self.send_file_fn.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed(
                "File delivery not configured for this gateway backend".into(),
            )
        })?;

        send_file_fn(
            platform.to_string(),
            recipient.to_string(),
            file_path.to_string(),
            caption.map(|s| s.to_string()),
        )
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("File delivery failed: {e}")))?;

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
        self.platforms.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn signal_backend_returns_pending() {
        let backend = SignalMessagingBackend::new();
        let result = backend.send("telegram", "12345", "hello").await.unwrap();
        assert!(result.contains("pending"));
        assert!(result.contains("telegram"));
    }

    #[tokio::test]
    async fn signal_backend_file_returns_pending() {
        let backend = SignalMessagingBackend::new();
        let result = backend
            .send_file("telegram", "12345", "/tmp/file.png", Some("caption"))
            .await
            .unwrap();
        assert!(result.contains("pending"));
        assert!(result.contains("file.png"));
    }

    #[tokio::test]
    async fn gateway_backend_delivers() {
        let backend = GatewayMessagingBackend::new(|_platform, _chat_id, _message| {
            Box::pin(async { Ok(()) })
        });
        let result = backend.send("telegram", "12345", "hello").await.unwrap();
        assert!(result.contains("delivered"));
    }

    #[tokio::test]
    async fn gateway_backend_propagates_error() {
        let backend = GatewayMessagingBackend::new(|_platform, _chat_id, _message| {
            Box::pin(async { Err("connection refused".to_string()) })
        });
        let err = backend.send("telegram", "12345", "hello").await.unwrap_err();
        assert!(err.to_string().contains("connection refused"));
    }

    #[tokio::test]
    async fn gateway_backend_file_delivery() {
        let backend = GatewayMessagingBackend::new(|_, _, _| Box::pin(async { Ok(()) }))
            .with_file_support(|_, _, _, _| Box::pin(async { Ok(()) }));
        let result = backend
            .send_file("telegram", "12345", "/tmp/img.png", Some("photo"))
            .await
            .unwrap();
        assert!(result.contains("delivered"));
        assert!(result.contains("img.png"));
    }

    #[tokio::test]
    async fn gateway_backend_file_not_configured() {
        let backend = GatewayMessagingBackend::new(|_, _, _| Box::pin(async { Ok(()) }));
        let err = backend
            .send_file("telegram", "12345", "/tmp/img.png", None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not configured"));
    }
}

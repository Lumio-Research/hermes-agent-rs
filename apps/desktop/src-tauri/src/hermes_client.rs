//! Remote client for hermes-dashboard backend.
//! Supports both HTTP POST (fallback) and WebSocket streaming.

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message as WsMsg;

#[derive(Serialize)]
struct SendMessageRequest {
    text: String,
    user_id: Option<String>,
}

#[derive(Deserialize)]
struct SendMessageResponse {
    reply: String,
    #[allow(dead_code)]
    session_id: String,
    #[allow(dead_code)]
    message_count: usize,
}

#[derive(Deserialize)]
struct HealthResponse {
    status: String,
}

#[derive(Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub tool: Option<String>,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(300))
        .connect_timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub async fn health_check(api_base: &str) -> Result<bool, String> {
    let resp = client()
        .get(format!("{}/health", api_base))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<HealthResponse>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.status == "ok")
}

/// Send message via HTTP POST (non-streaming fallback).
pub async fn send_message(api_base: &str, session_id: &str, text: &str) -> Result<String, String> {
    let url = format!("{}/v1/sessions/{}/messages", api_base, session_id);
    let resp = client()
        .post(&url)
        .json(&SendMessageRequest {
            text: text.to_string(),
            user_id: Some("app".to_string()),
        })
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status, body));
    }

    resp.json::<SendMessageResponse>()
        .await
        .map(|r| r.reply)
        .map_err(|e| format!("Parse error: {}", e))
}

/// Send message via WebSocket streaming.
///
/// `on_event(event_type, content, tool_name)` is called for each streaming event.
/// Returns the full reply text.
pub async fn send_message_stream<F>(
    api_base: &str,
    session_id: &str,
    text: &str,
    on_event: F,
) -> Result<String, String>
where
    F: Fn(&str, &str, Option<&str>) + Send + 'static,
{
    let ws_base = api_base
        .replace("http://", "ws://")
        .replace("https://", "wss://");
    let url = format!("{}/v1/ws-stream/{}", ws_base, session_id);

    let (ws_stream, _) = tokio_tungstenite::connect_async(&url)
        .await
        .map_err(|e| format!("WebSocket connect failed: {}", e))?;

    let (mut write, mut read) = ws_stream.split();

    // Wait for "connected" event
    if let Some(Ok(WsMsg::Text(t))) = read.next().await {
        let t_str = t.to_string();
        if !t_str.contains("connected") {
            return Err(format!("Unexpected handshake: {}", t_str));
        }
    }

    // Send the message
    let req = serde_json::json!({"text": text, "user_id": "app"});
    write
        .send(WsMsg::Text(req.to_string().into()))
        .await
        .map_err(|e| format!("WebSocket send failed: {}", e))?;

    // Read streaming events
    let mut full_reply = String::new();
    while let Some(Ok(msg)) = read.next().await {
        match msg {
            WsMsg::Text(json_text) => {
                let json_str = json_text.to_string();
                if let Ok(event) = serde_json::from_str::<StreamEvent>(&json_str) {
                    on_event(
                        &event.event_type,
                        &event.content,
                        event.tool.as_deref(),
                    );

                    match event.event_type.as_str() {
                        "done" => {
                            full_reply = event.content;
                            break;
                        }
                        "error" => {
                            return Err(event.content);
                        }
                        "text" => {
                            full_reply.push_str(&event.content);
                        }
                        _ => {}
                    }
                }
            }
            WsMsg::Close(_) => break,
            _ => {}
        }
    }

    let _ = write.send(WsMsg::Close(None)).await;
    Ok(full_reply)
}

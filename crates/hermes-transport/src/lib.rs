use serde::{Deserialize, Serialize};

pub type SessionId = String;
pub type MessageId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolMessage {
    pub id: MessageId,
    pub role: Role,
    pub content: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSummary {
    pub id: SessionId,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub message_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListSessionsResponse {
    pub sessions: Vec<SessionSummary>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionMessagesResponse {
    pub session_id: SessionId,
    pub messages: Vec<ProtocolMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateSessionRequest {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenameSessionRequest {
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SendMessageRequest {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personality: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SendMessageResponse {
    pub session_id: SessionId,
    pub reply: String,
    pub message_count: usize,
    pub trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WsEnvelope {
    pub version: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub trace_id: String,
    pub event: StreamEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    Connected { session_id: SessionId },
    Text { content: String },
    Thinking { content: String },
    ToolStart { tool: String, content: String },
    ToolComplete { tool: String, content: String },
    Status { content: String },
    Activity { content: String },
    Error { code: String, message: String },
    Done { content: String },
}

pub fn ws_schema_json() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "HermesWsEnvelope",
        "type": "object",
        "required": ["version", "trace_id", "event"],
        "properties": {
            "version": { "type": "integer", "const": 1 },
            "request_id": { "type": "string" },
            "trace_id": { "type": "string" },
            "event": {
                "oneOf": [
                    { "type": "object", "required": ["type", "session_id"], "properties": { "type": { "const": "connected" }, "session_id": { "type": "string" } } },
                    { "type": "object", "required": ["type", "content"], "properties": { "type": { "const": "text" }, "content": { "type": "string" } } },
                    { "type": "object", "required": ["type", "content"], "properties": { "type": { "const": "thinking" }, "content": { "type": "string" } } },
                    { "type": "object", "required": ["type", "tool", "content"], "properties": { "type": { "const": "tool_start" }, "tool": { "type": "string" }, "content": { "type": "string" } } },
                    { "type": "object", "required": ["type", "tool", "content"], "properties": { "type": { "const": "tool_complete" }, "tool": { "type": "string" }, "content": { "type": "string" } } },
                    { "type": "object", "required": ["type", "content"], "properties": { "type": { "const": "status" }, "content": { "type": "string" } } },
                    { "type": "object", "required": ["type", "content"], "properties": { "type": { "const": "activity" }, "content": { "type": "string" } } },
                    { "type": "object", "required": ["type", "code", "message"], "properties": { "type": { "const": "error" }, "code": { "type": "string" }, "message": { "type": "string" } } },
                    { "type": "object", "required": ["type", "content"], "properties": { "type": { "const": "done" }, "content": { "type": "string" } } }
                ]
            }
        }
    })
}

pub fn new_trace_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_event_serialization_uses_tagged_union() {
        let event = StreamEvent::Text {
            content: "hello".into(),
        };
        let value = serde_json::to_value(event).expect("serialize stream event");
        assert_eq!(value["type"], "text");
        assert_eq!(value["content"], "hello");
    }

    #[test]
    fn envelope_contains_trace_and_version() {
        let envelope = WsEnvelope {
            version: 1,
            request_id: Some("req-1".into()),
            trace_id: "trace-1".into(),
            event: StreamEvent::Done {
                content: "ok".into(),
            },
        };
        let value = serde_json::to_value(envelope).expect("serialize envelope");
        assert_eq!(value["version"], 1);
        assert_eq!(value["request_id"], "req-1");
        assert_eq!(value["trace_id"], "trace-1");
        assert_eq!(value["event"]["type"], "done");
    }

    #[test]
    fn fixtures_round_trip_across_protocol_matrix() {
        let fixtures = include_str!("../../../sdk/protocol-fixtures/ws_envelopes.json");
        let parsed: Vec<WsEnvelope> =
            serde_json::from_str(fixtures).expect("parse protocol fixtures");
        assert_eq!(parsed.len(), 4);
        assert!(matches!(parsed[0].event, StreamEvent::Connected { .. }));
        assert!(matches!(parsed[1].event, StreamEvent::Text { .. }));
        assert!(matches!(parsed[2].event, StreamEvent::Error { .. }));
        assert!(matches!(parsed[3].event, StreamEvent::Done { .. }));
    }
}

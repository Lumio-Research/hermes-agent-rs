use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::errors::{AgentError, GatewayError, ToolError};
use crate::tool_schema::ToolSchema;
use crate::types::{CommandOutput, LlmResponse, Message, Skill, SkillMeta, StreamChunk};

// ---------------------------------------------------------------------------
// LlmProvider
// ---------------------------------------------------------------------------

/// Trait for LLM provider backends.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Perform a single chat completion request.
    async fn chat_completion(
        &self,
        messages: &[crate::types::Message],
        tools: &[ToolSchema],
        max_tokens: Option<u32>,
        temperature: Option<f64>,
        model: Option<&str>,
        extra_body: Option<&serde_json::Value>,
    ) -> Result<LlmResponse, AgentError>;

    /// Perform a streaming chat completion, returning a stream of chunks.
    fn chat_completion_stream(
        &self,
        messages: &[crate::types::Message],
        tools: &[ToolSchema],
        max_tokens: Option<u32>,
        temperature: Option<f64>,
        model: Option<&str>,
        extra_body: Option<&serde_json::Value>,
    ) -> BoxStream<'static, Result<StreamChunk, AgentError>>;
}

// ---------------------------------------------------------------------------
// ToolHandler
// ---------------------------------------------------------------------------

/// Trait for tool execution handlers.
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// Execute the tool with the given parameters.
    async fn execute(&self, params: Value) -> Result<String, ToolError>;

    /// Return the schema describing this tool's parameters.
    fn schema(&self) -> ToolSchema;
}

// ---------------------------------------------------------------------------
// PlatformAdapter
// ---------------------------------------------------------------------------

/// Parse mode for platform messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseMode {
    Plain,
    Markdown,
    Html,
}

/// Trait for platform communication adapters (Telegram, Discord, etc.).
#[async_trait]
pub trait PlatformAdapter: Send + Sync {
    /// Start the platform adapter (connect, begin listening).
    async fn start(&self) -> Result<(), GatewayError>;

    /// Stop the platform adapter gracefully.
    async fn stop(&self) -> Result<(), GatewayError>;

    /// Send a text message to a chat.
    async fn send_message(
        &self,
        chat_id: &str,
        text: &str,
        parse_mode: Option<ParseMode>,
    ) -> Result<(), GatewayError>;

    /// Edit an existing message.
    async fn edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        text: &str,
    ) -> Result<(), GatewayError>;

    /// Send a file to a chat with an optional caption.
    async fn send_file(
        &self,
        chat_id: &str,
        file_path: &str,
        caption: Option<&str>,
    ) -> Result<(), GatewayError>;

    /// Check whether the adapter is currently running.
    fn is_running(&self) -> bool;

    /// Return the name of this platform (e.g. "telegram", "discord").
    fn platform_name(&self) -> &str;

    /// Periodic maintenance: prune token caches, dedup maps, etc.
    ///
    /// Default is a no-op. Adapters with long-lived in-memory caches should
    /// override this so gateway cleanup watchers can reclaim memory.
    async fn maintenance_prune(&self) {}
}

// ---------------------------------------------------------------------------
// TerminalBackend
// ---------------------------------------------------------------------------

/// Trait for terminal / shell backend implementations.
#[async_trait]
pub trait TerminalBackend: Send + Sync {
    /// Execute a command in the terminal.
    async fn execute_command(
        &self,
        command: &str,
        timeout: Option<u64>,
        workdir: Option<&str>,
        background: bool,
        pty: bool,
    ) -> Result<CommandOutput, AgentError>;

    /// Read a file's contents (with optional offset and line limit).
    async fn read_file(
        &self,
        path: &str,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> Result<String, AgentError>;

    /// Write content to a file.
    async fn write_file(&self, path: &str, content: &str) -> Result<(), AgentError>;

    /// Check whether a file exists at the given path.
    async fn file_exists(&self, path: &str) -> Result<bool, AgentError>;
}

// ---------------------------------------------------------------------------
// MemoryProvider
// ---------------------------------------------------------------------------

/// Trait for key-value memory storage backends.
#[async_trait]
pub trait MemoryProvider: Send + Sync {
    /// Save a value under a namespace + key.
    async fn save(&self, namespace: &str, key: &str, value: &str) -> Result<(), AgentError>;

    /// Load a value from a namespace + key.
    async fn load(&self, namespace: &str, key: &str) -> Result<Option<String>, AgentError>;

    /// List all namespaces.
    async fn list_namespaces(&self) -> Result<Vec<String>, AgentError>;

    /// Delete a value from a namespace + key.
    async fn delete(&self, namespace: &str, key: &str) -> Result<(), AgentError>;
}

// ---------------------------------------------------------------------------
// SkillProvider
// ---------------------------------------------------------------------------

/// Trait for skill management backends.
#[async_trait]
pub trait SkillProvider: Send + Sync {
    /// Create a new skill.
    async fn create_skill(
        &self,
        name: &str,
        content: &str,
        category: Option<&str>,
    ) -> Result<Skill, AgentError>;

    /// Get a skill by name.
    async fn get_skill(&self, name: &str) -> Result<Option<Skill>, AgentError>;

    /// List all skills with metadata.
    async fn list_skills(&self) -> Result<Vec<SkillMeta>, AgentError>;

    /// Update an existing skill's content.
    async fn update_skill(&self, name: &str, content: &str) -> Result<Skill, AgentError>;

    /// Delete a skill by name.
    async fn delete_skill(&self, name: &str) -> Result<(), AgentError>;
}

// ---------------------------------------------------------------------------
// AgentOverrides / AgentReply
// ---------------------------------------------------------------------------

/// Optional overrides for a single agent request.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentOverrides {
    pub model: Option<String>,
    pub personality: Option<String>,
}

/// Reply from an agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentReply {
    /// The assistant's text response.
    pub text: String,
    /// Total message count in the session after this exchange.
    pub message_count: usize,
}

// ---------------------------------------------------------------------------
// AgentService
// ---------------------------------------------------------------------------

/// Abstraction for agent execution — callers are agnostic about
/// whether the agent runs in-process or remotely.
#[async_trait]
pub trait AgentService: Send + Sync {
    /// Send a message to a session and get the reply.
    async fn send_message(
        &self,
        session_id: &str,
        text: &str,
        overrides: AgentOverrides,
    ) -> Result<AgentReply, AgentError>;

    /// Send a message and stream back chunks followed by a final reply.
    async fn send_message_stream(
        &self,
        session_id: &str,
        text: &str,
        overrides: AgentOverrides,
        on_chunk: Arc<dyn Fn(StreamChunk) + Send + Sync>,
    ) -> Result<AgentReply, AgentError>;

    /// Get all messages for a session.
    async fn get_session_messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<Message>, AgentError>;

    /// Clear a session's message history.
    async fn reset_session(
        &self,
        session_id: &str,
    ) -> Result<(), AgentError>;
}

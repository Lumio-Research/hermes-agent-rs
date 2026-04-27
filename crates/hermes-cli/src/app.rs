//! Application state management for the interactive CLI.
//!
//! The `App` struct owns the configuration, agent loop, tool registry,
//! and conversation message history. It coordinates input handling,
//! slash commands, and session management.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use hermes_agent::sub_agent_orchestrator::SubAgentOrchestrator;
use hermes_agent::{AgentLoop, InterruptController};
use hermes_config::{hermes_home as hermes_home_dir, load_config, GatewayConfig};
use hermes_core::AgentError;
use hermes_environments::LocalBackend;
use hermes_skills::{FileSkillStore, SkillManager};
use hermes_tools::ToolRegistry;

use crate::cli::Cli;
use crate::tui::StreamHandle;

// Re-export shared agent builder functions so that `hermes-cli/src/main.rs`
// (and any other consumer of `hermes_cli::app`) can continue importing them
// from this module without changes.
pub use hermes_agent::agent_builder::{
    bridge_tool_registry, build_agent_config, build_provider, provider_api_key_from_env,
};

/// `AgentLoop` returns full context including an injected **leading** system prompt.
/// For interactive CLI/TUI we keep a user-visible **transcript** in [`App::messages`];
/// the next [`AgentLoop::run`] rebuilds system from config/skills anyway, so dropping
/// leading `System` rows avoids duplicate system blocks and matches Python transcript UX.
fn strip_leading_system_messages(mut msgs: Vec<hermes_core::Message>) -> Vec<hermes_core::Message> {
    let first_non_system = msgs
        .iter()
        .position(|m| m.role != hermes_core::MessageRole::System)
        .unwrap_or(msgs.len());
    if first_non_system > 0 {
        msgs.drain(0..first_non_system);
    }
    msgs
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

/// Top-level application state for an interactive Hermes session.
pub struct App {
    /// Loaded gateway configuration.
    pub config: Arc<GatewayConfig>,

    /// The agent loop engine.
    pub agent: Arc<AgentLoop>,

    /// The tool registry (shared with the agent).
    pub tool_registry: Arc<ToolRegistry>,

    /// Conversation messages for the current session.
    pub messages: Vec<hermes_core::Message>,

    /// Unique identifier for the current session.
    pub session_id: String,

    /// Whether the application loop is still running.
    pub running: bool,

    /// Currently active model identifier (e.g. "openai:gpt-4o").
    pub current_model: String,

    /// Currently active personality name.
    pub current_personality: Option<String>,

    /// History of user inputs for recall.
    pub input_history: Vec<String>,

    /// Index into input_history for up/down arrow navigation.
    pub history_index: usize,

    /// Interrupt controller for stopping agent execution.
    pub interrupt_controller: InterruptController,

    /// Optional TUI streaming sink for incremental chunks.
    pub stream_handle: Option<StreamHandle>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("session_id", &self.session_id)
            .field("running", &self.running)
            .field("current_model", &self.current_model)
            .field("current_personality", &self.current_personality)
            .field("history_index", &self.history_index)
            .finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// SessionInfo (for serialization)
// ---------------------------------------------------------------------------

/// Serializable snapshot of a session (for save/restore).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub model: String,
    pub personality: Option<String>,
    pub message_count: usize,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// App implementation
// ---------------------------------------------------------------------------

impl App {
    /// Create a new `App` from the parsed CLI arguments.
    ///
    /// This loads (or creates) the gateway configuration, builds a tool
    /// registry with the configured tools, constructs an LLM provider,
    /// and initializes the agent loop.
    pub async fn new(cli: Cli) -> Result<Self, AgentError> {
        let config = load_config(cli.config_dir.as_deref())
            .map_err(|e| AgentError::Config(e.to_string()))?;

        let mut config = config;
        if let Some(ref model) = cli.model {
            config.model = Some(model.clone());
        }
        if let Some(ref personality) = cli.personality {
            config.personality = Some(personality.clone());
        }

        let current_model = config.model.clone().unwrap_or_else(|| "gpt-4o".to_string());
        let current_personality = config.personality.clone();

        let tool_registry = Arc::new(ToolRegistry::new());
        let terminal_backend: Arc<dyn hermes_core::TerminalBackend> =
            Arc::new(LocalBackend::default());
        let skill_store = Arc::new(FileSkillStore::new(FileSkillStore::default_dir()));
        let skill_provider: Arc<dyn hermes_core::SkillProvider> =
            Arc::new(SkillManager::new(skill_store));
        hermes_tools::register_builtin_tools(&tool_registry, terminal_backend, skill_provider);
        let live_count =
            crate::live_messaging::enable_live_messaging_tool(&config, &tool_registry).await;
        if live_count > 0 {
            tracing::info!(
                adapters = live_count,
                "Enabled live send_message delivery backend for CLI session"
            );
        }
        let agent_tool_registry = Arc::new(bridge_tool_registry(&tool_registry));

        let agent_config = build_agent_config(&config, &current_model, Some("cli"));
        let provider = build_provider(&config, &current_model);

        let agent_inner = AgentLoop::new(agent_config, agent_tool_registry, provider);
        let hermes_home = hermes_home_dir();
        let orchestrator = Arc::new(SubAgentOrchestrator::from_parent(&agent_inner, hermes_home));
        let agent = Arc::new(agent_inner.with_sub_agent_orchestrator(orchestrator));

        Ok(Self {
            config: Arc::new(config),
            agent,
            tool_registry,
            messages: Vec::new(),
            session_id: Uuid::new_v4().to_string(),
            running: true,
            current_model,
            current_personality,
            input_history: Vec::new(),
            history_index: 0,
            interrupt_controller: InterruptController::new(),
            stream_handle: None,
        })
    }

    /// Attach a streaming handle (used by TUI mode).
    pub fn set_stream_handle(&mut self, handle: Option<StreamHandle>) {
        self.stream_handle = handle;
    }

    /// Run the interactive REPL loop.
    ///
    /// This is the main entry point for interactive mode. It delegates
    /// to the TUI subsystem for rendering and event handling.
    pub async fn run_interactive(&mut self) -> Result<(), AgentError> {
        // The actual TUI loop is in crate::tui::run()
        // This method exists so non-TUI callers can drive the loop manually.
        if self.running {
            loop {
                if !self.running {
                    break;
                }
                // In a real implementation, the TUI event loop would drive this.
                // Here we just mark that we're ready.
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
        Ok(())
    }

    /// Handle a line of user input.
    ///
    /// If the input starts with `/` it is treated as a slash command.
    /// Otherwise it is sent as a user message to the agent.
    pub async fn handle_input(&mut self, input: &str) -> Result<(), AgentError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        // Store in input history
        self.input_history.push(trimmed.to_string());
        self.history_index = self.input_history.len();

        if trimmed.starts_with('/') {
            // Parse the slash command and its arguments
            let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
            let cmd = parts[0];
            let args: Vec<&str> = parts
                .get(1)
                .map(|s| s.split_whitespace().collect())
                .unwrap_or_default();

            let result = crate::commands::handle_slash_command(self, cmd, &args).await?;
            if result == crate::commands::CommandResult::Quit {
                self.running = false;
            }
        } else {
            // Regular user message
            self.messages.push(hermes_core::Message::user(trimmed));
            self.run_agent().await?;
        }

        Ok(())
    }

    /// Handle a slash command string (without the leading `/`).
    pub async fn handle_command(&mut self, cmd: &str) -> Result<(), AgentError> {
        let trimmed = cmd.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
        let slash_cmd = if parts[0].starts_with('/') {
            parts[0]
        } else {
            // Prepend / if not present
            return self.handle_input(&format!("/{}", trimmed)).await;
        };

        let args: Vec<&str> = parts
            .get(1)
            .map(|s| s.split_whitespace().collect())
            .unwrap_or_default();

        let result = crate::commands::handle_slash_command(self, slash_cmd, &args).await?;
        if result == crate::commands::CommandResult::Quit {
            self.running = false;
        }
        Ok(())
    }

    /// Create a new session, clearing all messages.
    pub fn new_session(&mut self) {
        self.session_id = Uuid::new_v4().to_string();
        self.messages.clear();
        self.input_history.clear();
        self.history_index = 0;
        self.agent.clear_pending_steer();
    }

    /// Reset the current session (clear messages but keep session ID).
    pub fn reset_session(&mut self) {
        self.messages.clear();
        self.input_history.clear();
        self.history_index = 0;
        self.agent.clear_pending_steer();
    }

    /// Retry the last user message by re-sending it to the agent.
    ///
    /// Finds the last user message in history, removes all messages after it
    /// (including the assistant response), and re-runs the agent.
    pub async fn retry_last(&mut self) -> Result<(), AgentError> {
        // Find the last user message
        let last_user_idx = self
            .messages
            .iter()
            .rposition(|m| m.role == hermes_core::MessageRole::User);

        if let Some(idx) = last_user_idx {
            let last_user_msg = self.messages[idx].clone();
            // Truncate messages to just before the last user message
            self.messages.truncate(idx);
            // Re-add the user message
            self.messages.push(last_user_msg);
            // Re-run the agent
            self.run_agent().await?;
        }

        Ok(())
    }

    /// Undo the last exchange (remove the last user message and its response).
    pub fn undo_last(&mut self) {
        // Find the last user message
        if let Some(idx) = self
            .messages
            .iter()
            .rposition(|m| m.role == hermes_core::MessageRole::User)
        {
            // Remove everything from the last user message onward
            self.messages.truncate(idx);
        }
    }

    /// Switch the active model, rebuilding the provider and agent loop.
    pub fn switch_model(&mut self, provider_model: &str) {
        self.current_model = provider_model.to_string();

        let provider = build_provider(&self.config, &self.current_model);
        let agent_config = build_agent_config(&self.config, &self.current_model, Some("cli"));
        let agent_tool_registry = Arc::new(bridge_tool_registry(&self.tool_registry));

        let agent_inner = AgentLoop::new(agent_config, agent_tool_registry, provider);
        let hermes_home = hermes_home_dir();
        let orchestrator = Arc::new(SubAgentOrchestrator::from_parent(&agent_inner, hermes_home));
        self.agent = Arc::new(agent_inner.with_sub_agent_orchestrator(orchestrator));

        tracing::info!("Switched model to: {}", provider_model);
    }

    /// Switch the active personality.
    pub fn switch_personality(&mut self, name: &str) {
        self.current_personality = Some(name.to_string());
        tracing::info!("Switched personality to: {}", name);
    }

    /// Run the agent on the current message history.
    ///
    /// Sends all messages to the agent loop and appends the result.
    /// Checks the interrupt controller before running and clears it after.
    async fn run_agent(&mut self) -> Result<(), AgentError> {
        self.interrupt_controller.clear_interrupt();

        let messages = self.messages.clone();
        let result = if self.config.streaming.enabled {
            let stream_handle = self.stream_handle.clone();
            let stream_cb: Option<Box<dyn Fn(hermes_core::StreamChunk) + Send + Sync>> =
                stream_handle.map(|h| {
                    Box::new(move |chunk: hermes_core::StreamChunk| {
                        h.send_chunk(chunk);
                    }) as Box<dyn Fn(hermes_core::StreamChunk) + Send + Sync>
                });
            self.agent.run_stream(messages, None, stream_cb).await
        } else {
            self.agent.run(messages, None).await
        };

        match result {
            Ok(result) => {
                self.messages = strip_leading_system_messages(result.messages);
                if let Some(handle) = &self.stream_handle {
                    handle.send_done();
                }
                if result.interrupted {
                    tracing::info!("Agent loop returned interrupted=true (graceful stop)");
                    println!("[Agent execution interrupted]");
                } else if !result.finished_naturally {
                    tracing::warn!(
                        "Agent stopped after {} turns (did not finish naturally)",
                        result.total_turns
                    );
                }
            }
            Err(AgentError::Interrupted { message }) => {
                self.interrupt_controller.clear_interrupt();
                self.agent.clear_pending_steer();
                if let Some(handle) = &self.stream_handle {
                    handle.send_done();
                }
                if let Some(redirect) = message {
                    tracing::info!("Agent interrupted with redirect: {}", redirect);
                } else {
                    tracing::info!("Agent interrupted by user");
                }
                println!("[Agent execution interrupted]");
            }
            Err(e) => {
                if let Some(handle) = &self.stream_handle {
                    handle.send_done();
                }
                return Err(e);
            }
        }

        Ok(())
    }

    /// Get a serializable snapshot of the current session info.
    pub fn session_info(&self) -> SessionInfo {
        SessionInfo {
            session_id: self.session_id.clone(),
            model: self.current_model.clone(),
            personality: self.current_personality.clone(),
            message_count: self.messages.len(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Navigate backward in input history.
    pub fn history_prev(&mut self) -> Option<&str> {
        if self.history_index > 0 {
            self.history_index -= 1;
            self.input_history
                .get(self.history_index)
                .map(|s| s.as_str())
        } else {
            None
        }
    }

    /// Navigate forward in input history.
    pub fn history_next(&mut self) -> Option<&str> {
        if self.history_index < self.input_history.len() {
            self.history_index += 1;
            if self.history_index < self.input_history.len() {
                self.input_history
                    .get(self.history_index)
                    .map(|s| s.as_str())
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_leading_system_messages_removes_prefix_system() {
        let msgs = vec![
            hermes_core::Message::system("injected prompt"),
            hermes_core::Message::user("hello"),
        ];
        let out = strip_leading_system_messages(msgs);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].role, hermes_core::MessageRole::User);
    }

    #[test]
    fn test_session_info_serialization() {
        let info = SessionInfo {
            session_id: "test-123".to_string(),
            model: "gpt-4o".to_string(),
            personality: Some("helpful".to_string()),
            message_count: 5,
            created_at: "2025-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_id, "test-123");
        assert_eq!(back.model, "gpt-4o");
    }
}

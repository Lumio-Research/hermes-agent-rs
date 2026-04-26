//! Local in-process implementation of the `AgentService` trait.

use std::sync::Arc;

use async_trait::async_trait;

use hermes_config::GatewayConfig;
use hermes_core::traits::{AgentOverrides, AgentReply, AgentService};
use hermes_core::{AgentError, LlmProvider, Message, StreamChunk};
use hermes_tools::ToolRegistry;

use crate::agent_builder::{build_agent_config, build_provider, bridge_tool_registry};
use crate::agent_loop::AgentLoop;
use crate::session_persistence::SessionPersistence;

/// Local in-process agent service.
///
/// This service runs the agent loop directly in the current process,
/// using SQLite for session persistence.
pub struct LocalAgentService {
    /// Gateway configuration.
    config: Arc<GatewayConfig>,
    /// Tool registry with all registered tools.
    tool_registry: Arc<ToolRegistry>,
    /// Session persistence manager.
    session_persistence: Arc<SessionPersistence>,
    /// Optional provider factory override, mainly for tests.
    provider_factory: Option<ProviderFactory>,
}

pub type ProviderFactory = Arc<dyn Fn(&GatewayConfig, &str) -> Arc<dyn LlmProvider> + Send + Sync>;

impl LocalAgentService {
    /// Create a new `LocalAgentService`.
    pub fn new(
        config: Arc<GatewayConfig>,
        tool_registry: Arc<ToolRegistry>,
        session_persistence: Arc<SessionPersistence>,
    ) -> Self {
        Self {
            config,
            tool_registry,
            session_persistence,
            provider_factory: None,
        }
    }

    /// Test-friendly constructor that allows overriding provider creation.
    pub fn new_with_provider_factory(
        config: Arc<GatewayConfig>,
        tool_registry: Arc<ToolRegistry>,
        session_persistence: Arc<SessionPersistence>,
        provider_factory: ProviderFactory,
    ) -> Self {
        Self {
            config,
            tool_registry,
            session_persistence,
            provider_factory: Some(provider_factory),
        }
    }

    fn resolve_provider(&self, effective_model: &str) -> Arc<dyn LlmProvider> {
        if let Some(factory) = &self.provider_factory {
            return factory(&self.config, effective_model);
        }
        build_provider(&self.config, effective_model)
    }
}

#[async_trait]
impl AgentService for LocalAgentService {
    async fn send_message(
        &self,
        session_id: &str,
        text: &str,
        overrides: AgentOverrides,
    ) -> Result<AgentReply, AgentError> {
        // Load existing session messages
        let mut messages = self
            .session_persistence
            .load_session(session_id)
            .unwrap_or_default();

        // Append the new user message
        messages.push(Message::user(text));

        // Determine effective model and personality
        let effective_model = overrides
            .model
            .clone()
            .or_else(|| self.config.model.clone())
            .unwrap_or_else(|| "gpt-4o".to_string());

        let effective_personality = overrides
            .personality
            .clone()
            .or_else(|| self.config.personality.clone());

        // Build agent configuration
        let mut agent_config = build_agent_config(&self.config, &effective_model, Some("local"));
        if let Some(personality) = effective_personality {
            agent_config.personality = Some(personality);
        }

        // Build provider
        let provider = self.resolve_provider(&effective_model);

        // Bridge tool registry
        let agent_tool_registry = Arc::new(bridge_tool_registry(&self.tool_registry));

        // Create and run agent
        let agent = AgentLoop::new(agent_config, agent_tool_registry, provider);
        let result = agent.run(messages.clone(), None).await?;

        // Update messages with agent response
        messages = result.messages;

        // Persist updated session
        let _ = self
            .session_persistence
            .persist_session(
                session_id,
                &messages,
                Some(&effective_model),
                Some("local"),
                None,
                None,
            );

        // Extract the last assistant reply
        let reply_text = messages
            .iter()
            .rev()
            .find(|m| m.role == hermes_core::MessageRole::Assistant)
            .and_then(|m| m.content.clone())
            .unwrap_or_default();

        Ok(AgentReply {
            text: reply_text,
            message_count: messages.len(),
        })
    }

    async fn send_message_stream(
        &self,
        session_id: &str,
        text: &str,
        overrides: AgentOverrides,
        on_chunk: Arc<dyn Fn(StreamChunk) + Send + Sync>,
    ) -> Result<AgentReply, AgentError> {
        // Load existing session messages
        let mut messages = self
            .session_persistence
            .load_session(session_id)
            .unwrap_or_default();

        // Append the new user message
        messages.push(Message::user(text));

        // Determine effective model and personality
        let effective_model = overrides
            .model
            .clone()
            .or_else(|| self.config.model.clone())
            .unwrap_or_else(|| "gpt-4o".to_string());

        let effective_personality = overrides
            .personality
            .clone()
            .or_else(|| self.config.personality.clone());

        // Build agent configuration
        let mut agent_config = build_agent_config(&self.config, &effective_model, Some("local"));
        if let Some(personality) = effective_personality {
            agent_config.personality = Some(personality);
        }

        // Build provider
        let provider = self.resolve_provider(&effective_model);

        // Bridge tool registry
        let agent_tool_registry = Arc::new(bridge_tool_registry(&self.tool_registry));

        // Convert Arc to Box for AgentLoop
        let boxed_on_chunk: Box<dyn Fn(StreamChunk) + Send + Sync> = Box::new(move |chunk| {
            on_chunk(chunk);
        });

        // Create and run agent with streaming
        let agent = AgentLoop::new(agent_config, agent_tool_registry, provider);
        let result = agent
            .run_stream(messages.clone(), None, Some(boxed_on_chunk))
            .await?;

        // Update messages with agent response
        messages = result.messages;

        // Persist updated session
        let _ = self
            .session_persistence
            .persist_session(
                session_id,
                &messages,
                Some(&effective_model),
                Some("local"),
                None,
                None,
            );

        // Extract the last assistant reply
        let reply_text = messages
            .iter()
            .rev()
            .find(|m| m.role == hermes_core::MessageRole::Assistant)
            .and_then(|m| m.content.clone())
            .unwrap_or_default();

        Ok(AgentReply {
            text: reply_text,
            message_count: messages.len(),
        })
    }

    async fn get_session_messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<Message>, AgentError> {
        self.session_persistence
            .load_session(session_id)
            .map_err(|e| AgentError::Io(e.to_string()))
    }

    async fn reset_session(
        &self,
        session_id: &str,
    ) -> Result<(), AgentError> {
        // First check if session exists
        let messages = self.session_persistence.load_session(session_id);
        if messages.is_ok() {
            // Delete session by persisting an empty session
            let _ = self.session_persistence.persist_session(
                session_id,
                &[],
                None,
                None,
                None,
                None,
            );
        }
        Ok(())
    }
}
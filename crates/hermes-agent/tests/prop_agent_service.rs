use std::sync::Arc;

use futures::stream::BoxStream;
use hermes_agent::local_agent_service::{LocalAgentService, ProviderFactory};
use hermes_agent::session_persistence::SessionPersistence;
use hermes_core::traits::{AgentOverrides, AgentService};
use hermes_core::{AgentError, LlmProvider, LlmResponse, Message, StreamChunk, ToolSchema};
use hermes_tools::ToolRegistry;
use proptest::prelude::*;
use tempfile::tempdir;

struct EchoProvider;

#[async_trait::async_trait]
impl LlmProvider for EchoProvider {
    async fn chat_completion(
        &self,
        _messages: &[Message],
        _tools: &[ToolSchema],
        _max_tokens: Option<u32>,
        _temperature: Option<f64>,
        _model: Option<&str>,
        _extra_body: Option<&serde_json::Value>,
    ) -> Result<LlmResponse, AgentError> {
        Ok(LlmResponse {
            message: Message::assistant("mock-assistant-reply"),
            usage: None,
            model: "mock:model".to_string(),
            finish_reason: Some("stop".to_string()),
        })
    }

    fn chat_completion_stream(
        &self,
        _messages: &[Message],
        _tools: &[ToolSchema],
        _max_tokens: Option<u32>,
        _temperature: Option<f64>,
        _model: Option<&str>,
        _extra_body: Option<&serde_json::Value>,
    ) -> BoxStream<'static, Result<StreamChunk, AgentError>> {
        Box::pin(futures::stream::empty())
    }
}

fn make_service() -> LocalAgentService {
    let tmp = tempdir().expect("tempdir");
    let home = tmp.keep();
    let session = Arc::new(SessionPersistence::new(&home));
    let _ = session.ensure_db();

    let mut cfg = hermes_config::GatewayConfig::default();
    cfg.home_dir = Some(home.to_string_lossy().to_string());
    cfg.model = Some("mock:model".to_string());
    let factory: ProviderFactory = Arc::new(|_, _| Arc::new(EchoProvider));
    LocalAgentService::new_with_provider_factory(
        Arc::new(cfg),
        Arc::new(ToolRegistry::new()),
        session,
        factory,
    )
}

proptest! {
    // Feature: unified-runtime-architecture, Property 3: send_message appends user message and assistant reply
    #[test]
    fn send_message_appends_user_and_assistant(
        sid in "[a-z0-9]{1,20}",
        text in ".{1,40}",
    ) {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let (reply, messages) = rt.block_on(async {
            let service = make_service();
            let reply = service
                .send_message(&sid, &text, AgentOverrides::default())
                .await
                .expect("send_message");

            let messages = service
                .get_session_messages(&sid)
                .await
                .expect("session messages");
            (reply, messages)
        });
        prop_assert_eq!(reply.text, "mock-assistant-reply");
        prop_assert!(messages.len() >= 2);
        let penultimate = &messages[messages.len() - 2];
        let last = &messages[messages.len() - 1];
        prop_assert_eq!(penultimate.role, hermes_core::MessageRole::User);
        prop_assert_eq!(penultimate.content.as_deref(), Some(text.as_str()));
        prop_assert_eq!(last.role, hermes_core::MessageRole::Assistant);
        prop_assert_eq!(last.content.as_deref(), Some("mock-assistant-reply"));
    }
}

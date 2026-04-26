use std::sync::Arc;

use hermes_agent::local_agent_service::LocalAgentService;
use hermes_agent::session_persistence::SessionPersistence;
use hermes_core::traits::{AgentOverrides, AgentService};
use hermes_core::{AgentError, Message};
use hermes_tools::ToolRegistry;
use tempfile::tempdir;

fn make_service() -> (Arc<SessionPersistence>, LocalAgentService) {
    let tmp = tempdir().expect("tempdir");
    let home = tmp.keep();
    for key in [
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "OPENROUTER_API_KEY",
        "DASHSCOPE_API_KEY",
        "MOONSHOT_API_KEY",
        "MINIMAX_API_KEY",
        "NOUS_API_KEY",
        "GITHUB_COPILOT_TOKEN",
    ] {
        unsafe {
            std::env::remove_var(key);
        }
    }
    let session = Arc::new(SessionPersistence::new(&home));
    let _ = session.ensure_db();

    let mut cfg = hermes_config::GatewayConfig::default();
    cfg.home_dir = Some(home.to_string_lossy().to_string());
    cfg.model = Some("openai:gpt-4o".to_string());

    let service = LocalAgentService::new(
        Arc::new(cfg),
        Arc::new(ToolRegistry::new()),
        session.clone(),
    );
    (session, service)
}

#[tokio::test]
async fn send_message_returns_stub_provider_error_without_api_key() {
    let (_session, service) = make_service();
    let err = service
        .send_message("s1", "hello", AgentOverrides::default())
        .await
        .expect_err("stub provider should fail without API key");
    match err {
        AgentError::LlmApi(msg) => assert!(msg.contains("StubProvider")),
        other => panic!("expected AgentError::LlmApi, got {:?}", other),
    }
}

#[tokio::test]
async fn send_message_stream_returns_stub_provider_error_without_api_key() {
    let (_session, service) = make_service();
    let seen = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let seen_cb = seen.clone();
    let on_chunk = Arc::new(move |chunk: hermes_core::StreamChunk| {
        if let Some(delta) = chunk.delta.and_then(|d| d.content) {
            seen_cb.lock().expect("lock").push(delta);
        }
    });
    let err = service
        .send_message_stream("s1", "hello", AgentOverrides::default(), on_chunk)
        .await
        .expect_err("stub provider should fail without API key");
    match err {
        AgentError::LlmApi(msg) => assert!(msg.contains("StubProvider")),
        other => panic!("expected AgentError::LlmApi, got {:?}", other),
    }
    assert!(seen.lock().expect("lock").is_empty());
}

#[tokio::test]
async fn get_session_messages_and_reset_session_round_trip() {
    let (session, service) = make_service();
    let messages = vec![Message::user("u"), Message::assistant("a")];
    session
        .persist_session("s2", &messages, Some("openai:gpt-4o"), Some("test"), None, None)
        .expect("persist");

    let loaded = service
        .get_session_messages("s2")
        .await
        .expect("load from service");
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].content.as_deref(), Some("u"));
    assert_eq!(loaded[1].content.as_deref(), Some("a"));

    service.reset_session("s2").await.expect("reset");
    let after = service
        .get_session_messages("s2")
        .await
        .expect("load after reset");
    assert!(after.is_empty(), "reset should clear persisted session");
}

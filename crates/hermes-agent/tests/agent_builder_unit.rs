use std::sync::Arc;

use hermes_agent::agent_builder::{
    bridge_tool_registry, build_agent_config, provider_api_key_from_env,
};
use hermes_config::GatewayConfig;
use hermes_environments::LocalBackend;
use hermes_tools::ToolRegistry;

#[test]
fn build_agent_config_copies_core_fields() {
    let mut cfg = GatewayConfig::default();
    cfg.max_turns = 42;
    cfg.system_prompt = Some("sys".to_string());
    cfg.personality = Some("coder".to_string());
    cfg.model = Some("openai:gpt-4o-mini".to_string());
    cfg.smart_model_routing.enabled = true;
    cfg.smart_model_routing.max_simple_chars = 123;
    cfg.smart_model_routing.max_simple_words = 17;
    cfg.agent.memory_nudge_interval = 3;
    cfg.agent.skill_creation_nudge_interval = 4;
    cfg.agent.background_review_enabled = false;

    let agent_cfg = build_agent_config(&cfg, "openai:gpt-4o-mini", Some("cli"));
    assert_eq!(agent_cfg.max_turns, 42);
    assert_eq!(agent_cfg.system_prompt.as_deref(), Some("sys"));
    assert_eq!(agent_cfg.personality.as_deref(), Some("coder"));
    assert_eq!(agent_cfg.platform.as_deref(), Some("cli"));
    assert!(agent_cfg.smart_model_routing.enabled);
    assert_eq!(agent_cfg.memory_nudge_interval, 3);
    assert_eq!(agent_cfg.skill_creation_nudge_interval, 4);
    assert!(!agent_cfg.background_review_enabled);
}

#[test]
fn provider_api_key_from_env_maps_known_provider() {
    // Non-destructive check: verify mapping exists by setting env at runtime.
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "test-openai-key");
    }
    let key = provider_api_key_from_env("openai");
    assert_eq!(key.as_deref(), Some("test-openai-key"));
}

#[test]
fn bridge_tool_registry_keeps_schema_count() {
    let tools = ToolRegistry::new();
    let terminal_backend: Arc<dyn hermes_core::TerminalBackend> =
        Arc::new(LocalBackend::default());
    let skill_store = Arc::new(hermes_skills::FileSkillStore::new(
        hermes_skills::FileSkillStore::default_dir(),
    ));
    let skill_provider: Arc<dyn hermes_core::SkillProvider> =
        Arc::new(hermes_skills::SkillManager::new(skill_store));
    hermes_tools::register_builtin_tools(&tools, terminal_backend, skill_provider);

    let defs = tools.get_definitions();
    let bridged = bridge_tool_registry(&tools);
    let bridged_names = bridged.names();
    assert!(!defs.is_empty(), "builtin tool definitions should be present");
    assert_eq!(
        bridged_names.len(),
        defs.len(),
        "bridged registry count should match source definitions"
    );
}

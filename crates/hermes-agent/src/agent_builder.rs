//! Shared agent construction functions.
//!
//! These helpers build an [`AgentConfig`], LLM provider, and tool-registry
//! bridge from a [`GatewayConfig`] + model string.  Both the CLI (`app.rs`)
//! and the Dashboard (`lib.rs`) import from here so the logic lives in one
//! place.

use std::sync::Arc;

use futures::StreamExt;
use serde_json::Value;

use hermes_config::GatewayConfig;
use hermes_core::{AgentError, LlmProvider};
use hermes_tools::ToolRegistry;

use crate::agent_loop::{
    CheapModelRouteConfig, RuntimeProviderConfig, SmartModelRoutingConfig,
    ToolRegistry as AgentToolRegistry,
};
use crate::provider::{AnthropicProvider, GenericProvider, OpenAiProvider, OpenRouterProvider};
use crate::providers_extra::{
    CopilotProvider, KimiProvider, MiniMaxProvider, NousProvider, QwenProvider,
};
use crate::AgentConfig;

// ---------------------------------------------------------------------------
// parse_model_string
// ---------------------------------------------------------------------------

/// Split a `"provider:model"` string into `(provider, model)`.
///
/// If no colon is present the provider defaults to `"openai"`.
pub fn parse_model_string(model: &str) -> (&str, &str) {
    model.split_once(':').unwrap_or(("openai", model))
}

// ---------------------------------------------------------------------------
// build_agent_config
// ---------------------------------------------------------------------------

/// Build an [`AgentConfig`] from a [`GatewayConfig`] and a model string.
///
/// `platform` lets callers tag the config with their origin (`"cli"`,
/// `"http"`, `"gateway"`, …).  When `None` the field is left unset.
pub fn build_agent_config(
    config: &GatewayConfig,
    model: &str,
    platform: Option<&str>,
) -> AgentConfig {
    let provider_from_model = model.split_once(':').map(|(p, _)| p.to_string());
    AgentConfig {
        max_turns: config.max_turns,
        budget: config.budget.clone(),
        model: model.to_string(),
        system_prompt: config.system_prompt.clone(),
        personality: config.personality.clone(),
        hermes_home: config.home_dir.clone(),
        provider: provider_from_model,
        stream: config.streaming.enabled,
        platform: platform.map(|s| s.to_string()),
        pass_session_id: true,
        runtime_providers: config
            .llm_providers
            .iter()
            .map(|(name, cfg)| {
                (
                    name.clone(),
                    RuntimeProviderConfig {
                        api_key: cfg.api_key.clone(),
                        base_url: cfg.base_url.clone(),
                        command: cfg.command.clone(),
                        args: cfg.args.clone(),
                        oauth_token_url: cfg.oauth_token_url.clone(),
                        oauth_client_id: cfg.oauth_client_id.clone(),
                    },
                )
            })
            .collect(),
        smart_model_routing: SmartModelRoutingConfig {
            enabled: config.smart_model_routing.enabled,
            max_simple_chars: config.smart_model_routing.max_simple_chars,
            max_simple_words: config.smart_model_routing.max_simple_words,
            cheap_model: config.smart_model_routing.cheap_model.as_ref().map(|m| {
                CheapModelRouteConfig {
                    provider: m.provider.clone(),
                    model: m.model.clone(),
                    base_url: m.base_url.clone(),
                    api_key_env: m.api_key_env.clone(),
                }
            }),
        },
        memory_nudge_interval: config.agent.memory_nudge_interval,
        skill_creation_nudge_interval: config.agent.skill_creation_nudge_interval,
        background_review_enabled: config.agent.background_review_enabled,
        ..AgentConfig::default()
    }
}

// ---------------------------------------------------------------------------
// bridge_tool_registry
// ---------------------------------------------------------------------------

/// Bridge a full [`hermes_tools::ToolRegistry`] into the minimal
/// [`AgentToolRegistry`](crate::agent_loop::ToolRegistry) consumed by
/// [`AgentLoop`](crate::AgentLoop).
pub fn bridge_tool_registry(tools: &ToolRegistry) -> AgentToolRegistry {
    let mut agent_registry = AgentToolRegistry::new();
    for schema in tools.get_definitions() {
        let name = schema.name.clone();
        let tools_clone = tools.clone();
        agent_registry.register(
            name.clone(),
            schema,
            Arc::new(
                move |params: Value| -> Result<String, hermes_core::ToolError> {
                    Ok(tools_clone.dispatch(&name, params))
                },
            ),
        );
    }
    agent_registry
}

// ---------------------------------------------------------------------------
// provider_api_key_from_env
// ---------------------------------------------------------------------------

/// Resolve an API key / token for a named LLM provider from well-known
/// environment variables.
pub fn provider_api_key_from_env(provider: &str) -> Option<String> {
    let var = match provider {
        "openai" => "OPENAI_API_KEY",
        "anthropic" => "ANTHROPIC_API_KEY",
        "openrouter" => "OPENROUTER_API_KEY",
        "qwen" => "DASHSCOPE_API_KEY",
        "kimi" | "moonshot" => "MOONSHOT_API_KEY",
        "minimax" => "MINIMAX_API_KEY",
        "nous" => "NOUS_API_KEY",
        "copilot" => "GITHUB_COPILOT_TOKEN",
        _ => return None,
    };
    std::env::var(var).ok().filter(|s| !s.is_empty())
}

// ---------------------------------------------------------------------------
// build_provider
// ---------------------------------------------------------------------------

/// Build an `Arc<dyn LlmProvider>` from a [`GatewayConfig`] and model string.
///
/// Falls back to [`StubProvider`] when no API key can be resolved.
pub fn build_provider(config: &GatewayConfig, model: &str) -> Arc<dyn LlmProvider> {
    let (provider_name, model_name) = parse_model_string(model);

    let provider_config = config.llm_providers.get(provider_name);

    let api_key = provider_config
        .and_then(|c| c.api_key.clone())
        .or_else(|| provider_api_key_from_env(provider_name));

    let api_key = match api_key {
        Some(k) => k,
        None => {
            tracing::warn!(
                "No API key for provider '{provider_name}'; falling back to StubProvider"
            );
            return Arc::new(StubProvider {
                model: model.to_string(),
            });
        }
    };

    let base_url = provider_config.and_then(|c| c.base_url.clone());

    match provider_name {
        "openai" => {
            let mut p = OpenAiProvider::new(&api_key).with_model(model_name);
            if let Some(url) = base_url {
                p = p.with_base_url(url);
            }
            Arc::new(p)
        }
        "anthropic" => {
            let mut p = AnthropicProvider::new(&api_key).with_model(model_name);
            if let Some(url) = base_url {
                p = p.with_base_url(url);
            }
            Arc::new(p)
        }
        "openrouter" => {
            let mut p = OpenRouterProvider::new(&api_key).with_model(model_name);
            if let Some(url) = base_url {
                p = p.with_base_url(url);
            }
            if let Some(cfg) = provider_config {
                if !cfg.provider_order.is_empty() {
                    p = p.with_provider_order(cfg.provider_order.clone());
                }
            }
            Arc::new(p)
        }
        "qwen" => {
            let mut p = QwenProvider::new(&api_key).with_model(model_name);
            if let Some(url) = base_url {
                p = p.with_base_url(url);
            }
            Arc::new(p)
        }
        "kimi" | "moonshot" => {
            let mut p = KimiProvider::new(&api_key).with_model(model_name);
            if let Some(url) = base_url {
                p = p.with_base_url(url);
            }
            Arc::new(p)
        }
        "minimax" => {
            let mut p = MiniMaxProvider::new(&api_key).with_model(model_name);
            if let Some(url) = base_url {
                p = p.with_base_url(url);
            }
            Arc::new(p)
        }
        "nous" => {
            let mut p = NousProvider::new(&api_key).with_model(model_name);
            if let Some(url) = base_url {
                p = p.with_base_url(url);
            }
            Arc::new(p)
        }
        "copilot" => {
            let p = CopilotProvider::new(
                base_url.unwrap_or_else(|| "https://api.github.com/copilot".to_string()),
                &api_key,
            )
            .with_model(model_name);
            Arc::new(p)
        }
        _ => {
            let url = base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string());
            Arc::new(GenericProvider::new(url, &api_key, model_name))
        }
    }
}

// ---------------------------------------------------------------------------
// StubProvider — fallback when no API key is configured
// ---------------------------------------------------------------------------

/// Fallback LLM provider returned by [`build_provider`] when no API key
/// can be resolved for the requested provider.  Every call returns an error
/// directing the user to configure credentials.
pub struct StubProvider {
    pub model: String,
}

#[async_trait::async_trait]
impl LlmProvider for StubProvider {
    async fn chat_completion(
        &self,
        _messages: &[hermes_core::Message],
        _tools: &[hermes_core::ToolSchema],
        _max_tokens: Option<u32>,
        _temperature: Option<f64>,
        _model: Option<&str>,
        _extra_body: Option<&Value>,
    ) -> Result<hermes_core::LlmResponse, AgentError> {
        Err(AgentError::LlmApi(format!(
            "StubProvider: no LLM backend configured for model '{}'. \
             Configure an API key and provider in the config file.",
            self.model
        )))
    }

    fn chat_completion_stream(
        &self,
        _messages: &[hermes_core::Message],
        _tools: &[hermes_core::ToolSchema],
        _max_tokens: Option<u32>,
        _temperature: Option<f64>,
        _model: Option<&str>,
        _extra_body: Option<&Value>,
    ) -> futures::stream::BoxStream<'static, Result<hermes_core::StreamChunk, AgentError>> {
        futures::stream::once(async move {
            Err(AgentError::LlmApi(
                "StubProvider: no LLM backend configured for streaming.".to_string(),
            ))
        })
        .boxed()
    }
}

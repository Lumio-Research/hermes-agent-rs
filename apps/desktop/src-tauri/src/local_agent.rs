//! Local Agent mode — runs hermes-agent directly in the Tauri process.

use std::sync::Arc;

use hermes_agent::agent_builder::{bridge_tool_registry, build_agent_config, build_provider};
use hermes_agent::agent_loop::{AgentCallbacks, AgentConfig, AgentLoop};
use hermes_config::GatewayConfig;
use hermes_core::{LlmProvider, Message, MessageRole};
use hermes_tools::ToolRegistry;
use serde_json::Value;

type AgentToolRegistry = hermes_agent::agent_loop::ToolRegistry;

/// Run the agent with streaming callbacks.
pub async fn run_agent_streaming<F>(
    user_message: &str,
    model: &str,
    on_delta: F,
) -> Result<String, String>
where
    F: Fn(&str, &str, Option<&str>) + Send + Sync + 'static,
{
    let config = hermes_config::load_config(None)
        .map_err(|e| format!("Failed to load config: {}", e))?;

    let effective_model = if model.is_empty() {
        config.model.clone().unwrap_or_else(|| "openrouter:openai/gpt-4o-mini".to_string())
    } else {
        model.to_string()
    };

    eprintln!("[local-agent] model={}, provider_count={}", effective_model, config.llm_providers.len());
    for (name, cfg) in &config.llm_providers {
        eprintln!("[local-agent] provider '{}': has_key={}, base_url={:?}", name, cfg.api_key.is_some(), cfg.base_url);
    }

    let provider = build_provider(&config, &effective_model);
    let agent_config = build_agent_config(&config, &effective_model, Some("app"));

    eprintln!("[local-agent] calling LLM API...");

    // Build tool registry
    let tool_registry = ToolRegistry::new();
    let agent_tool_registry = Arc::new(bridge_tool_registry(&tool_registry));

    // Set up streaming callbacks
    let on_delta = Arc::new(on_delta);
    let delta_text = on_delta.clone();
    let delta_think = on_delta.clone();
    let delta_tool_start = on_delta.clone();
    let delta_tool_complete = on_delta.clone();
    let delta_status = on_delta.clone();

    let callbacks = AgentCallbacks {
        on_stream_delta: None, // Handled by run_stream's stream_callback
        on_thinking: Some(Box::new(move |text| {
            delta_think("thinking", text, None);
        })),
        on_tool_start: Some(Box::new(move |name, _params| {
            delta_tool_start("tool_start", &format!("执行工具: {}", name), Some(name));
        })),
        on_tool_complete: Some(Box::new(move |name, result| {
            let preview = if result.len() > 200 {
                format!("{}...", &result[..200])
            } else {
                result.to_string()
            };
            delta_tool_complete("tool_complete", &preview, Some(name));
        })),
        on_step_complete: None,
        background_review_callback: None,
        status_callback: Some(Arc::new(move |category, msg| {
            if category == "activity" {
                delta_status("activity", msg, None);
            } else {
                delta_status("status", &format!("[{}] {}", category, msg), None);
            }
        })),
    };

    // Create agent with callbacks
    let agent = AgentLoop::new(agent_config, agent_tool_registry, provider)
        .with_callbacks(callbacks);

    // Build messages
    let messages = vec![Message::user(user_message)];

    // Run the agent with streaming
    eprintln!("[local-agent] running agent.run_stream()...");

    let on_delta_for_stream = on_delta.clone();
    let stream_callback: Box<dyn Fn(hermes_core::StreamChunk) + Send + Sync> =
        Box::new(move |chunk: hermes_core::StreamChunk| {
            if let Some(ref delta) = chunk.delta {
                if let Some(ref text) = delta.content {
                    eprintln!("[local-agent] STREAM CHUNK: {} chars", text.len());
                    on_delta_for_stream("text", text, None);
                }
            }
        });

    let result = agent
        .run_stream(messages, None, Some(stream_callback))
        .await
        .map_err(|e| {
            eprintln!("[local-agent] agent error: {}", e);
            format!("Agent error: {}", e)
        })?;

    eprintln!("[local-agent] agent done, {} messages returned", result.messages.len());

    // Extract the last assistant reply
    let reply = result
        .messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, MessageRole::Assistant))
        .and_then(|m| m.content.clone())
        .unwrap_or_else(|| "(no reply)".to_string());

    Ok(reply)
}



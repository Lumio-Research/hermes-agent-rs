//! Environment variable (API key) management endpoints.

use axum::extract::State;
use axum::Json;
use std::collections::HashMap;

use crate::{HttpError, HttpServerState};
use super::types::*;

/// Known environment variables with metadata.
struct EnvVarMeta {
    description: &'static str,
    url: Option<&'static str>,
    category: &'static str,
    is_password: bool,
    tools: &'static [&'static str],
    advanced: bool,
}

fn known_env_vars() -> Vec<(&'static str, EnvVarMeta)> {
    vec![
        ("OPENAI_API_KEY", EnvVarMeta {
            description: "OpenAI API key for GPT models",
            url: Some("https://platform.openai.com/api-keys"),
            category: "provider", is_password: true, tools: &[], advanced: false,
        }),
        ("ANTHROPIC_API_KEY", EnvVarMeta {
            description: "Anthropic API key for Claude models",
            url: Some("https://console.anthropic.com/settings/keys"),
            category: "provider", is_password: true, tools: &[], advanced: false,
        }),
        ("OPENROUTER_API_KEY", EnvVarMeta {
            description: "OpenRouter API key for multi-model routing",
            url: Some("https://openrouter.ai/keys"),
            category: "provider", is_password: true, tools: &[], advanced: false,
        }),
        ("DEEPSEEK_API_KEY", EnvVarMeta {
            description: "DeepSeek API key",
            url: Some("https://platform.deepseek.com/api_keys"),
            category: "provider", is_password: true, tools: &[], advanced: false,
        }),
        ("DASHSCOPE_API_KEY", EnvVarMeta {
            description: "DashScope (Qwen) API key",
            url: Some("https://dashscope.console.aliyun.com/apiKey"),
            category: "provider", is_password: true, tools: &[], advanced: false,
        }),
        ("KIMI_API_KEY", EnvVarMeta {
            description: "Kimi / Moonshot API key",
            url: Some("https://platform.moonshot.cn/console/api-keys"),
            category: "provider", is_password: true, tools: &[], advanced: false,
        }),
        ("MINIMAX_API_KEY", EnvVarMeta {
            description: "MiniMax API key",
            url: Some("https://platform.minimaxi.com/user-center/basic-information/interface-key"),
            category: "provider", is_password: true, tools: &[], advanced: false,
        }),
        ("EXA_API_KEY", EnvVarMeta {
            description: "Exa search API key",
            url: Some("https://exa.ai"),
            category: "tool", is_password: true, tools: &["web_search"], advanced: false,
        }),
        ("FIRECRAWL_API_KEY", EnvVarMeta {
            description: "Firecrawl web scraping API key",
            url: Some("https://firecrawl.dev"),
            category: "tool", is_password: true, tools: &["web_search", "web_extract"], advanced: false,
        }),
        ("ELEVENLABS_API_KEY", EnvVarMeta {
            description: "ElevenLabs text-to-speech API key",
            url: Some("https://elevenlabs.io"),
            category: "tool", is_password: true, tools: &["tts"], advanced: true,
        }),
    ]
}

fn redact(value: &str) -> String {
    if value.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}...{}", &value[..4], &value[value.len()-4..])
    }
}

fn read_env_file(hermes_home: &std::path::Path) -> HashMap<String, String> {
    let env_path = hermes_home.join(".env");
    let mut map = HashMap::new();
    if let Ok(content) = std::fs::read_to_string(&env_path) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || !line.contains('=') {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                let val = val.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    map.insert(key.trim().to_string(), val.to_string());
                }
            }
        }
    }
    map
}

fn write_env_file(hermes_home: &std::path::Path, vars: &HashMap<String, String>) {
    let env_path = hermes_home.join(".env");
    let mut lines = Vec::new();
    for (key, val) in vars {
        lines.push(format!("{}=\"{}\"", key, val));
    }
    lines.sort();
    let _ = std::fs::write(env_path, lines.join("\n") + "\n");
}

/// GET /api/env
pub async fn get_env_vars(
    State(state): State<HttpServerState>,
) -> Json<HashMap<String, EnvVarInfo>> {
    let file_vars = read_env_file(&state.hermes_home);
    let mut result = HashMap::new();

    for (key, meta) in known_env_vars() {
        let env_val = std::env::var(key).ok().or_else(|| file_vars.get(key).cloned());
        let is_set = env_val.is_some();
        let redacted_value = env_val.as_deref().map(redact);

        result.insert(key.to_string(), EnvVarInfo {
            is_set,
            redacted_value,
            description: meta.description.to_string(),
            url: meta.url.map(|s| s.to_string()),
            category: meta.category.to_string(),
            is_password: meta.is_password,
            tools: meta.tools.iter().map(|s| s.to_string()).collect(),
            advanced: meta.advanced,
        });
    }

    Json(result)
}

/// PUT /api/env
pub async fn set_env_var(
    State(state): State<HttpServerState>,
    Json(body): Json<EnvVarUpdate>,
) -> Result<Json<OkResponse>, HttpError> {
    let mut vars = read_env_file(&state.hermes_home);
    vars.insert(body.key.clone(), body.value.clone());
    write_env_file(&state.hermes_home, &vars);
    // Also set in current process
    std::env::set_var(&body.key, &body.value);
    Ok(Json(OkResponse { ok: true }))
}

/// DELETE /api/env
pub async fn delete_env_var(
    State(state): State<HttpServerState>,
    Json(body): Json<EnvVarDelete>,
) -> Result<Json<OkResponse>, HttpError> {
    let mut vars = read_env_file(&state.hermes_home);
    vars.remove(&body.key);
    write_env_file(&state.hermes_home, &vars);
    std::env::remove_var(&body.key);
    Ok(Json(OkResponse { ok: true }))
}

/// POST /api/env/reveal
pub async fn reveal_env_var(
    State(state): State<HttpServerState>,
    Json(body): Json<EnvVarReveal>,
) -> Result<Json<serde_json::Value>, HttpError> {
    let file_vars = read_env_file(&state.hermes_home);
    let value = std::env::var(&body.key)
        .ok()
        .or_else(|| file_vars.get(&body.key).cloned())
        .unwrap_or_default();

    Ok(Json(serde_json::json!({
        "key": body.key,
        "value": value,
    })))
}

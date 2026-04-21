//! Tool call parsers for various LLM output formats.
//!
//! Different models emit tool calls in different formats:
//! - **Anthropic** (Claude): `<tool_call>` XML blocks or native API tool_use
//! - **OpenAI** (GPT): JSON function_call in the API response
//! - **Hermes** (fine-tuned): `<tool_call>{"name":...,"arguments":...}</tool_call>`
//! - **Qwen**: `<tool_call>` blocks similar to Hermes format
//! - **Llama/Mistral**: `[TOOL_CALL]` or `<function=name>` formats
//! - **DeepSeek**: `<tool_call>` with slightly different JSON structure
//!
//! Each parser extracts a list of `ParsedToolCall` from raw model output text.

use serde::{Deserialize, Serialize};

/// A parsed tool call extracted from model output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParsedToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
    /// Raw text span that was parsed (for debugging).
    pub raw_span: Option<String>,
}

/// Parser identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParserKind {
    Hermes,
    Anthropic,
    OpenAi,
    Qwen,
    Llama,
    DeepSeek,
    /// Auto-detect from content.
    Auto,
}

/// Parse tool calls from model output text.
pub fn parse_tool_calls(text: &str, kind: ParserKind) -> Vec<ParsedToolCall> {
    match kind {
        ParserKind::Hermes => parse_hermes(text),
        ParserKind::Anthropic => parse_anthropic(text),
        ParserKind::OpenAi => parse_openai_text(text),
        ParserKind::Qwen => parse_qwen(text),
        ParserKind::Llama => parse_llama(text),
        ParserKind::DeepSeek => parse_deepseek(text),
        ParserKind::Auto => parse_auto(text),
    }
}

/// Auto-detect parser from content patterns.
pub fn parse_auto(text: &str) -> Vec<ParsedToolCall> {
    // Try each parser in order of specificity
    let hermes = parse_hermes(text);
    if !hermes.is_empty() {
        return hermes;
    }
    let anthropic = parse_anthropic(text);
    if !anthropic.is_empty() {
        return anthropic;
    }
    let llama = parse_llama(text);
    if !llama.is_empty() {
        return llama;
    }
    let openai = parse_openai_text(text);
    if !openai.is_empty() {
        return openai;
    }
    Vec::new()
}

// ---------------------------------------------------------------------------
// Hermes format: <tool_call>{"name":"...","arguments":{...}}</tool_call>
// ---------------------------------------------------------------------------

fn parse_hermes(text: &str) -> Vec<ParsedToolCall> {
    let mut calls = Vec::new();
    let open_tag = "<tool_call>";
    let close_tag = "</tool_call>";

    let mut search_from = 0;
    while let Some(start) = text[search_from..].find(open_tag) {
        let abs_start = search_from + start + open_tag.len();
        if let Some(end) = text[abs_start..].find(close_tag) {
            let abs_end = abs_start + end;
            let content = text[abs_start..abs_end].trim();
            if let Some(call) = parse_json_tool_call(content) {
                calls.push(ParsedToolCall {
                    raw_span: Some(text[search_from + start..abs_end + close_tag.len()].to_string()),
                    ..call
                });
            }
            search_from = abs_end + close_tag.len();
        } else {
            break;
        }
    }
    calls
}

// ---------------------------------------------------------------------------
// Anthropic format: <tool_use> blocks or <invoke> blocks
// ---------------------------------------------------------------------------

fn parse_anthropic(text: &str) -> Vec<ParsedToolCall> {
    let mut calls = Vec::new();

    // Pattern 1: <invoke name="tool_name">{"key":"value"}</invoke>
    let invoke_re = regex::Regex::new(
        r#"<invoke\s+name="([^"]+)"[^>]*>([\s\S]*?)</invoke>"#
    ).unwrap();
    for cap in invoke_re.captures_iter(text) {
        let name = cap[1].to_string();
        let body = cap[2].trim();
        let arguments = serde_json::from_str(body).unwrap_or(serde_json::json!({}));
        calls.push(ParsedToolCall {
            name,
            arguments,
            raw_span: Some(cap[0].to_string()),
        });
    }

    // Pattern 2: <tool_use><name>tool_name</name><input>{...}</input></tool_use>
    if calls.is_empty() {
        let tool_use_re = regex::Regex::new(
            r#"<tool_use>\s*<name>([^<]+)</name>\s*<input>([\s\S]*?)</input>\s*</tool_use>"#
        ).unwrap();
        for cap in tool_use_re.captures_iter(text) {
            let name = cap[1].trim().to_string();
            let body = cap[2].trim();
            let arguments = serde_json::from_str(body).unwrap_or(serde_json::json!({}));
            calls.push(ParsedToolCall {
                name,
                arguments,
                raw_span: Some(cap[0].to_string()),
            });
        }
    }

    calls
}

// ---------------------------------------------------------------------------
// OpenAI text format (when model outputs JSON in text, not via API)
// ---------------------------------------------------------------------------

fn parse_openai_text(text: &str) -> Vec<ParsedToolCall> {
    let mut calls = Vec::new();

    // Pattern: {"function_call": {"name": "...", "arguments": "..."}}
    // or: {"name": "...", "arguments": {...}}
    let re = regex::Regex::new(
        r#"\{[^{}]*"name"\s*:\s*"([^"]+)"[^{}]*"arguments"\s*:\s*(\{[^}]*\}|"[^"]*")[^{}]*\}"#
    ).unwrap();

    for cap in re.captures_iter(text) {
        let name = cap[1].to_string();
        let args_raw = &cap[2];
        let arguments = if args_raw.starts_with('"') {
            // Arguments as escaped JSON string
            let unescaped = args_raw.trim_matches('"');
            serde_json::from_str(unescaped).unwrap_or(serde_json::json!({}))
        } else {
            serde_json::from_str(args_raw).unwrap_or(serde_json::json!({}))
        };
        calls.push(ParsedToolCall {
            name,
            arguments,
            raw_span: Some(cap[0].to_string()),
        });
    }

    calls
}

// ---------------------------------------------------------------------------
// Qwen format: similar to Hermes but may use ✿FUNCTION✿ markers
// ---------------------------------------------------------------------------

fn parse_qwen(text: &str) -> Vec<ParsedToolCall> {
    let mut calls = Vec::new();

    // Pattern 1: ✿FUNCTION✿: tool_name\n✿ARGS✿: {...}\n✿RESULT✿
    let qwen_re = regex::Regex::new(
        r"✿FUNCTION✿:\s*(\S+)\s*\n✿ARGS✿:\s*([\s\S]*?)(?:\n✿RESULT✿|$)"
    ).unwrap();
    for cap in qwen_re.captures_iter(text) {
        let name = cap[1].trim().to_string();
        let body = cap[2].trim();
        let arguments = serde_json::from_str(body).unwrap_or(serde_json::json!({}));
        calls.push(ParsedToolCall {
            name,
            arguments,
            raw_span: Some(cap[0].to_string()),
        });
    }

    // Pattern 2: Fall back to Hermes-style <tool_call> tags
    if calls.is_empty() {
        calls = parse_hermes(text);
    }

    calls
}

// ---------------------------------------------------------------------------
// Llama / Mistral format: <function=name>{...}</function> or [TOOL_CALL]
// ---------------------------------------------------------------------------

fn parse_llama(text: &str) -> Vec<ParsedToolCall> {
    let mut calls = Vec::new();

    // Pattern 1: <function=tool_name>{"key":"value"}</function>
    let func_re = regex::Regex::new(
        r#"<function=(\w+)>([\s\S]*?)</function>"#
    ).unwrap();
    for cap in func_re.captures_iter(text) {
        let name = cap[1].to_string();
        let body = cap[2].trim();
        let arguments = serde_json::from_str(body).unwrap_or(serde_json::json!({}));
        calls.push(ParsedToolCall {
            name,
            arguments,
            raw_span: Some(cap[0].to_string()),
        });
    }

    // Pattern 2: [TOOL_CALL] {"name":"...","arguments":{...}} [/TOOL_CALL]
    if calls.is_empty() {
        let tool_re = regex::Regex::new(
            r"\[TOOL_CALL\]\s*([\s\S]*?)\s*\[/TOOL_CALL\]"
        ).unwrap();
        for cap in tool_re.captures_iter(text) {
            let content = cap[1].trim();
            if let Some(call) = parse_json_tool_call(content) {
                calls.push(ParsedToolCall {
                    raw_span: Some(cap[0].to_string()),
                    ..call
                });
            }
        }
    }

    calls
}

// ---------------------------------------------------------------------------
// DeepSeek format: <tool_call> with id field
// ---------------------------------------------------------------------------

fn parse_deepseek(text: &str) -> Vec<ParsedToolCall> {
    // DeepSeek uses <tool_call> tags like Hermes but may include an "id" field
    parse_hermes(text)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a JSON blob that contains "name" and "arguments" fields.
fn parse_json_tool_call(json_str: &str) -> Option<ParsedToolCall> {
    let v: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let name = v.get("name")?.as_str()?.to_string();
    let arguments = v.get("arguments").cloned().unwrap_or(serde_json::json!({}));
    // If arguments is a string, try to parse it as JSON
    let arguments = if let Some(s) = arguments.as_str() {
        serde_json::from_str(s).unwrap_or(serde_json::json!({}))
    } else {
        arguments
    };
    Some(ParsedToolCall {
        name,
        arguments,
        raw_span: None,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hermes_single_tool_call() {
        let text = r#"Let me check that. <tool_call>{"name":"terminal","arguments":{"command":"ls -la"}}</tool_call>"#;
        let calls = parse_tool_calls(text, ParserKind::Hermes);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "terminal");
        assert_eq!(calls[0].arguments["command"], "ls -la");
    }

    #[test]
    fn hermes_multiple_tool_calls() {
        let text = r#"<tool_call>{"name":"read_file","arguments":{"path":"a.txt"}}</tool_call> then <tool_call>{"name":"write_file","arguments":{"path":"b.txt","content":"hi"}}</tool_call>"#;
        let calls = parse_tool_calls(text, ParserKind::Hermes);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[1].name, "write_file");
    }

    #[test]
    fn anthropic_invoke_format() {
        let text = r#"<invoke name="terminal">{"command": "pwd"}</invoke>"#;
        let calls = parse_tool_calls(text, ParserKind::Anthropic);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "terminal");
        assert_eq!(calls[0].arguments["command"], "pwd");
    }

    #[test]
    fn anthropic_tool_use_format() {
        let text = r#"<tool_use><name>read_file</name><input>{"path": "/etc/hosts"}</input></tool_use>"#;
        let calls = parse_tool_calls(text, ParserKind::Anthropic);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
    }

    #[test]
    fn llama_function_format() {
        let text = r#"<function=terminal>{"command": "echo hello"}</function>"#;
        let calls = parse_tool_calls(text, ParserKind::Llama);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "terminal");
    }

    #[test]
    fn llama_tool_call_brackets() {
        let text = r#"[TOOL_CALL] {"name":"search","arguments":{"query":"rust"}} [/TOOL_CALL]"#;
        let calls = parse_tool_calls(text, ParserKind::Llama);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "search");
    }

    #[test]
    fn qwen_function_markers() {
        let text = "✿FUNCTION✿: terminal\n✿ARGS✿: {\"command\": \"ls\"}\n✿RESULT✿";
        let calls = parse_tool_calls(text, ParserKind::Qwen);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "terminal");
    }

    #[test]
    fn auto_detect_hermes() {
        let text = r#"<tool_call>{"name":"test","arguments":{}}</tool_call>"#;
        let calls = parse_tool_calls(text, ParserKind::Auto);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "test");
    }

    #[test]
    fn auto_detect_anthropic() {
        let text = r#"<invoke name="test">{"key":"val"}</invoke>"#;
        let calls = parse_tool_calls(text, ParserKind::Auto);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "test");
    }

    #[test]
    fn no_tool_calls_returns_empty() {
        let text = "Just a normal response with no tool calls.";
        let calls = parse_tool_calls(text, ParserKind::Auto);
        assert!(calls.is_empty());
    }

    #[test]
    fn malformed_json_skipped() {
        let text = r#"<tool_call>not valid json</tool_call>"#;
        let calls = parse_tool_calls(text, ParserKind::Hermes);
        assert!(calls.is_empty());
    }

    #[test]
    fn deepseek_uses_hermes_format() {
        let text = r#"<tool_call>{"name":"code","arguments":{"lang":"python"}}</tool_call>"#;
        let calls = parse_tool_calls(text, ParserKind::DeepSeek);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "code");
    }
}

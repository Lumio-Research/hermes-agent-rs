//! `hermes doctor` — dependency and configuration health checks.

use std::path::Path;
use std::process::Command;

use hermes_config::{hermes_home, load_config};
use hermes_core::AgentError;
use hermes_gateway::{evaluate_gateway_requirements, RequirementScope, RequirementSeverity};

/// A single check result.
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
}

pub enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

impl CheckResult {
    fn ok(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Ok,
            detail: detail.into(),
        }
    }
    fn warn(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Warn,
            detail: detail.into(),
        }
    }
    fn fail(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Fail,
            detail: detail.into(),
        }
    }

    pub fn icon(&self) -> &'static str {
        match self.status {
            CheckStatus::Ok => "✓",
            CheckStatus::Warn => "⚠",
            CheckStatus::Fail => "✗",
        }
    }
}

/// Run all doctor checks and return results.
pub fn run_doctor() -> Vec<CheckResult> {
    let mut results = Vec::new();

    // System info
    results.push(CheckResult::ok(
        "System",
        format!(
            "{} / {} / {}",
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::env::consts::FAMILY
        ),
    ));

    // Hermes home directory
    let home = hermes_home();
    if home.exists() {
        results.push(CheckResult::ok(
            "Hermes home",
            format!("{} (exists)", home.display()),
        ));
    } else {
        results.push(CheckResult::warn(
            "Hermes home",
            format!(
                "{} (not found — will be created on first run)",
                home.display()
            ),
        ));
    }

    // Config file
    check_config(&home, &mut results);

    check_gateway_platform_requirements(&mut results);

    // API keys
    check_api_keys(&mut results);

    // External tools
    check_external_tool("git", &["--version"], &mut results);
    check_external_tool("python3", &["--version"], &mut results);
    check_external_tool("node", &["--version"], &mut results);

    // SQLite (bundled, always available)
    results.push(CheckResult::ok("SQLite", "bundled (rusqlite)"));

    // Memory files
    check_memory_files(&home, &mut results);

    // Skills directory
    let skills_dir = home.join("skills");
    if skills_dir.exists() {
        let count = std::fs::read_dir(&skills_dir)
            .map(|entries| entries.filter_map(|e| e.ok()).count())
            .unwrap_or(0);
        results.push(CheckResult::ok(
            "Skills",
            format!("{} skills in {}", count, skills_dir.display()),
        ));
    } else {
        results.push(CheckResult::ok(
            "Skills",
            "no skills directory (none installed)",
        ));
    }

    results
}

/// Enabled gateway platforms: credential preflight via `hermes-gateway` evaluator.
fn check_gateway_platform_requirements(results: &mut Vec<CheckResult>) {
    match load_config(None) {
        Ok(cfg) => {
            for issue in evaluate_gateway_requirements(&cfg, RequirementScope::Doctor) {
                let name = format!("Gateway / {}", issue.platform);
                let detail = format!("[{}] {}", issue.code, issue.message);
                match issue.severity {
                    RequirementSeverity::Fatal => {
                        results.push(CheckResult::fail(name, detail));
                    }
                    RequirementSeverity::Warn => {
                        results.push(CheckResult::warn(name, detail));
                    }
                }
            }
        }
        Err(_) => {}
    }
}

fn check_config(home: &Path, results: &mut Vec<CheckResult>) {
    let config_path = home.join("config.yaml");
    if !config_path.exists() {
        results.push(CheckResult::warn(
            "Config",
            "config.yaml not found — using defaults",
        ));
        return;
    }

    match load_config(None) {
        Ok(_cfg) => {
            results.push(CheckResult::ok(
                "Config",
                format!("{} (valid)", config_path.display()),
            ));
        }
        Err(e) => {
            results.push(CheckResult::fail("Config", format!("parse error: {}", e)));
        }
    }
}

fn check_api_keys(results: &mut Vec<CheckResult>) {
    let keys = [
        ("ANTHROPIC_API_KEY", "Anthropic (Claude)"),
        ("OPENAI_API_KEY", "OpenAI"),
        ("OPENROUTER_API_KEY", "OpenRouter"),
        ("EXA_API_KEY", "Exa (web search)"),
        ("ELEVENLABS_API_KEY", "ElevenLabs (TTS)"),
    ];

    let mut found = 0;
    for (env_var, label) in &keys {
        if std::env::var(env_var).is_ok() {
            found += 1;
            results.push(CheckResult::ok(
                format!("API key: {}", label),
                format!("{} is set", env_var),
            ));
        }
    }

    if found == 0 {
        results.push(CheckResult::warn(
            "API keys",
            "No LLM API keys found. Set ANTHROPIC_API_KEY, OPENAI_API_KEY, or OPENROUTER_API_KEY.",
        ));
    }
}

fn check_external_tool(name: &str, args: &[&str], results: &mut Vec<CheckResult>) {
    match Command::new(name).args(args).output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            results.push(CheckResult::ok(name, version));
        }
        Ok(_) => {
            results.push(CheckResult::warn(name, "found but returned error"));
        }
        Err(_) => {
            results.push(CheckResult::warn(name, "not found (optional)"));
        }
    }
}

fn check_memory_files(home: &Path, results: &mut Vec<CheckResult>) {
    let memory_dir = home.join("memories");
    let memory_md = memory_dir.join("MEMORY.md");
    let user_md = memory_dir.join("USER.md");

    if memory_md.exists() {
        let size = std::fs::metadata(&memory_md).map(|m| m.len()).unwrap_or(0);
        results.push(CheckResult::ok("MEMORY.md", format!("{} bytes", size)));
    } else {
        results.push(CheckResult::ok(
            "MEMORY.md",
            "not yet created (will be created on first memory write)",
        ));
    }

    if user_md.exists() {
        let size = std::fs::metadata(&user_md).map(|m| m.len()).unwrap_or(0);
        results.push(CheckResult::ok("USER.md", format!("{} bytes", size)));
    } else {
        results.push(CheckResult::ok("USER.md", "not yet created"));
    }
}

/// Legacy wrapper for backward compatibility.
pub fn run_basic_checks() -> Result<Vec<String>, AgentError> {
    let results = run_doctor();
    Ok(results
        .iter()
        .map(|r| format!("{} {}: {}", r.icon(), r.name, r.detail))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_runs_without_panic() {
        let results = run_doctor();
        assert!(!results.is_empty());
        // System check should always be present
        assert!(results.iter().any(|r| r.name == "System"));
    }

    #[test]
    fn doctor_detects_system_info() {
        let results = run_doctor();
        let system = results.iter().find(|r| r.name == "System").unwrap();
        assert!(system.detail.contains(std::env::consts::OS));
    }

    #[test]
    fn basic_checks_returns_strings() {
        let checks = run_basic_checks().unwrap();
        assert!(!checks.is_empty());
        assert!(checks[0].contains("System"));
    }
}

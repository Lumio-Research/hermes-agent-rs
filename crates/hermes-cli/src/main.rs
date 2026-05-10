#![allow(
    clippy::type_complexity,
    clippy::print_literal,
    clippy::field_reassign_with_default,
    clippy::useless_format,
    clippy::needless_borrow,
    clippy::unwrap_or_default,
    unused_variables
)]
//! Hermes Agent — binary entry point.
//!
//! Initializes logging, parses CLI arguments, and dispatches to the
//! appropriate subcommand handler.

use clap::CommandFactory;
use clap::Parser;
use clap_complete::{generate, Shell as CompletionShell};
use hermes_cli::app::provider_api_key_from_env;
use hermes_cli::cli::{Cli, CliCommand};
use hermes_cli::App;
use hermes_config::{
    apply_user_config_patch, gateway_pid_path_in, hermes_home, load_config, load_user_config_file,
    platform_token_or_extra, save_config_yaml, state_dir, user_config_field_display,
    validate_config, ConfigError, PlatformConfig,
};
use hermes_core::AgentError;
#[cfg(test)]
use hermes_core::PlatformAdapter;
use hermes_cron::{cron_scheduler_for_data_dir, CronError};
#[cfg(test)]
use hermes_gateway::Gateway;
use hermes_telemetry::init_telemetry_from_env;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::Arc;

mod oauth_store;
use oauth_store::{AuthManager, FileTokenStore, OAuthCredential};
#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize tracing
    init_tracing(cli.verbose);

    tracing::debug!("Hermes Agent starting");

    let result = match cli.effective_command() {
        CliCommand::Hermes => run_interactive(cli).await,
        CliCommand::Chat {
            query,
            preload_skill,
            yolo,
        } => hermes_cli::commands::handle_cli_chat(query, preload_skill, yolo).await,
        CliCommand::Model {
            provider_model,
            sub_action,
            sub_arg,
        } => {
            // "hermes model provider list" → delegate to runtime-provider handler
            if provider_model.as_deref() == Some("provider") {
                hermes_cli::commands::handle_cli_runtime_provider(sub_action, sub_arg).await
            } else {
                run_model(cli, provider_model).await
            }
        }
        CliCommand::Tools { action } => run_tools(cli, action).await,
        CliCommand::Config { action, key, value } => {
            // "hermes config set region <value>" → delegate to region handler
            if action.as_deref() == Some("set") && key.as_deref() == Some("region") {
                hermes_cli::commands::handle_cli_region(Some("set".to_string()), value).await
            } else {
                run_config(cli, action, key, value).await
            }
        }
        CliCommand::Gateway { action, platform } => {
            // "hermes gateway setup whatsapp" → delegate to whatsapp handler
            if let (Some("setup"), Some(plat)) = (action.as_deref(), platform.as_deref()) {
                match plat {
                    "whatsapp" | "wa" => {
                        hermes_cli::commands::handle_cli_whatsapp(Some("setup".to_string())).await
                    }
                    _ => run_gateway(cli, action).await,
                }
            } else {
                run_gateway(cli, action).await
            }
        }
        CliCommand::Setup => run_setup().await,
        CliCommand::Doctor => run_doctor(cli).await,
        CliCommand::Update => run_update().await,
        CliCommand::Status => run_status(cli).await,
        CliCommand::Logs { lines, follow } => run_logs(cli, lines, follow).await,
        CliCommand::Profile { action, name } => run_profile(cli, action, name).await,
        CliCommand::Auth {
            action,
            provider,
            qr,
            model,
        } => {
            // "hermes auth login lumio" → delegate to lumio handler
            if action.as_deref() == Some("login") && provider.as_deref() == Some("lumio") {
                run_lumio(Some("login".to_string()), model).await
            } else {
                run_auth(cli, action, provider, qr).await
            }
        }
        CliCommand::Skills {
            action,
            name,
            extra,
        } => hermes_cli::commands::handle_cli_skills(action, name, extra).await,
        CliCommand::Plugins {
            action,
            name,
            git_ref,
            allow_untrusted_git_host,
        } => {
            hermes_cli::commands::handle_cli_plugins(
                action,
                name,
                git_ref,
                allow_untrusted_git_host,
            )
            .await
        }
        CliCommand::Memory { action, provider } => {
            // "hermes memory setup redis" → delegate to memory-setup handler
            if action.as_deref() == Some("setup") || provider.is_some() {
                hermes_cli::commands::handle_cli_memory_setup(action, provider).await
            } else {
                hermes_cli::commands::handle_cli_memory(action).await
            }
        }
        CliCommand::Mcp { action, server } => {
            hermes_cli::commands::handle_cli_mcp(action, server).await
        }
        CliCommand::Sessions {
            action,
            id,
            name,
            output,
        } => {
            // "hermes sessions dump" → delegate to dump
            if action.as_deref() == Some("dump") {
                run_dump(cli, id, output).await
            } else if action.as_deref() == Some("stats") {
                hermes_cli::commands::handle_cli_insights(30, None).await
            } else {
                hermes_cli::commands::handle_cli_sessions(action, id, name).await
            }
        }
        CliCommand::Acp { action } => hermes_cli::commands::handle_cli_acp(action).await,
        CliCommand::Backup { output } => hermes_cli::commands::handle_cli_backup(output).await,
        CliCommand::Import { path } => hermes_cli::commands::handle_cli_import(path).await,
        CliCommand::Version => hermes_cli::commands::handle_cli_version(),
        CliCommand::Cron {
            action,
            id,
            schedule,
            prompt,
        } => run_cron(cli, action, id, schedule, prompt).await,
        CliCommand::Serve {
            action,
            host,
            port,
            no_gateway,
            no_cron,
        } => run_serve(cli, action, host, port, no_gateway, no_cron).await,
        CliCommand::Cloud {
            action,
            agent_id,
            prompt,
            env,
            attempts,
            email,
            password,
            url_override,
            register,
            once,
            poll_interval_secs,
            limit,
            json,
        } => {
            run_cloud(
                action,
                agent_id,
                prompt,
                env,
                attempts,
                email,
                password,
                url_override,
                register,
                once,
                poll_interval_secs,
                limit,
                json,
            )
            .await
        }
        CliCommand::Completion { shell } => run_completion(shell),
        CliCommand::Uninstall { yes } => run_uninstall(yes).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

/// Initialize the tracing subscriber with env filter.
fn init_tracing(verbose: bool) {
    let default = if verbose { "debug" } else { "info" };
    init_telemetry_from_env("hermes-cli", default);
}

/// Run the interactive REPL (default command).
async fn run_interactive(cli: Cli) -> Result<(), AgentError> {
    let app = App::new(cli).await?;
    hermes_cli::tui::run(app).await
}

/// Handle `hermes model [provider:model]`.
async fn run_model(cli: Cli, provider_model: Option<String>) -> Result<(), AgentError> {
    let config =
        load_config(cli.config_dir.as_deref()).map_err(|e| AgentError::Config(e.to_string()))?;

    match provider_model {
        Some(pm) => {
            println!("Model switched to: {}", pm);
            println!("(To persist, run: hermes config set model {})", pm);
        }
        None => {
            let current = config.model.as_deref().unwrap_or("gpt-4o");
            println!("Current model: {}", current);

            // List known providers
            println!("\nAvailable providers:");
            println!("  openai       — OpenAI (gpt-4o, gpt-4o-mini, ...)");
            println!("  anthropic    — Anthropic (claude-3-5-sonnet, claude-3-opus, ...)");
            println!("  openrouter   — OpenRouter (multi-provider routing)");
            println!("\nUsage: hermes model <provider>:<model>");
        }
    }
    Ok(())
}

/// Handle `hermes tools [action]`.
async fn run_tools(_cli: Cli, action: Option<String>) -> Result<(), AgentError> {
    let registry = hermes_tools::ToolRegistry::new();
    let tools = registry.list_tools();

    match action.as_deref() {
        None | Some("list") => {
            if tools.is_empty() {
                println!("No tools registered (tools are loaded at runtime).");
                println!("\nBuilt-in tool categories:");
                let categories = [
                    "web",
                    "terminal",
                    "file",
                    "browser",
                    "vision",
                    "image_gen",
                    "skills",
                    "memory",
                    "session_search",
                    "todo",
                    "clarify",
                    "code_execution",
                    "delegation",
                    "cronjob",
                    "messaging",
                    "homeassistant",
                ];
                for cat in &categories {
                    println!("  • {}", cat);
                }
            } else {
                println!("Registered tools ({}):", tools.len());
                for tool in &tools {
                    println!("  • {} — {}", tool.name, tool.description);
                }
            }
        }
        Some(other) => {
            println!("Unknown tools action: {}. Use 'list'.", other);
        }
    }
    Ok(())
}

/// Handle `hermes config [action] [key] [value]`.
async fn run_config(
    cli: Cli,
    action: Option<String>,
    key: Option<String>,
    value: Option<String>,
) -> Result<(), AgentError> {
    let config =
        load_config(cli.config_dir.as_deref()).map_err(|e| AgentError::Config(e.to_string()))?;

    match action.as_deref() {
        None => {
            // Show full config as JSON
            let json = serde_json::to_string_pretty(&config)
                .map_err(|e| AgentError::Config(e.to_string()))?;
            println!("{}", json);
        }
        Some("get") => {
            let key = key.ok_or_else(|| {
                AgentError::Config("Missing key. Usage: hermes config get <key>".into())
            })?;
            match user_config_field_display(&config, &key) {
                Ok(s) => println!("{}", s),
                Err(ConfigError::NotFound(_)) => println!("Unknown config key: {}", key),
                Err(e) => return Err(AgentError::Config(e.to_string())),
            }
        }
        Some("set") => {
            let key = key.ok_or_else(|| {
                AgentError::Config("Missing key. Usage: hermes config set <key> <value>".into())
            })?;
            let value = value.ok_or_else(|| {
                AgentError::Config("Missing value. Usage: hermes config set <key> <value>".into())
            })?;
            let base: PathBuf = cli
                .config_dir
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(hermes_home);
            let cfg_path = base.join("config.yaml");
            let mut disk =
                load_user_config_file(&cfg_path).map_err(|e| AgentError::Config(e.to_string()))?;
            apply_user_config_patch(&mut disk, &key, &value)
                .map_err(|e| AgentError::Config(e.to_string()))?;
            validate_config(&disk).map_err(|e| AgentError::Config(e.to_string()))?;
            save_config_yaml(&cfg_path, &disk).map_err(|e| AgentError::Config(e.to_string()))?;
            println!("Saved {} = {} -> {}", key, value, cfg_path.display());
        }
        Some("show") => {
            let json = serde_json::to_string_pretty(&config)
                .map_err(|e| AgentError::Config(e.to_string()))?;
            println!("{}", json);
        }
        Some("path") => {
            let base: PathBuf = cli
                .config_dir
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(hermes_home);
            let cfg_path = base.join("config.yaml");
            println!("{}", cfg_path.display());
        }
        Some("env-path") => {
            let env_path = hermes_home().join(".env");
            println!("{}", env_path.display());
            if env_path.exists() {
                println!("(exists)");
            } else {
                println!("(not found — create it to set environment overrides)");
            }
        }
        Some("check") | Some("validate") => {
            println!("Validating configuration...");
            match validate_config(&config) {
                Ok(()) => println!("Configuration is valid. ✓"),
                Err(e) => println!("Configuration error: {}", e),
            }
        }
        Some("edit") => {
            let base: PathBuf = cli
                .config_dir
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(hermes_home);
            let cfg_path = base.join("config.yaml");
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
            println!("Opening {} with {}...", cfg_path.display(), editor);
            let status = std::process::Command::new(&editor).arg(&cfg_path).status();
            match status {
                Ok(s) if s.success() => println!("Config saved."),
                Ok(s) => println!("Editor exited with: {}", s),
                Err(e) => println!("Could not launch editor '{}': {}", editor, e),
            }
        }
        Some("migrate") => {
            println!("Config Migration");
            println!("----------------");
            let base: PathBuf = cli
                .config_dir
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(hermes_home);
            let old_json = base.join("config.json");
            let new_yaml = base.join("config.yaml");
            if old_json.exists() && !new_yaml.exists() {
                println!("Found legacy config.json — converting to config.yaml...");
                match std::fs::read_to_string(&old_json) {
                    Ok(content) => {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                            match serde_yaml::to_string(&val) {
                                Ok(yaml) => {
                                    std::fs::write(&new_yaml, &yaml)
                                        .map_err(|e| AgentError::Io(e.to_string()))?;
                                    println!("Migrated config.json -> config.yaml");
                                    println!("The old config.json was preserved.");
                                }
                                Err(e) => println!("YAML conversion error: {}", e),
                            }
                        } else {
                            println!("Could not parse config.json as JSON.");
                        }
                    }
                    Err(e) => println!("Could not read config.json: {}", e),
                }
            } else if new_yaml.exists() {
                println!("config.yaml already exists. No migration needed.");
            } else {
                println!("No legacy config.json found. Nothing to migrate.");
            }
        }
        Some(other) => {
            println!("Unknown config action: '{}'.", other);
            println!("Available: show, get, set, path, env-path, check, edit, migrate");
        }
    }
    Ok(())
}

/// Config/state root shared by CLI, `hermes gateway`, cron, and `webhooks.json`.
fn hermes_state_root(cli: &Cli) -> PathBuf {
    state_dir(cli.config_dir.as_deref().map(Path::new))
}

fn gateway_pid_path_for_cli(cli: &Cli) -> PathBuf {
    gateway_pid_path_in(hermes_state_root(cli))
}

fn serve_pid_path_for_cli(cli: &Cli) -> PathBuf {
    hermes_state_root(cli).join("serve.pid")
}

fn read_gateway_pid(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

#[cfg(unix)]
fn gateway_pid_is_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
fn gateway_pid_is_alive(_pid: u32) -> bool {
    false
}

#[cfg(unix)]
fn gateway_pid_terminate(pid: u32) -> std::io::Result<()> {
    let r = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    if r == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(unix))]
fn gateway_pid_terminate(_pid: u32) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "gateway stop is not supported on this platform",
    ))
}

/// Handle `hermes gateway [action]`.
async fn run_gateway(cli: Cli, action: Option<String>) -> Result<(), AgentError> {
    let config =
        load_config(cli.config_dir.as_deref()).map_err(|e| AgentError::Config(e.to_string()))?;

    match action.as_deref() {
        Some("setup") => {
            run_gateway_setup(&cli).await?;
        }
        None | Some("start") => {
            println!("Starting Hermes Gateway...");
            println!("Gateway start in engine mode uses `hermes serve` pipeline.");
            let pid_path = gateway_pid_path_for_cli(&cli);
            if let Some(pid) = read_gateway_pid(&pid_path) {
                if gateway_pid_is_alive(pid) {
                    println!(
                        "Gateway already appears to be running (PID {}, file {}). Stop it first or remove a stale PID file.",
                        pid,
                        pid_path.display()
                    );
                    return Ok(());
                }
                let _ = std::fs::remove_file(&pid_path);
            }
            if let Some(parent) = pid_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&pid_path, format!("{}\n", std::process::id()))
                .map_err(|e| AgentError::Io(format!("failed to write PID file: {}", e)))?;

            println!("RuntimeBuilder is not enabled in engine-only mode.");
            println!("Use `hermes serve start` to run API + gateway together.");
            let res = Ok(());
            let _ = std::fs::remove_file(&pid_path);
            return res;
        }
        Some("status") => {
            let pid_path = gateway_pid_path_for_cli(&cli);
            match std::fs::read_to_string(&pid_path) {
                Ok(raw) => match raw.trim().parse::<u32>() {
                    Ok(pid) if gateway_pid_is_alive(pid) => {
                        println!(
                            "Gateway status: running (PID {}, file {})",
                            pid,
                            pid_path.display()
                        );
                    }
                    Ok(pid) => {
                        println!(
                            "Gateway status: not running (stale PID {} in {})",
                            pid,
                            pid_path.display()
                        );
                    }
                    Err(_) => {
                        println!("Gateway status: invalid PID file at {}", pid_path.display());
                    }
                },
                Err(_) => {
                    println!("Gateway status: not running (no PID file; start with `hermes gateway start`)");
                }
            }
        }
        Some("stop") => {
            let pid_path = gateway_pid_path_for_cli(&cli);
            let Some(pid) = read_gateway_pid(&pid_path) else {
                println!("Gateway stop: no PID file (nothing to stop).");
                return Ok(());
            };
            if !gateway_pid_is_alive(pid) {
                let _ = std::fs::remove_file(&pid_path);
                println!(
                    "Gateway stop: process {} not running; removed stale PID file {}.",
                    pid,
                    pid_path.display()
                );
                return Ok(());
            }
            match gateway_pid_terminate(pid) {
                Ok(()) => {
                    println!("Sent SIGTERM to gateway PID {}.", pid);
                    let _ = std::fs::remove_file(&pid_path);
                    println!("Removed {}.", pid_path.display());
                }
                Err(e) => println!("Gateway stop: failed to signal PID {}: {}", pid, e),
            }
        }
        Some(other) => {
            println!(
                "Unknown gateway action: {}. Use 'start', 'stop', or 'status'.",
                other
            );
        }
    }
    Ok(())
}

async fn prompt_yes_no(question: &str, default_yes: bool) -> Result<bool, AgentError> {
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    let ans = prompt_line(format!("{question} {hint}: ")).await?;
    if ans.trim().is_empty() {
        return Ok(default_yes);
    }
    let v = ans.trim().to_ascii_lowercase();
    Ok(matches!(v.as_str(), "y" | "yes" | "1" | "true" | "on"))
}

fn parse_csv_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn normalize_gateway_platform_key(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "telegram" | "tg" => Some("telegram"),
        "weixin" | "wechat" | "wx" => Some("weixin"),
        "qq" | "qqbot" => Some("qqbot"),
        "discord" => Some("discord"),
        "slack" => Some("slack"),
        "matrix" => Some("matrix"),
        "mattermost" | "mm" => Some("mattermost"),
        "signal" => Some("signal"),
        "whatsapp" | "wa" => Some("whatsapp"),
        "dingtalk" => Some("dingtalk"),
        "feishu" | "lark" => Some("feishu"),
        "wecom" => Some("wecom"),
        "wecom_callback" | "wecom-callback" => Some("wecom_callback"),
        "bluebubbles" | "imessage" => Some("bluebubbles"),
        "email" => Some("email"),
        "sms" => Some("sms"),
        "homeassistant" | "ha" => Some("homeassistant"),
        "webhook" => Some("webhook"),
        "api_server" | "api-server" | "api" => Some("api_server"),
        _ => None,
    }
}

fn enabled_flag(platform: Option<&PlatformConfig>) -> &'static str {
    if platform.map(|p| p.enabled).unwrap_or(false) {
        "enabled"
    } else {
        "disabled"
    }
}

fn set_extra_string_if_nonempty(platform: &mut PlatformConfig, key: &str, value: &str) {
    let v = value.trim();
    if !v.is_empty() {
        platform
            .extra
            .insert(key.to_string(), serde_json::Value::String(v.to_string()));
    }
}

async fn configure_platform_basic_prompts(
    disk: &mut hermes_config::GatewayConfig,
    key: &str,
) -> Result<(), AgentError> {
    let p = disk
        .platforms
        .entry(key.to_string())
        .or_insert_with(PlatformConfig::default);
    p.enabled = true;

    match key {
        "discord" => {
            let token = prompt_line("Discord bot token: ").await?;
            if !token.trim().is_empty() {
                p.token = Some(token.trim().to_string());
            }
            let app_id = prompt_line("Discord application_id (optional): ").await?;
            set_extra_string_if_nonempty(p, "application_id", &app_id);
            let allowed =
                prompt_line("Discord allowed users (comma-separated, optional): ").await?;
            if !allowed.trim().is_empty() {
                p.allowed_users = parse_csv_list(&allowed);
            }
            let home = prompt_line("Discord home channel (optional): ").await?;
            if !home.trim().is_empty() {
                p.home_channel = Some(home.trim().to_string());
            }
        }
        "slack" => {
            let token = prompt_line("Slack bot token (xoxb-...): ").await?;
            if !token.trim().is_empty() {
                p.token = Some(token.trim().to_string());
            }
            let app_token = prompt_line("Slack app token (xapp-..., optional): ").await?;
            set_extra_string_if_nonempty(p, "app_token", &app_token);
            let socket_mode = prompt_yes_no("Slack use socket_mode?", true).await?;
            p.extra.insert(
                "socket_mode".to_string(),
                serde_json::Value::Bool(socket_mode),
            );
        }
        "matrix" => {
            let homeserver =
                prompt_line("Matrix homeserver_url (e.g. https://matrix.org): ").await?;
            set_extra_string_if_nonempty(p, "homeserver_url", &homeserver);
            let user_id = prompt_line("Matrix user_id (e.g. @bot:matrix.org): ").await?;
            set_extra_string_if_nonempty(p, "user_id", &user_id);
            let token = prompt_line("Matrix access token: ").await?;
            if !token.trim().is_empty() {
                p.token = Some(token.trim().to_string());
            }
            let room = prompt_line("Matrix home room_id (optional): ").await?;
            set_extra_string_if_nonempty(p, "room_id", &room);
        }
        "mattermost" => {
            let server_url = prompt_line("Mattermost server_url: ").await?;
            set_extra_string_if_nonempty(p, "server_url", &server_url);
            let token = prompt_line("Mattermost bot token: ").await?;
            if !token.trim().is_empty() {
                p.token = Some(token.trim().to_string());
            }
            let team_id = prompt_line("Mattermost team_id (optional): ").await?;
            set_extra_string_if_nonempty(p, "team_id", &team_id);
            let home = prompt_line("Mattermost home channel (optional): ").await?;
            if !home.trim().is_empty() {
                p.home_channel = Some(home.trim().to_string());
            }
        }
        "signal" => {
            let account = prompt_line("Signal phone_number/account (e.g. +15551234567): ").await?;
            set_extra_string_if_nonempty(p, "phone_number", &account);
            let api_url = prompt_line("Signal api_url (default http://localhost:8080): ").await?;
            set_extra_string_if_nonempty(p, "api_url", &api_url);
        }
        "whatsapp" => {
            let token = prompt_line("WhatsApp Cloud API token: ").await?;
            if !token.trim().is_empty() {
                p.token = Some(token.trim().to_string());
            }
            let phone_id = prompt_line("WhatsApp phone_number_id: ").await?;
            set_extra_string_if_nonempty(p, "phone_number_id", &phone_id);
            let verify = prompt_line("WhatsApp verify_token (optional): ").await?;
            set_extra_string_if_nonempty(p, "verify_token", &verify);
            let home = prompt_line("WhatsApp home channel (optional): ").await?;
            if !home.trim().is_empty() {
                p.home_channel = Some(home.trim().to_string());
            }
        }
        "dingtalk" => {
            let client_id = prompt_line("DingTalk client_id/appkey: ").await?;
            set_extra_string_if_nonempty(p, "client_id", &client_id);
            let client_secret = prompt_line("DingTalk client_secret: ").await?;
            set_extra_string_if_nonempty(p, "client_secret", &client_secret);
        }
        "feishu" => {
            let app_id = prompt_line("Feishu/Lark app_id: ").await?;
            set_extra_string_if_nonempty(p, "app_id", &app_id);
            let app_secret = prompt_line("Feishu/Lark app_secret: ").await?;
            set_extra_string_if_nonempty(p, "app_secret", &app_secret);
            let verify = prompt_line("Feishu verification_token (optional): ").await?;
            set_extra_string_if_nonempty(p, "verification_token", &verify);
            let encrypt_key = prompt_line("Feishu encrypt_key (optional): ").await?;
            set_extra_string_if_nonempty(p, "encrypt_key", &encrypt_key);
        }
        "wecom" => {
            let corp_id = prompt_line("WeCom corp_id: ").await?;
            set_extra_string_if_nonempty(p, "corp_id", &corp_id);
            let agent_id = prompt_line("WeCom agent_id: ").await?;
            set_extra_string_if_nonempty(p, "agent_id", &agent_id);
            let secret = prompt_line("WeCom secret: ").await?;
            set_extra_string_if_nonempty(p, "secret", &secret);
        }
        "wecom_callback" => {
            let corp_id = prompt_line("WeCom callback corp_id: ").await?;
            set_extra_string_if_nonempty(p, "corp_id", &corp_id);
            let corp_secret = prompt_line("WeCom callback corp_secret: ").await?;
            set_extra_string_if_nonempty(p, "corp_secret", &corp_secret);
            let agent_id = prompt_line("WeCom callback agent_id: ").await?;
            set_extra_string_if_nonempty(p, "agent_id", &agent_id);
            let token = prompt_line("WeCom callback token: ").await?;
            set_extra_string_if_nonempty(p, "token", &token);
            let aes = prompt_line("WeCom callback encoding_aes_key: ").await?;
            set_extra_string_if_nonempty(p, "encoding_aes_key", &aes);
            let host = prompt_line("WeCom callback host (default 0.0.0.0): ").await?;
            set_extra_string_if_nonempty(p, "host", &host);
            let port = prompt_line("WeCom callback port (default 8645): ").await?;
            if let Ok(v) = port.trim().parse::<u16>() {
                p.extra
                    .insert("port".to_string(), serde_json::Value::from(v));
            }
            let path = prompt_line("WeCom callback path (default /wecom/callback): ").await?;
            set_extra_string_if_nonempty(p, "path", &path);
        }
        "qqbot" => {
            let app_id = prompt_line("QQBot app_id: ").await?;
            set_extra_string_if_nonempty(p, "app_id", &app_id);
            let secret = prompt_line("QQBot client_secret: ").await?;
            set_extra_string_if_nonempty(p, "client_secret", &secret);
            let markdown = prompt_yes_no("QQBot markdown_support?", true).await?;
            p.extra.insert(
                "markdown_support".to_string(),
                serde_json::Value::Bool(markdown),
            );
        }
        "bluebubbles" => {
            let server_url = prompt_line("BlueBubbles server_url: ").await?;
            set_extra_string_if_nonempty(p, "server_url", &server_url);
            let password = prompt_line("BlueBubbles password: ").await?;
            set_extra_string_if_nonempty(p, "password", &password);
        }
        "email" => {
            let username = prompt_line("Email username/address: ").await?;
            set_extra_string_if_nonempty(p, "username", &username);
            let password = prompt_line("Email password/app password: ").await?;
            set_extra_string_if_nonempty(p, "password", &password);
            let imap_host = prompt_line("Email imap_host: ").await?;
            set_extra_string_if_nonempty(p, "imap_host", &imap_host);
            let smtp_host = prompt_line("Email smtp_host: ").await?;
            set_extra_string_if_nonempty(p, "smtp_host", &smtp_host);
            let imap_port = prompt_line("Email imap_port (default 993): ").await?;
            if let Ok(v) = imap_port.trim().parse::<u16>() {
                p.extra
                    .insert("imap_port".to_string(), serde_json::Value::from(v));
            }
            let smtp_port = prompt_line("Email smtp_port (default 587): ").await?;
            if let Ok(v) = smtp_port.trim().parse::<u16>() {
                p.extra
                    .insert("smtp_port".to_string(), serde_json::Value::from(v));
            }
        }
        "sms" => {
            let sid = prompt_line("Twilio account_sid: ").await?;
            set_extra_string_if_nonempty(p, "account_sid", &sid);
            let auth = prompt_line("Twilio auth_token: ").await?;
            set_extra_string_if_nonempty(p, "auth_token", &auth);
            let from = prompt_line("Twilio from_number (E.164): ").await?;
            set_extra_string_if_nonempty(p, "from_number", &from);
        }
        "homeassistant" => {
            let base_url =
                prompt_line("HomeAssistant base_url (e.g. http://127.0.0.1:8123): ").await?;
            set_extra_string_if_nonempty(p, "base_url", &base_url);
            let token = prompt_line("HomeAssistant long_lived_token: ").await?;
            if !token.trim().is_empty() {
                p.token = Some(token.trim().to_string());
            }
            let webhook_id = prompt_line("HomeAssistant webhook_id (optional): ").await?;
            set_extra_string_if_nonempty(p, "webhook_id", &webhook_id);
        }
        "webhook" => {
            let secret = prompt_line("Webhook secret: ").await?;
            set_extra_string_if_nonempty(p, "secret", &secret);
            let port = prompt_line("Webhook port (default 9000): ").await?;
            if let Ok(v) = port.trim().parse::<u16>() {
                p.extra
                    .insert("port".to_string(), serde_json::Value::from(v));
            }
            let path = prompt_line("Webhook path (default /webhook): ").await?;
            set_extra_string_if_nonempty(p, "path", &path);
        }
        "api_server" => {
            let host = prompt_line("API server host (default 0.0.0.0): ").await?;
            set_extra_string_if_nonempty(p, "host", &host);
            let port = prompt_line("API server port (default 8090): ").await?;
            if let Ok(v) = port.trim().parse::<u16>() {
                p.extra
                    .insert("port".to_string(), serde_json::Value::from(v));
            }
            let token = prompt_line("API server auth_token (optional): ").await?;
            set_extra_string_if_nonempty(p, "auth_token", &token);
        }
        _ => {}
    }
    Ok(())
}

async fn run_gateway_setup(cli: &Cli) -> Result<(), AgentError> {
    println!("Gateway setup wizard");
    println!("--------------------");
    let cfg_path = hermes_state_root(cli).join("config.yaml");
    let mut disk =
        load_user_config_file(&cfg_path).map_err(|e| AgentError::Config(e.to_string()))?;
    println!("This wizard configures messaging platforms in config.yaml.");
    println!("Current platform status:");
    for (k, label) in [
        ("weixin", "Weixin"),
        ("qqbot", "QQBot"),
        ("telegram", "Telegram"),
        ("discord", "Discord"),
        ("slack", "Slack"),
        ("matrix", "Matrix"),
        ("mattermost", "Mattermost"),
        ("whatsapp", "WhatsApp"),
        ("signal", "Signal"),
        ("dingtalk", "DingTalk"),
        ("feishu", "Feishu"),
        ("wecom", "WeCom"),
        ("wecom_callback", "WeCom Callback"),
        ("bluebubbles", "BlueBubbles"),
        ("email", "Email"),
        ("sms", "SMS"),
        ("homeassistant", "HomeAssistant"),
        ("webhook", "Webhook"),
        ("api_server", "API Server"),
    ] {
        println!("  - {:<13} {}", label, enabled_flag(disk.platforms.get(k)));
    }
    println!();
    println!("Examples: weixin,telegram   or   discord,slack,matrix");
    let raw = prompt_line(
        "Platforms to configure (comma-separated, empty defaults to weixin,telegram): ",
    )
    .await?;
    let mut selected: Vec<String> = if raw.trim().is_empty() {
        vec!["weixin".to_string(), "telegram".to_string()]
    } else {
        parse_csv_list(&raw)
            .into_iter()
            .filter_map(|k| normalize_gateway_platform_key(&k).map(|v| v.to_string()))
            .collect()
    };
    selected.sort();
    selected.dedup();
    if selected.is_empty() {
        println!("No valid platforms selected.");
        return Ok(());
    }

    for key in selected {
        println!();
        println!("Configuring {}...", key);
        match key.as_str() {
            "weixin" => {
                run_auth(
                    cli.clone(),
                    Some("login".to_string()),
                    Some("weixin".to_string()),
                    true,
                )
                .await?;
                disk = load_user_config_file(&cfg_path)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                let wx = disk
                    .platforms
                    .entry("weixin".to_string())
                    .or_insert_with(PlatformConfig::default);
                wx.enabled = true;
                println!("Direct message policy: 1)pairing 2)open 3)allowlist 4)disabled");
                let dm_choice = prompt_line("Choose [1-4] (default 1): ").await?;
                match dm_choice.trim() {
                    "2" => {
                        wx.extra
                            .insert("dm_policy".to_string(), serde_json::json!("open"));
                        wx.extra
                            .insert("allow_from".to_string(), serde_json::json!([]));
                    }
                    "3" => {
                        let ids = parse_csv_list(
                            &prompt_line("Allowed Weixin user IDs (comma-separated): ").await?,
                        );
                        wx.extra
                            .insert("dm_policy".to_string(), serde_json::json!("allowlist"));
                        wx.extra.insert(
                            "allow_from".to_string(),
                            serde_json::Value::Array(
                                ids.into_iter().map(serde_json::Value::String).collect(),
                            ),
                        );
                    }
                    "4" => {
                        wx.extra
                            .insert("dm_policy".to_string(), serde_json::json!("disabled"));
                        wx.extra
                            .insert("allow_from".to_string(), serde_json::json!([]));
                    }
                    _ => {
                        wx.extra
                            .insert("dm_policy".to_string(), serde_json::json!("pairing"));
                        wx.extra
                            .insert("allow_from".to_string(), serde_json::json!([]));
                    }
                }
                println!("Group policy: 1)disabled 2)open 3)allowlist");
                let group_choice = prompt_line("Choose [1-3] (default 1): ").await?;
                match group_choice.trim() {
                    "2" => {
                        wx.extra
                            .insert("group_policy".to_string(), serde_json::json!("open"));
                        wx.extra
                            .insert("group_allow_from".to_string(), serde_json::json!([]));
                    }
                    "3" => {
                        let ids = parse_csv_list(
                            &prompt_line("Allowed Weixin group IDs (comma-separated): ").await?,
                        );
                        wx.extra
                            .insert("group_policy".to_string(), serde_json::json!("allowlist"));
                        wx.extra.insert(
                            "group_allow_from".to_string(),
                            serde_json::Value::Array(
                                ids.into_iter().map(serde_json::Value::String).collect(),
                            ),
                        );
                    }
                    _ => {
                        wx.extra
                            .insert("group_policy".to_string(), serde_json::json!("disabled"));
                        wx.extra
                            .insert("group_allow_from".to_string(), serde_json::json!([]));
                    }
                }
                let home = prompt_line("Weixin home channel (optional): ").await?;
                if !home.trim().is_empty() {
                    wx.home_channel = Some(home.trim().to_string());
                }
            }
            "telegram" => {
                run_auth(
                    cli.clone(),
                    Some("login".to_string()),
                    Some("telegram".to_string()),
                    false,
                )
                .await?;
                disk = load_user_config_file(&cfg_path)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                let tg = disk
                    .platforms
                    .entry("telegram".to_string())
                    .or_insert_with(PlatformConfig::default);
                tg.enabled = true;
                let polling = prompt_yes_no("Telegram use polling mode?", true).await?;
                tg.extra
                    .insert("polling".to_string(), serde_json::Value::Bool(polling));
                if !polling {
                    let webhook_url = prompt_line("Telegram webhook URL: ").await?;
                    if !webhook_url.trim().is_empty() {
                        tg.webhook_url = Some(webhook_url.trim().to_string());
                    }
                }
                let home = prompt_line("Telegram home channel (optional): ").await?;
                if !home.trim().is_empty() {
                    tg.home_channel = Some(home.trim().to_string());
                }
            }
            other => configure_platform_basic_prompts(&mut disk, other).await?,
        }
    }

    validate_config(&disk).map_err(|e| AgentError::Config(e.to_string()))?;
    save_config_yaml(&cfg_path, &disk).map_err(|e| AgentError::Config(e.to_string()))?;

    println!();
    println!("Gateway setup complete.");
    println!("Config saved: {}", cfg_path.display());
    println!("Next step: `hermes gateway start`");
    Ok(())
}

#[cfg(test)]
async fn register_gateway_adapters(
    config: &hermes_config::GatewayConfig,
    gateway: Arc<Gateway>,
    sidecar_tasks: &mut Vec<tokio::task::JoinHandle<()>>,
) -> Result<hermes_gateway::platform_registry::RegistrationSummary, AgentError> {
    let summary =
        hermes_gateway::platform_registry::register_platforms(&gateway, config, sidecar_tasks)
            .await?;

    #[cfg(test)]
    {
        use async_trait::async_trait;
        use hermes_core::{GatewayError, ParseMode};

        struct NoopAdapter {
            name: &'static str,
        }

        #[async_trait]
        impl PlatformAdapter for NoopAdapter {
            async fn start(&self) -> Result<(), GatewayError> {
                Ok(())
            }
            async fn stop(&self) -> Result<(), GatewayError> {
                Ok(())
            }
            async fn send_message(
                &self,
                _chat_id: &str,
                _text: &str,
                _parse_mode: Option<ParseMode>,
            ) -> Result<(), GatewayError> {
                Ok(())
            }
            async fn edit_message(
                &self,
                _chat_id: &str,
                _message_id: &str,
                _text: &str,
            ) -> Result<(), GatewayError> {
                Ok(())
            }
            async fn send_file(
                &self,
                _chat_id: &str,
                _file_path: &str,
                _caption: Option<&str>,
            ) -> Result<(), GatewayError> {
                Ok(())
            }
            fn is_running(&self) -> bool {
                true
            }
            fn platform_name(&self) -> &str {
                self.name
            }
        }

        if let Some(qqbot) = config.platforms.get("qqbot") {
            let ready = qqbot.enabled
                && hermes_config::extra_string(qqbot, "app_id").is_some()
                && hermes_config::extra_string(qqbot, "client_secret").is_some();
            if ready && !summary.registered.iter().any(|n| n == "qqbot") {
                gateway
                    .register_adapter("qqbot", Arc::new(NoopAdapter { name: "qqbot" }))
                    .await;
            }
        }

        if let Some(wecom_cb) = config.platforms.get("wecom_callback") {
            let ready = wecom_cb.enabled
                && hermes_config::extra_string(wecom_cb, "corp_id").is_some()
                && hermes_config::extra_string(wecom_cb, "corp_secret").is_some()
                && hermes_config::extra_string(wecom_cb, "agent_id").is_some()
                && platform_token_or_extra(wecom_cb).is_some()
                && hermes_config::extra_string(wecom_cb, "encoding_aes_key").is_some();
            if ready && !summary.registered.iter().any(|n| n == "wecom_callback") {
                gateway
                    .register_adapter(
                        "wecom_callback",
                        Arc::new(NoopAdapter {
                            name: "wecom_callback",
                        }),
                    )
                    .await;
            }
        }
    }

    Ok(summary)
}

/// Default auth provider: CLI arg, then `HERMES_AUTH_DEFAULT_PROVIDER`, then `openai`.
///
/// Set `HERMES_AUTH_DEFAULT_PROVIDER=telegram` if you primarily use the Telegram gateway.
fn resolve_auth_provider(provider: Option<String>) -> String {
    if let Some(raw) = provider.filter(|s| !s.trim().is_empty()) {
        return normalize_auth_provider(&raw);
    }

    if let Ok(pool) = std::env::var("HERMES_AUTH_PROVIDER_POOL") {
        for item in pool.split(',') {
            let item = item.trim();
            if !item.is_empty() {
                return normalize_auth_provider(item);
            }
        }
    }

    let raw = std::env::var("HERMES_AUTH_DEFAULT_PROVIDER")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "openai".to_string());
    normalize_auth_provider(&raw)
}

fn normalize_auth_provider(provider: &str) -> String {
    match provider.trim().to_ascii_lowercase().as_str() {
        "wechat" | "wx" => "weixin".to_string(),
        "qq" => "qqbot".to_string(),
        "tg" => "telegram".to_string(),
        "api-server" => "api_server".to_string(),
        "home-assistant" => "homeassistant".to_string(),
        "wecom-callback" => "wecom_callback".to_string(),
        "mm" => "mattermost".to_string(),
        "github-copilot" => "copilot".to_string(),
        other => other.to_string(),
    }
}

fn gateway_platform_provider_key(provider: &str) -> Option<&'static str> {
    match provider {
        "discord" => Some("discord"),
        "slack" => Some("slack"),
        "matrix" => Some("matrix"),
        "mattermost" => Some("mattermost"),
        "signal" => Some("signal"),
        "whatsapp" => Some("whatsapp"),
        "dingtalk" => Some("dingtalk"),
        "feishu" => Some("feishu"),
        "wecom" => Some("wecom"),
        "wecom_callback" => Some("wecom_callback"),
        "qqbot" | "qq" => Some("qqbot"),
        "bluebubbles" => Some("bluebubbles"),
        "email" => Some("email"),
        "sms" => Some("sms"),
        "homeassistant" => Some("homeassistant"),
        "webhook" => Some("webhook"),
        "api_server" => Some("api_server"),
        _ => None,
    }
}

fn is_weixin_provider(provider: &str) -> bool {
    provider == "weixin"
}

fn is_truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

async fn telegram_bot_token_from_env_or_prompt() -> Result<String, AgentError> {
    if let Ok(t) = std::env::var("TELEGRAM_BOT_TOKEN") {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Ok(t);
        }
    }
    let line = tokio::task::spawn_blocking(|| {
        use std::io::{self, Write};
        print!("Enter Telegram bot token (from @BotFather): ");
        let _ = io::stdout().flush();
        let mut buf = String::new();
        io::stdin().read_line(&mut buf).map(|_| buf)
    })
    .await
    .map_err(|e| AgentError::Io(format!("telegram token prompt: {e}")))?
    .map_err(|e| AgentError::Io(format!("stdin: {e}")))?;
    let t = line.trim().to_string();
    if t.is_empty() {
        return Err(AgentError::Config(
            "Telegram bot token cannot be empty (set TELEGRAM_BOT_TOKEN or paste token)".into(),
        ));
    }
    Ok(t)
}

async fn weixin_account_id_from_env_or_prompt() -> Result<String, AgentError> {
    if let Ok(v) = std::env::var("WEIXIN_ACCOUNT_ID") {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Ok(v);
        }
    }
    let line = tokio::task::spawn_blocking(|| {
        use std::io::{self, Write};
        print!("Enter Weixin account_id (个人号 wxid/账号标识): ");
        let _ = io::stdout().flush();
        let mut buf = String::new();
        io::stdin().read_line(&mut buf).map(|_| buf)
    })
    .await
    .map_err(|e| AgentError::Io(format!("weixin account_id prompt: {e}")))?
    .map_err(|e| AgentError::Io(format!("stdin: {e}")))?;
    let v = line.trim().to_string();
    if v.is_empty() {
        return Err(AgentError::Config(
            "Weixin account_id cannot be empty (set WEIXIN_ACCOUNT_ID or input manually)".into(),
        ));
    }
    Ok(v)
}

fn weixin_account_file_path(account_id: &str) -> PathBuf {
    hermes_home()
        .join("weixin")
        .join("accounts")
        .join(format!("{account_id}.json"))
}

fn load_persisted_weixin_token(account_id: &str) -> Option<String> {
    let p = weixin_account_file_path(account_id);
    let s = std::fs::read_to_string(p).ok()?;
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    v.get("token")
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(String::from)
}

fn save_persisted_weixin_account(
    account_id: &str,
    token: &str,
    base_url: Option<&str>,
    user_id: Option<&str>,
) -> Result<(), AgentError> {
    let p = weixin_account_file_path(account_id);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AgentError::Io(format!("create weixin account dir: {e}")))?;
    }
    let payload = serde_json::json!({
        "token": token,
        "base_url": base_url.unwrap_or(""),
        "user_id": user_id.unwrap_or(""),
        "saved_at": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(&p, payload.to_string())
        .map_err(|e| AgentError::Io(format!("write weixin account file {}: {e}", p.display())))?;
    Ok(())
}

async fn weixin_token_from_env_or_prompt(account_id: &str) -> Result<String, AgentError> {
    if let Ok(v) = std::env::var("WEIXIN_TOKEN") {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Ok(v);
        }
    }
    if let Some(v) = load_persisted_weixin_token(account_id) {
        return Ok(v);
    }
    let line = tokio::task::spawn_blocking(|| {
        use std::io::{self, Write};
        print!("Enter Weixin iLink token (WEIXIN_TOKEN): ");
        let _ = io::stdout().flush();
        let mut buf = String::new();
        io::stdin().read_line(&mut buf).map(|_| buf)
    })
    .await
    .map_err(|e| AgentError::Io(format!("weixin token prompt: {e}")))?
    .map_err(|e| AgentError::Io(format!("stdin: {e}")))?;
    let v = line.trim().to_string();
    if v.is_empty() {
        return Err(AgentError::Config(
            "Weixin token cannot be empty (set WEIXIN_TOKEN / saved account file / input manually)"
                .into(),
        ));
    }
    Ok(v)
}

fn weixin_login_base_url_from_disk(disk: &hermes_config::GatewayConfig) -> String {
    if let Some(wx) = disk.platforms.get("weixin") {
        if let Some(v) = wx.extra.get("base_url").and_then(|v| v.as_str()) {
            let s = v.trim();
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    if let Ok(v) = std::env::var("WEIXIN_BASE_URL") {
        let s = v.trim();
        if !s.is_empty() {
            return s.to_string();
        }
    }
    "https://ilinkai.weixin.qq.com".to_string()
}

fn weixin_login_endpoints_from_disk(disk: &hermes_config::GatewayConfig) -> (String, String) {
    let mut start_ep = "ilink/bot/get_bot_qrcode".to_string();
    let mut poll_ep = "ilink/bot/get_qrcode_status".to_string();
    if let Some(wx) = disk.platforms.get("weixin") {
        if let Some(v) = wx
            .extra
            .get("qr_get_bot_qrcode_endpoint")
            .or_else(|| wx.extra.get("qr_start_endpoint"))
            .and_then(|v| v.as_str())
        {
            let s = v.trim();
            if !s.is_empty() {
                start_ep = s.to_string();
            }
        }
        if let Some(v) = wx
            .extra
            .get("qr_get_qrcode_status_endpoint")
            .or_else(|| wx.extra.get("qr_poll_endpoint"))
            .and_then(|v| v.as_str())
        {
            let s = v.trim();
            if !s.is_empty() {
                poll_ep = s.to_string();
            }
        }
    }
    (start_ep, poll_ep)
}

fn weixin_extract_string(v: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(s) = v.get(*key).and_then(|x| x.as_str()) {
            let s = s.trim();
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn render_qr_to_terminal(data: &str) {
    let len = data.len();
    let side = (len as f64).sqrt().ceil() as usize;
    if side == 0 {
        println!("(empty QR data)");
        return;
    }
    let bytes = data.as_bytes();
    let is_dark = |row: usize, col: usize| -> bool {
        let idx = row * side + col;
        if idx < bytes.len() {
            bytes[idx] % 2 == 1
        } else {
            false
        }
    };
    let mut row = 0;
    while row < side {
        let mut line = String::new();
        for col in 0..side {
            let top = is_dark(row, col);
            let bottom = if row + 1 < side {
                is_dark(row + 1, col)
            } else {
                false
            };
            line.push(match (top, bottom) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        println!("  {}", line);
        row += 2;
    }
}

async fn weixin_qr_login_flow(
    base_url: &str,
    start_ep: &str,
    poll_ep: &str,
    _account_id_hint: Option<&str>,
) -> Result<(String, String, String, String), AgentError> {
    let initial_base = base_url.trim_end_matches('/').to_string();
    let client = reqwest::Client::new();
    async fn fetch_weixin_qr(
        client: &reqwest::Client,
        base: &str,
        start_ep: &str,
    ) -> Result<serde_json::Value, AgentError> {
        let url = format!(
            "{}/{}",
            base.trim_end_matches('/'),
            start_ep.trim_start_matches('/')
        );
        let resp = client
            .get(&url)
            .query(&[("bot_type", "3")])
            .timeout(std::time::Duration::from_secs(35))
            .send()
            .await
            .map_err(|e| AgentError::Io(format!("weixin qr get_bot_qrcode request: {e}")))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AgentError::Config(format!(
                "weixin qr get_bot_qrcode failed ({}): {}",
                status, body
            )));
        }
        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| AgentError::Io(format!("weixin qr get_bot_qrcode parse: {e}")))
    }

    let mut current_base = initial_base.clone();
    let mut qr_json = fetch_weixin_qr(&client, &current_base, start_ep).await?;
    let mut qrcode_value = weixin_extract_string(&qr_json, &["qrcode"])
        .ok_or_else(|| AgentError::Config("weixin qr response missing qrcode".to_string()))?;
    let mut qrcode_url =
        weixin_extract_string(&qr_json, &["qrcode_img_content"]).unwrap_or_default();
    let qr_scan_data = if !qrcode_url.trim().is_empty() {
        qrcode_url.clone()
    } else {
        qrcode_value.clone()
    };
    println!();
    if !qrcode_url.trim().is_empty() {
        println!("{}", qrcode_url);
    }
    render_qr_to_terminal(&qr_scan_data);
    println!();
    println!("请使用微信扫描二维码，并在手机端确认登录。");

    let poll_interval = std::time::Duration::from_secs(1);
    let timeout = std::time::Duration::from_secs(480);
    let started = std::time::Instant::now();
    let mut refresh_count = 0u8;
    loop {
        if started.elapsed() >= timeout {
            return Err(AgentError::Config(
                "weixin qr login timed out after 480s".to_string(),
            ));
        }
        tokio::time::sleep(poll_interval).await;
        let poll_url = format!(
            "{}/{}",
            current_base.trim_end_matches('/'),
            poll_ep.trim_start_matches('/')
        );
        let poll_resp = match client
            .get(&poll_url)
            .query(&[("qrcode", qrcode_value.as_str())])
            .timeout(std::time::Duration::from_secs(35))
            .send()
            .await
        {
            Ok(v) => v,
            Err(_) => continue,
        };
        if !poll_resp.status().is_success() {
            continue;
        }
        let poll_json: serde_json::Value = match poll_resp.json().await {
            Ok(v) => v,
            Err(_) => continue,
        };
        let status = weixin_extract_string(&poll_json, &["status"])
            .unwrap_or_else(|| "wait".to_string())
            .to_ascii_lowercase();
        match status.as_str() {
            "wait" => {}
            "scaned" => {
                println!("已扫码，请在微信里确认...");
            }
            "scaned_but_redirect" => {
                if let Some(redirect_host) =
                    weixin_extract_string(&poll_json, &["redirect_host"]).filter(|s| !s.is_empty())
                {
                    current_base = format!("https://{}", redirect_host);
                }
            }
            "expired" => {
                refresh_count = refresh_count.saturating_add(1);
                if refresh_count > 3 {
                    return Err(AgentError::Config(
                        "weixin qr expired too many times".to_string(),
                    ));
                }
                println!("二维码已过期，正在刷新... ({}/3)", refresh_count);
                qr_json = fetch_weixin_qr(&client, &initial_base, start_ep).await?;
                qrcode_value = weixin_extract_string(&qr_json, &["qrcode"]).ok_or_else(|| {
                    AgentError::Config("weixin qr refresh missing qrcode".to_string())
                })?;
                qrcode_url =
                    weixin_extract_string(&qr_json, &["qrcode_img_content"]).unwrap_or_default();
                let refreshed_qr = if !qrcode_url.trim().is_empty() {
                    qrcode_url.clone()
                } else {
                    qrcode_value.clone()
                };
                if !qrcode_url.trim().is_empty() {
                    println!("{}", qrcode_url);
                }
                render_qr_to_terminal(&refreshed_qr);
            }
            "confirmed" => {
                let account_id = weixin_extract_string(&poll_json, &["ilink_bot_id", "account_id"])
                    .unwrap_or_default();
                let token =
                    weixin_extract_string(&poll_json, &["bot_token", "token"]).unwrap_or_default();
                let resolved_base_url =
                    weixin_extract_string(&poll_json, &["baseurl"]).unwrap_or(initial_base.clone());
                let user_id = weixin_extract_string(&poll_json, &["ilink_user_id", "user_id"])
                    .unwrap_or_default();
                if account_id.trim().is_empty() || token.trim().is_empty() {
                    return Err(AgentError::Config(
                        "weixin qr confirmed but payload missing ilink_bot_id/bot_token"
                            .to_string(),
                    ));
                }
                return Ok((account_id, token, resolved_base_url, user_id));
            }
            _ => {}
        }
    }
}

async fn print_auth_status_matrix(cli: &Cli, manager: &AuthManager) -> Result<(), AgentError> {
    let cfg_path = hermes_state_root(cli).join("config.yaml");
    let disk = load_user_config_file(&cfg_path).map_err(|e| AgentError::Config(e.to_string()))?;

    println!("Auth status matrix:");
    println!("-------------------");

    for provider in ["openai", "anthropic", "openrouter", "copilot"] {
        let env_present = provider_api_key_from_env(provider).is_some()
            || (provider == "copilot"
                && std::env::var("GITHUB_COPILOT_TOKEN")
                    .ok()
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false));
        let store_present = manager.get_access_token(provider).await?.is_some();
        let (present, source) = if env_present {
            (true, "env")
        } else if store_present {
            (true, "token_store")
        } else {
            (false, "none")
        };
        println!("  - {:<16} present={} source={}", provider, present, source);
    }

    for provider in [
        "telegram",
        "weixin",
        "discord",
        "slack",
        "qqbot",
        "wecom_callback",
    ] {
        let (enabled, cfg_token) = disk
            .platforms
            .get(provider)
            .map(|p| (p.enabled, platform_token_or_extra(p).is_some()))
            .unwrap_or((false, false));
        let env_present = match provider {
            "telegram" => std::env::var("TELEGRAM_BOT_TOKEN")
                .ok()
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false),
            "weixin" => std::env::var("WEIXIN_TOKEN")
                .ok()
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false),
            "qqbot" => {
                std::env::var("QQ_APP_ID")
                    .ok()
                    .map(|v| !v.trim().is_empty())
                    .unwrap_or(false)
                    && std::env::var("QQ_CLIENT_SECRET")
                        .ok()
                        .map(|v| !v.trim().is_empty())
                        .unwrap_or(false)
            }
            _ => false,
        };
        let (present, source) = if env_present {
            (true, "env")
        } else if cfg_token {
            (true, "config")
        } else {
            (false, "none")
        };
        println!(
            "  - {:<16} present={} source={} enabled={}",
            provider, present, source, enabled
        );
    }
    Ok(())
}

async fn run_auth(
    cli: Cli,
    action: Option<String>,
    provider: Option<String>,
    qr: bool,
) -> Result<(), AgentError> {
    let provider = resolve_auth_provider(provider);
    let auth_store_path = hermes_home().join("auth").join("tokens.json");
    let token_store = FileTokenStore::new(auth_store_path).await?;
    let manager = AuthManager::new(token_store.clone());
    match action.as_deref().unwrap_or("status") {
        "login" => {
            if provider == "telegram" {
                let token = telegram_bot_token_from_env_or_prompt().await?;
                let cfg_path = hermes_state_root(&cli).join("config.yaml");
                let mut disk = load_user_config_file(&cfg_path)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                let tg = disk
                    .platforms
                    .entry("telegram".to_string())
                    .or_insert_with(PlatformConfig::default);
                tg.token = Some(token);
                tg.enabled = true;
                validate_config(&disk).map_err(|e| AgentError::Config(e.to_string()))?;
                save_config_yaml(&cfg_path, &disk)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                println!(
                    "Telegram: token saved and platform enabled in {}",
                    cfg_path.display()
                );
                return Ok(());
            }
            if is_weixin_provider(&provider) {
                let cfg_path = hermes_state_root(&cli).join("config.yaml");
                let mut disk = load_user_config_file(&cfg_path)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                let qr_preferred = qr
                    || std::env::var("HERMES_WEIXIN_QR_LOGIN")
                        .ok()
                        .map(|v| is_truthy(&v))
                        .unwrap_or(false);
                let mut account_id_opt = disk
                    .platforms
                    .get("weixin")
                    .and_then(|p| p.extra.get("account_id"))
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from);
                let (account_id, token, qr_base_url, qr_user_id) = if qr_preferred {
                    let base_url = weixin_login_base_url_from_disk(&disk);
                    let (start_ep, poll_ep) = weixin_login_endpoints_from_disk(&disk);
                    match weixin_qr_login_flow(
                        &base_url,
                        &start_ep,
                        &poll_ep,
                        account_id_opt.as_deref(),
                    )
                    .await
                    {
                        Ok(pair) => pair,
                        Err(e) => {
                            println!("Weixin QR 登录失败，将回退到手动 token 输入: {}", e);
                            let fallback_account_id = if let Some(v) = account_id_opt.take() {
                                v
                            } else {
                                weixin_account_id_from_env_or_prompt().await?
                            };
                            let fallback_token =
                                weixin_token_from_env_or_prompt(&fallback_account_id).await?;
                            (fallback_account_id, fallback_token, base_url, String::new())
                        }
                    }
                } else {
                    let manual_account_id = if let Some(v) = account_id_opt.take() {
                        v
                    } else {
                        weixin_account_id_from_env_or_prompt().await?
                    };
                    let manual_token = weixin_token_from_env_or_prompt(&manual_account_id).await?;
                    let base_url = weixin_login_base_url_from_disk(&disk);
                    (manual_account_id, manual_token, base_url, String::new())
                };
                let wx = disk
                    .platforms
                    .entry("weixin".to_string())
                    .or_insert_with(PlatformConfig::default);
                wx.enabled = true;
                wx.token = Some(token.clone());
                wx.extra.insert(
                    "account_id".to_string(),
                    serde_json::Value::String(account_id.clone()),
                );
                if !qr_base_url.trim().is_empty() {
                    wx.extra.insert(
                        "base_url".to_string(),
                        serde_json::Value::String(qr_base_url.clone()),
                    );
                }
                save_persisted_weixin_account(
                    &account_id,
                    &token,
                    Some(qr_base_url.as_str()),
                    Some(qr_user_id.as_str()),
                )?;
                validate_config(&disk).map_err(|e| AgentError::Config(e.to_string()))?;
                save_config_yaml(&cfg_path, &disk)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                println!(
                    "Weixin: account_id/token saved and platform enabled in {}",
                    cfg_path.display()
                );
                return Ok(());
            }
            if let Some(platform_key) = gateway_platform_provider_key(&provider) {
                let cfg_path = hermes_state_root(&cli).join("config.yaml");
                let mut disk = load_user_config_file(&cfg_path)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                configure_platform_basic_prompts(&mut disk, platform_key).await?;
                validate_config(&disk).map_err(|e| AgentError::Config(e.to_string()))?;
                save_config_yaml(&cfg_path, &disk)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                println!(
                    "{}: config updated and platform enabled in {}",
                    platform_key,
                    cfg_path.display()
                );
                return Ok(());
            }
            if provider == "copilot" || provider == "github-copilot" {
                let access_token = hermes_cli::copilot_auth::start_copilot_device_flow().await?;
                manager
                    .save_credential(OAuthCredential {
                        provider: "copilot".to_string(),
                        access_token,
                        refresh_token: None,
                        token_type: "bearer".to_string(),
                        scope: None,
                        expires_at: None,
                    })
                    .await?;
                println!("GitHub device login complete; credential saved as provider 'copilot'.");
                println!("Ensure GITHUB_COPILOT_TOKEN is set for the agent (see printed instructions above).");
                return Ok(());
            }

            let access_token = resolve_llm_login_token(&cli, &provider).await?;
            manager
                .save_credential(OAuthCredential {
                    provider: provider.clone(),
                    access_token,
                    refresh_token: None,
                    token_type: "bearer".to_string(),
                    scope: None,
                    expires_at: None,
                })
                .await?;
            let msg = hermes_cli::auth::login(&provider).await?;
            println!("{}", msg);
        }
        "logout" => {
            if provider == "telegram" {
                let cfg_path = hermes_state_root(&cli).join("config.yaml");
                let mut disk = load_user_config_file(&cfg_path)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                if let Some(tg) = disk.platforms.get_mut("telegram") {
                    tg.token = None;
                    tg.enabled = false;
                }
                validate_config(&disk).map_err(|e| AgentError::Config(e.to_string()))?;
                save_config_yaml(&cfg_path, &disk)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                println!(
                    "Telegram: token cleared and platform disabled in {}",
                    cfg_path.display()
                );
                return Ok(());
            }
            if is_weixin_provider(&provider) {
                let cfg_path = hermes_state_root(&cli).join("config.yaml");
                let mut disk = load_user_config_file(&cfg_path)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                if let Some(wx) = disk.platforms.get_mut("weixin") {
                    wx.token = None;
                    wx.enabled = false;
                }
                validate_config(&disk).map_err(|e| AgentError::Config(e.to_string()))?;
                save_config_yaml(&cfg_path, &disk)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                println!(
                    "Weixin: token cleared and platform disabled in {} (account file retained)",
                    cfg_path.display()
                );
                return Ok(());
            }
            if let Some(platform_key) = gateway_platform_provider_key(&provider) {
                let cfg_path = hermes_state_root(&cli).join("config.yaml");
                let mut disk = load_user_config_file(&cfg_path)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                if let Some(p) = disk.platforms.get_mut(platform_key) {
                    p.enabled = false;
                    p.token = None;
                }
                validate_config(&disk).map_err(|e| AgentError::Config(e.to_string()))?;
                save_config_yaml(&cfg_path, &disk)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                println!(
                    "{}: disabled and token cleared in {}",
                    platform_key,
                    cfg_path.display()
                );
                return Ok(());
            }
            let msg = hermes_cli::auth::logout(&provider).await?;
            token_store.remove(&provider).await?;
            println!("{} (removed credential for provider: {})", msg, provider);
        }
        _ => {
            if provider == "all" || provider == "*" {
                print_auth_status_matrix(&cli, &manager).await?;
                return Ok(());
            }
            if provider == "telegram" {
                let cfg_path = hermes_state_root(&cli).join("config.yaml");
                let disk = load_user_config_file(&cfg_path)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                let (has, en) = disk
                    .platforms
                    .get("telegram")
                    .map(|p| {
                        (
                            p.token
                                .as_deref()
                                .map(|t| !t.trim().is_empty())
                                .unwrap_or(false),
                            p.enabled,
                        )
                    })
                    .unwrap_or((false, false));
                println!(
                    "Telegram ({}): token_present={} enabled={}",
                    cfg_path.display(),
                    has,
                    en
                );
                return Ok(());
            }
            if is_weixin_provider(&provider) {
                let cfg_path = hermes_state_root(&cli).join("config.yaml");
                let disk = load_user_config_file(&cfg_path)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                let (account_id, has_cfg_token, enabled) = disk
                    .platforms
                    .get("weixin")
                    .map(|p| {
                        let account_id = p
                            .extra
                            .get("account_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let has_cfg_token = p
                            .token
                            .as_deref()
                            .map(|t| !t.trim().is_empty())
                            .unwrap_or(false);
                        (account_id, has_cfg_token, p.enabled)
                    })
                    .unwrap_or_else(|| ("".to_string(), false, false));
                let has_saved_token = if account_id.is_empty() {
                    false
                } else {
                    load_persisted_weixin_token(&account_id).is_some()
                };
                println!(
                    "Weixin ({}): account_id={} cfg_token_present={} saved_token_present={} enabled={}",
                    cfg_path.display(),
                    if account_id.is_empty() {
                        "(none)"
                    } else {
                        account_id.as_str()
                    },
                    has_cfg_token,
                    has_saved_token,
                    enabled
                );
                return Ok(());
            }
            if let Some(platform_key) = gateway_platform_provider_key(&provider) {
                let cfg_path = hermes_state_root(&cli).join("config.yaml");
                let disk = load_user_config_file(&cfg_path)
                    .map_err(|e| AgentError::Config(e.to_string()))?;
                let (enabled, token_present) = disk
                    .platforms
                    .get(platform_key)
                    .map(|p| (p.enabled, platform_token_or_extra(p).is_some()))
                    .unwrap_or((false, false));
                println!(
                    "{} ({}): credential_present={} enabled={}",
                    platform_key,
                    cfg_path.display(),
                    token_present,
                    enabled
                );
                return Ok(());
            }
            let env_present = provider_api_key_from_env(&provider).is_some();
            let store_present = manager.get_access_token(&provider).await?.is_some();
            let (has_token, source) = if env_present {
                (true, "env")
            } else if store_present {
                (true, "token_store")
            } else {
                (false, "none")
            };
            println!(
                "Auth status: provider='{}', credential_present={}, source={}",
                provider, has_token, source
            );
        }
    }
    Ok(())
}

fn cron_cli_error(e: CronError) -> AgentError {
    AgentError::Config(e.to_string())
}

async fn run_cron(
    cli: Cli,
    action: Option<String>,
    id: Option<String>,
    schedule: Option<String>,
    prompt: Option<String>,
) -> Result<(), AgentError> {
    let data_dir = hermes_state_root(&cli).join("cron");
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| AgentError::Io(format!("cron dir {}: {}", data_dir.display(), e)))?;
    let sched = cron_scheduler_for_data_dir(data_dir.clone());

    match action.as_deref().unwrap_or("list") {
        "list" => {
            let jobs = sched.list_jobs().await;
            if jobs.is_empty() {
                println!("(no cron jobs in {})", data_dir.display());
                return Ok(());
            }
            println!("Cron jobs ({}):", data_dir.display());
            for j in jobs {
                let snippet: String = j.prompt.chars().take(48).collect();
                println!(
                    "  {}  [{}]  {:?}  next_run={:?}  {}",
                    j.id, j.schedule, j.status, j.next_run, snippet
                );
            }
        }
        "create" => {
            let schedule = schedule.unwrap_or_else(|| "0 * * * *".to_string());
            let prompt = prompt
                .ok_or_else(|| AgentError::Config("cron create: use --prompt \"...\"".into()))?;
            let job = hermes_cron::CronJob::new(schedule, prompt);
            let jid = sched.create_job(job).await.map_err(cron_cli_error)?;
            println!(
                "Created cron job id={} (persisted under {})",
                jid,
                data_dir.display()
            );
        }
        "delete" | "pause" | "resume" | "run" | "history" => {
            let act = action.as_deref().unwrap_or("cron");
            let jid = id
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AgentError::Config(format!("{}: use --id <job-id>", act)))?;
            match act {
                "delete" => {
                    sched.remove_job(&jid).await.map_err(cron_cli_error)?;
                    println!("Deleted job {}", jid);
                }
                "pause" => {
                    sched.pause_job(&jid).await.map_err(cron_cli_error)?;
                    println!("Paused job {}", jid);
                }
                "resume" => {
                    sched.resume_job(&jid).await.map_err(cron_cli_error)?;
                    println!("Resumed job {}", jid);
                }
                "run" => {
                    let result = sched.run_job(&jid).await.map_err(cron_cli_error)?;
                    let json = serde_json::to_string_pretty(&result)
                        .unwrap_or_else(|_| format!("{result:#?}"));
                    println!("{}", json);
                }
                "history" => {
                    let job = sched
                        .get_job(&jid)
                        .await
                        .ok_or_else(|| AgentError::Config(format!("unknown job id: {}", jid)))?;
                    let json = serde_json::to_string_pretty(&job)
                        .map_err(|e| AgentError::Config(e.to_string()))?;
                    println!("{}", json);
                }
                _ => {
                    return Err(AgentError::Config(format!(
                        "internal: unexpected cron action '{}'",
                        act
                    )));
                }
            }
        }
        other => {
            return Err(AgentError::Config(format!(
                "Unknown cron action: {} (use list|create|delete|pause|resume|run|history)",
                other
            )));
        }
    }
    Ok(())
}

async fn prompt_line(prompt: impl Into<String>) -> Result<String, AgentError> {
    let prompt = prompt.into();
    let line = tokio::task::spawn_blocking(move || {
        use std::io::{self, Write};
        print!("{}", prompt);
        let _ = io::stdout().flush();
        let mut buf = String::new();
        io::stdin().read_line(&mut buf).map(|_| buf)
    })
    .await
    .map_err(|e| AgentError::Io(format!("stdin task: {}", e)))?
    .map_err(|e| AgentError::Io(format!("stdin: {}", e)))?;
    Ok(line.trim().to_string())
}

/// Resolve API key for `hermes auth login <provider>`: env → merged config → stdin.
async fn resolve_llm_login_token(cli: &Cli, provider: &str) -> Result<String, AgentError> {
    if let Some(k) = provider_api_key_from_env(provider) {
        return Ok(k);
    }
    let cfg =
        load_config(cli.config_dir.as_deref()).map_err(|e| AgentError::Config(e.to_string()))?;
    if let Some(k) = cfg
        .llm_providers
        .get(provider)
        .and_then(|c| c.api_key.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return Ok(k.to_string());
    }
    let fallback_var = format!("{}_API_KEY", provider.to_uppercase().replace('-', "_"));
    let msg = format!(
        "No API key in env or config for provider '{}'.\n\
         Set {} (see `hermes config set llm.{}.api_key ...`) or paste key now: ",
        provider, fallback_var, provider
    );
    let pasted = prompt_line(msg).await?;
    if pasted.is_empty() {
        return Err(AgentError::Config(format!(
            "Missing API key for provider '{}'",
            provider
        )));
    }
    Ok(pasted)
}

async fn run_serve(
    cli: Cli,
    action: Option<String>,
    host: String,
    port: u16,
    no_gateway: bool,
    no_cron: bool,
) -> Result<(), AgentError> {
    match action.as_deref() {
        None | Some("start") => {
            let config = hermes_config::load_config(cli.config_dir.as_deref())
                .map_err(|e| AgentError::Config(e.to_string()))?;
            let addr: std::net::SocketAddr = format!("{}:{}", host, port)
                .parse()
                .map_err(|e| AgentError::Config(format!("invalid address: {}", e)))?;

            let pid_path = serve_pid_path_for_cli(&cli);
            if let Some(pid) = read_gateway_pid(&pid_path) {
                if gateway_pid_is_alive(pid) {
                    println!(
                        "Serve already appears to be running (PID {}, file {}). Stop it first or remove a stale PID file.",
                        pid,
                        pid_path.display()
                    );
                    return Ok(());
                }
                let _ = std::fs::remove_file(&pid_path);
            }
            if let Some(parent) = pid_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(&pid_path, format!("{}\n", std::process::id()))
                .map_err(|e| AgentError::Io(format!("failed to write PID file: {}", e)))?;

            let cloud_mode = std::env::var("HERMES_CLOUD_MODE")
                .ok()
                .map(|v| is_truthy(&v))
                .unwrap_or(false);
            if cloud_mode {
                println!("Cloud mode is not available in engine-only hermes-server.");
                println!("Falling back to single-user API server mode.");
            }
            if !no_gateway || !no_cron {
                println!("Gateway/cron sidecars require runtime integration and are skipped in engine mode.");
            }
            println!("API server listening at http://{}", addr);

            let res = hermes_server::run_server(addr, config).await;
            let _ = std::fs::remove_file(&pid_path);
            res
        }
        Some("status") => {
            let pid_path = serve_pid_path_for_cli(&cli);
            match std::fs::read_to_string(&pid_path) {
                Ok(raw) => match raw.trim().parse::<u32>() {
                    Ok(pid) if gateway_pid_is_alive(pid) => {
                        println!(
                            "Serve status: running (PID {}, file {})",
                            pid,
                            pid_path.display()
                        );
                    }
                    Ok(pid) => {
                        println!(
                            "Serve status: not running (stale PID {} in {})",
                            pid,
                            pid_path.display()
                        );
                    }
                    Err(_) => {
                        println!("Serve status: invalid PID file at {}", pid_path.display());
                    }
                },
                Err(_) => {
                    println!(
                        "Serve status: not running (no PID file; start with `hermes serve start`)"
                    );
                }
            }
            Ok(())
        }
        Some("stop") => {
            let pid_path = serve_pid_path_for_cli(&cli);
            let Some(pid) = read_gateway_pid(&pid_path) else {
                println!("Serve stop: no PID file (nothing to stop).");
                return Ok(());
            };
            if !gateway_pid_is_alive(pid) {
                let _ = std::fs::remove_file(&pid_path);
                println!(
                    "Serve stop: process {} not running; removed stale PID file {}.",
                    pid,
                    pid_path.display()
                );
                return Ok(());
            }
            match gateway_pid_terminate(pid) {
                Ok(()) => {
                    println!("Sent SIGTERM to serve PID {}.", pid);
                    let _ = std::fs::remove_file(&pid_path);
                    println!("Removed {}.", pid_path.display());
                }
                Err(e) => println!("Serve stop: failed to signal PID {}: {}", pid, e),
            }
            Ok(())
        }
        Some(other) => {
            println!(
                "Unknown serve action: {}. Use 'start', 'stop', or 'status'.",
                other
            );
            Ok(())
        }
    }
}

/// Persistent cloud credential cached on disk.
///
/// Mirrors the response from `/api/v1/auth/login` and the user's chosen base
/// URL so subsequent `hermes cloud ...` invocations don't need any env vars.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct StoredCloudCredential {
    base_url: String,
    access_token: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    tenant_id: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    saved_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn cloud_credential_path() -> PathBuf {
    hermes_config::hermes_home().join("cloud_credentials.json")
}

fn load_cloud_credential() -> Option<StoredCloudCredential> {
    let path = cloud_credential_path();
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_cloud_credential(cred: &StoredCloudCredential) -> Result<(), AgentError> {
    let path = cloud_credential_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AgentError::Io(format!("create config dir: {}", e)))?;
    }
    let body = serde_json::to_string_pretty(cred)
        .map_err(|e| AgentError::Config(format!("serialize credential: {}", e)))?;
    std::fs::write(&path, body)
        .map_err(|e| AgentError::Io(format!("write {}: {}", path.display(), e)))?;
    // Restrict permissions on Unix-y systems.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(&path, perms);
        }
    }
    Ok(())
}

fn clear_cloud_credential() -> Result<bool, AgentError> {
    let path = cloud_credential_path();
    if !path.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&path)
        .map_err(|e| AgentError::Io(format!("remove {}: {}", path.display(), e)))?;
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
async fn run_cloud(
    action: Option<String>,
    agent_id: Option<String>,
    prompt: Option<String>,
    env: Option<String>,
    attempts: Option<u8>,
    email: Option<String>,
    password: Option<String>,
    url_override: Option<String>,
    register: bool,
    once: bool,
    poll_interval_secs: u64,
    limit: u32,
    json: bool,
) -> Result<(), AgentError> {
    let action = action.unwrap_or_else(|| "list".to_string());

    // Resolution order (highest to lowest):
    //   env: HERMES_CLOUD_API_URL / HERMES_CLOUD_BASE_URL
    //   stored credential: ~/.hermes/cloud_credentials.json (from `cloud login`)
    //   default: http://127.0.0.1:8787
    let stored = load_cloud_credential();
    let base_url = std::env::var("HERMES_CLOUD_API_URL")
        .ok()
        .or_else(|| std::env::var("HERMES_CLOUD_BASE_URL").ok())
        .or_else(|| stored.as_ref().map(|c| c.base_url.clone()))
        .or_else(|| url_override.clone())
        .unwrap_or_else(|| "http://127.0.0.1:8787".to_string());
    let token = std::env::var("HERMES_CLOUD_TOKEN")
        .ok()
        .or_else(|| std::env::var("HERMES_AUTH_TOKEN").ok())
        .or_else(|| stored.as_ref().map(|c| c.access_token.clone()));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| AgentError::Io(format!("failed to initialize cloud HTTP client: {}", e)))?;

    let with_auth = |req: reqwest::RequestBuilder| {
        if let Some(t) = token.as_ref() {
            req.bearer_auth(t)
        } else {
            req
        }
    };

    match action.as_str() {
        "login" => {
            return cloud_login(&client, url_override, email, password, register).await;
        }
        "logout" => {
            return cloud_logout();
        }
        "whoami" => {
            return cloud_whoami(&client, &base_url, token.as_deref()).await;
        }
        "logs" => {
            let id = agent_id.ok_or_else(|| {
                AgentError::Config(
                    "Missing --agent-id. Usage: hermes cloud logs --agent-id <id>".into(),
                )
            })?;
            return cloud_logs(
                &client,
                &base_url,
                token.as_deref(),
                &id,
                once,
                poll_interval_secs,
                json,
            )
            .await;
        }
        _ => {}
    }

    match action.as_str() {
        "list" => {
            let req = with_auth(client.get(format!("{}/api/v1/agents", base_url)));
            let res = req
                .send()
                .await
                .map_err(|e| AgentError::Io(format!("cloud list request failed: {}", e)))?;
            if !res.status().is_success() {
                let status = res.status();
                let body = res.text().await.unwrap_or_default();
                return Err(AgentError::Io(format!(
                    "cloud list failed ({}): {}",
                    status, body
                )));
            }
            let data: serde_json::Value = res
                .json()
                .await
                .map_err(|e| AgentError::Io(format!("cloud list JSON parse failed: {}", e)))?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&data).unwrap_or_else(|_| data.to_string())
                );
                return Ok(());
            }
            let items = if let Some(arr) = data.as_array() {
                arr.clone()
            } else {
                data.get("sessions")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default()
            };
            println!(
                "Cloud agents: {} (showing up to {})",
                items.len().min(limit as usize),
                limit
            );
            for item in items.into_iter().take(limit as usize) {
                let id = item
                    .get("session_id")
                    .or_else(|| item.get("agent_id"))
                    .or_else(|| item.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("<unknown>");
                let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("-");
                let model = item.get("model").and_then(|v| v.as_str()).unwrap_or("-");
                let updated = item
                    .get("updated_at")
                    .or_else(|| item.get("last_activity_at"))
                    .or_else(|| item.get("last_active_at"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                println!(
                    "  • {}  status={}  model={}  updated={}",
                    id, status, model, updated
                );
            }
            Ok(())
        }
        "status" => {
            let id = agent_id.ok_or_else(|| {
                AgentError::Config(
                    "Missing --agent-id. Usage: hermes cloud status --agent-id <id>".into(),
                )
            })?;
            let req = with_auth(client.get(format!("{}/api/v1/agents/{}/status", base_url, id)));
            let res = req
                .send()
                .await
                .map_err(|e| AgentError::Io(format!("cloud status request failed: {}", e)))?;
            if !res.status().is_success() {
                let status = res.status();
                let body = res.text().await.unwrap_or_default();
                return Err(AgentError::Io(format!(
                    "cloud status failed ({}): {}",
                    status, body
                )));
            }
            let data: serde_json::Value = res
                .json()
                .await
                .map_err(|e| AgentError::Io(format!("cloud status JSON parse failed: {}", e)))?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&data).unwrap_or_else(|_| data.to_string())
                );
            } else {
                println!(
                    "Cloud agent status ({}): {}",
                    id,
                    data.get("status")
                        .or_else(|| data.get("session").and_then(|v| v.get("status")))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                );
                println!(
                    "{}",
                    serde_json::to_string_pretty(&data).unwrap_or_else(|_| data.to_string())
                );
            }
            Ok(())
        }
        "exec" => {
            let prompt = prompt.ok_or_else(|| {
                AgentError::Config(
                    "Missing prompt. Usage: hermes cloud exec --agent-id <id> \"your task\"".into(),
                )
            })?;
            if let Some(n) = attempts {
                if !(1..=4).contains(&n) {
                    return Err(AgentError::Config(
                        "--attempts must be between 1 and 4".into(),
                    ));
                }
            }
            if env.is_some() || attempts.is_some() {
                println!(
                    "Note: --env/--attempts are accepted for Codex-style parity; this server may ignore them unless implemented."
                );
            }

            let target_agent_id = if let Some(id) = agent_id {
                id
            } else {
                let create_body = serde_json::json!({
                    "branch": "main",
                    "workspace_mode": "blank",
                    "startup_commands": []
                });
                let req = with_auth(
                    client
                        .post(format!("{}/api/v1/agents", base_url))
                        .json(&create_body),
                );
                let res = req.send().await.map_err(|e| {
                    AgentError::Io(format!("cloud exec create-agent request failed: {}", e))
                })?;
                if !res.status().is_success() {
                    let status = res.status();
                    let body = res.text().await.unwrap_or_default();
                    return Err(AgentError::Io(format!(
                        "cloud exec create-agent failed ({}): {}",
                        status, body
                    )));
                }
                let data: serde_json::Value = res.json().await.map_err(|e| {
                    AgentError::Io(format!("cloud exec create-agent JSON parse failed: {}", e))
                })?;
                data.get("session_id")
                    .or_else(|| data.get("agent_id"))
                    .or_else(|| data.get("id"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::Io(
                            "cloud exec create-agent returned no session_id/agent_id/id".into(),
                        )
                    })?
                    .to_string()
            };

            let body = serde_json::json!({
                "text": prompt
            });
            let req = with_auth(
                client
                    .post(format!(
                        "{}/api/v1/agents/{}/messages",
                        base_url, target_agent_id
                    ))
                    .json(&body),
            );
            let res = req
                .send()
                .await
                .map_err(|e| AgentError::Io(format!("cloud exec request failed: {}", e)))?;
            if !res.status().is_success() {
                let status = res.status();
                let body = res.text().await.unwrap_or_default();
                return Err(AgentError::Io(format!(
                    "cloud exec failed ({}): {}",
                    status, body
                )));
            }
            let data: serde_json::Value = res
                .json()
                .await
                .map_err(|e| AgentError::Io(format!("cloud exec JSON parse failed: {}", e)))?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&data).unwrap_or_else(|_| data.to_string())
                );
            } else {
                println!("Cloud exec submitted to agent: {}", target_agent_id);
                if let Some(text) = data
                    .get("text")
                    .or_else(|| data.get("reply"))
                    .and_then(|v| v.as_str())
                {
                    println!("\n{}", text);
                } else {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&data).unwrap_or_else(|_| data.to_string())
                    );
                }
            }
            Ok(())
        }
        other => Err(AgentError::Config(format!(
            "Unknown cloud action: {}. Use 'list', 'exec', 'status', 'logs', \
             'login', 'logout', or 'whoami'.",
            other
        ))),
    }
}

async fn cloud_login(
    client: &reqwest::Client,
    url_override: Option<String>,
    email: Option<String>,
    password: Option<String>,
    register: bool,
) -> Result<(), AgentError> {
    let stored = load_cloud_credential();
    let base_url = url_override
        .or_else(|| std::env::var("HERMES_CLOUD_API_URL").ok())
        .or_else(|| std::env::var("HERMES_CLOUD_BASE_URL").ok())
        .or_else(|| stored.as_ref().map(|c| c.base_url.clone()))
        .unwrap_or_else(|| "http://127.0.0.1:8787".to_string());

    let email = match email {
        Some(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => prompt_line("Email: ").await?,
    };
    if email.is_empty() {
        return Err(AgentError::Config("email is required".into()));
    }
    let password = match password {
        Some(v) if !v.is_empty() => v,
        _ => prompt_line("Password: ").await?,
    };
    if password.is_empty() {
        return Err(AgentError::Config("password is required".into()));
    }

    let endpoint = if register {
        format!("{}/api/v1/auth/register", base_url.trim_end_matches('/'))
    } else {
        format!("{}/api/v1/auth/login", base_url.trim_end_matches('/'))
    };
    let body = serde_json::json!({ "email": email, "password": password });
    let res = client
        .post(&endpoint)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            AgentError::Io(format!(
                "cloud {} request failed: {}",
                if register { "register" } else { "login" },
                e
            ))
        })?;
    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(AgentError::Io(format!(
            "cloud {} failed ({}): {}",
            if register { "register" } else { "login" },
            status,
            text
        )));
    }
    let data: serde_json::Value = res
        .json()
        .await
        .map_err(|e| AgentError::Io(format!("cloud login response parse failed: {}", e)))?;
    let token = data
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AgentError::Io("cloud login response missing access_token".into()))?
        .to_string();
    let user = data.get("user");
    let cred = StoredCloudCredential {
        base_url: base_url.trim_end_matches('/').to_string(),
        access_token: token,
        email: user
            .and_then(|u| u.get("email"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or(Some(email.clone())),
        user_id: user
            .and_then(|u| u.get("id"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        tenant_id: user
            .and_then(|u| u.get("tenant_id"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        expires_in: data.get("expires_in").and_then(|v| v.as_u64()),
        saved_at: Some(chrono::Utc::now()),
    };
    save_cloud_credential(&cred)?;
    println!(
        "Signed in to {} as {}",
        cred.base_url,
        cred.email.as_deref().unwrap_or(&email)
    );
    println!("Credential cached at {}", cloud_credential_path().display());
    Ok(())
}

fn cloud_logout() -> Result<(), AgentError> {
    let removed = clear_cloud_credential()?;
    if removed {
        println!("Signed out (removed {})", cloud_credential_path().display());
    } else {
        println!("No cloud credential cached.");
    }
    Ok(())
}

async fn cloud_logs(
    client: &reqwest::Client,
    base_url: &str,
    token: Option<&str>,
    agent_id: &str,
    once: bool,
    poll_interval_secs: u64,
    json: bool,
) -> Result<(), AgentError> {
    let endpoint = format!(
        "{}/api/v1/agents/{}/messages",
        base_url.trim_end_matches('/'),
        agent_id
    );
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let interval = std::time::Duration::from_secs(poll_interval_secs.max(1));
    let mut iteration = 0u64;
    loop {
        iteration += 1;
        let mut req = client.get(&endpoint);
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        let res = req
            .send()
            .await
            .map_err(|e| AgentError::Io(format!("cloud logs request failed: {}", e)))?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(AgentError::Io(format!(
                "cloud logs failed ({}): {}",
                status, body
            )));
        }
        let data: serde_json::Value = res
            .json()
            .await
            .map_err(|e| AgentError::Io(format!("cloud logs parse failed: {}", e)))?;
        let messages = data
            .get("messages")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for msg in messages.iter() {
            let id = msg
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !id.is_empty() && !seen_ids.insert(id.clone()) {
                continue;
            }
            if json {
                println!(
                    "{}",
                    serde_json::to_string(msg).unwrap_or_else(|_| msg.to_string())
                );
                continue;
            }
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("?");
            let created = msg.get("created_at").and_then(|v| v.as_str()).unwrap_or("");
            let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let prefix = match role {
                "user" => "↑",
                "assistant" => "↓",
                _ => "•",
            };
            println!("{} [{}] {}: {}", prefix, created, role, content);
        }
        if once {
            if iteration == 1 && messages.is_empty() {
                println!("(no messages yet)");
            }
            return Ok(());
        }
        tokio::time::sleep(interval).await;
    }
}

async fn cloud_whoami(
    client: &reqwest::Client,
    base_url: &str,
    token: Option<&str>,
) -> Result<(), AgentError> {
    let Some(token) = token else {
        println!("Not signed in. Run `hermes cloud login` first.");
        return Ok(());
    };
    let res = client
        .get(format!("{}/api/v1/auth/me", base_url.trim_end_matches('/')))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| AgentError::Io(format!("cloud whoami request failed: {}", e)))?;
    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(AgentError::Io(format!(
            "cloud whoami failed ({}): {}",
            status, text
        )));
    }
    let data: serde_json::Value = res
        .json()
        .await
        .map_err(|e| AgentError::Io(format!("cloud whoami parse failed: {}", e)))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&data).unwrap_or_else(|_| data.to_string())
    );
    Ok(())
}

async fn run_dump(
    cli: Cli,
    session: Option<String>,
    output: Option<String>,
) -> Result<(), AgentError> {
    let home = cli
        .config_dir
        .as_deref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(hermes_config::hermes_home);
    let sessions_dir = home.join("sessions");
    let session = session.unwrap_or_else(|| "latest".to_string());
    let out = output.unwrap_or_else(|| format!("{}.dump.json", session));
    let payload = serde_json::json!({
        "session": session,
        "source_dir": sessions_dir,
        "note": "Session export scaffold"
    });
    std::fs::write(
        &out,
        serde_json::to_string_pretty(&payload).unwrap_or_default(),
    )
    .map_err(|e| AgentError::Io(format!("Failed to write dump: {}", e)))?;
    println!("Wrote dump to {}", out);
    Ok(())
}

fn run_completion(shell: Option<String>) -> Result<(), AgentError> {
    let mut cmd = Cli::command();
    let sh = match shell.as_deref().unwrap_or("zsh") {
        "bash" => CompletionShell::Bash,
        "fish" => CompletionShell::Fish,
        "powershell" => CompletionShell::PowerShell,
        "elvish" => CompletionShell::Elvish,
        _ => CompletionShell::Zsh,
    };
    generate(sh, &mut cmd, "hermes", &mut std::io::stdout());
    Ok(())
}

async fn run_uninstall(yes: bool) -> Result<(), AgentError> {
    let home = hermes_config::hermes_home();
    if !yes {
        println!("Uninstall is destructive. Re-run with `hermes uninstall --yes`.");
        return Ok(());
    }
    if home.exists() {
        std::fs::remove_dir_all(&home)
            .map_err(|e| AgentError::Io(format!("Failed to remove {}: {}", home.display(), e)))?;
        println!("Removed {}", home.display());
    } else {
        println!("Nothing to uninstall.");
    }
    Ok(())
}

/// Handle `hermes lumio [action]`.
async fn run_lumio(action: Option<String>, model: Option<String>) -> Result<(), AgentError> {
    match action.as_deref() {
        None | Some("login") => {
            hermes_cli::lumio::setup(model.as_deref(), true).await?;
        }
        Some("logout") => {
            hermes_cli::lumio::clear_token();
            println!("✅ Lumio token removed.");
        }
        Some("status") => match hermes_cli::lumio::load_token() {
            Some(t) => {
                let user = if t.username.is_empty() {
                    "(unknown)"
                } else {
                    &t.username
                };
                println!("Lumio: logged in as {}", user);
                println!("  API: {}", t.base_url);
                println!(
                    "  Token: {}...{}",
                    &t.token[..8.min(t.token.len())],
                    &t.token[t.token.len().saturating_sub(4)..]
                );
            }
            None => {
                println!("Lumio: not logged in");
                println!("  Run `hermes lumio` to login.");
            }
        },
        Some(other) => {
            println!(
                "Unknown lumio action: '{}'. Use: login, logout, status.",
                other
            );
        }
    }
    Ok(())
}

/// Handle `hermes setup`.
async fn run_setup() -> Result<(), AgentError> {
    use std::io::{self, BufRead, Write};

    println!("Hermes Agent — Setup Wizard");
    println!("===========================\n");

    let config_dir = hermes_config::hermes_home();
    println!("Config directory: {}", config_dir.display());

    // 1. Create directory structure
    let subdirs = ["profiles", "sessions", "logs", "skills"];
    for dir in [config_dir.clone()]
        .into_iter()
        .chain(subdirs.iter().map(|d| config_dir.join(d)))
    {
        if dir.exists() {
            println!("  ✓ {} exists", dir.display());
        } else {
            std::fs::create_dir_all(&dir).map_err(|e| {
                AgentError::Io(format!("Failed to create {}: {}", dir.display(), e))
            })?;
            println!("  ✓ Created {}", dir.display());
        }
    }

    let config_path = config_dir.join("config.yaml");
    let stdin = io::stdin();
    let mut reader = stdin.lock();

    // 2. Prompt for API key
    print!("\nOpenAI API key (leave blank to skip): ");
    io::stdout().flush().ok();
    let mut api_key = String::new();
    reader.read_line(&mut api_key).ok();
    let api_key = api_key.trim().to_string();

    // 3. Prompt for model
    println!("\nAvailable models:");
    println!("  1) openai:gpt-4o          (recommended)");
    println!("  2) openai:gpt-4o-mini     (fast & cheap)");
    println!("  3) anthropic:claude-3-5-sonnet");
    println!("  4) openrouter:auto        (multi-provider)");
    print!("Choose model [1]: ");
    io::stdout().flush().ok();
    let mut model_choice = String::new();
    reader.read_line(&mut model_choice).ok();
    let model = match model_choice.trim() {
        "2" => "openai:gpt-4o-mini",
        "3" => "anthropic:claude-3-5-sonnet",
        "4" => "openrouter:auto",
        _ => "openai:gpt-4o",
    };

    // 4. Prompt for personality
    print!("\nPersonality (default, concise, creative, technical) [default]: ");
    io::stdout().flush().ok();
    let mut personality = String::new();
    reader.read_line(&mut personality).ok();
    let personality = personality.trim();
    let personality = if personality.is_empty() {
        "default"
    } else {
        personality
    };

    // 5. Write config.yaml
    if config_path.exists() {
        print!("\nconfig.yaml already exists. Overwrite? [y/N]: ");
        io::stdout().flush().ok();
        let mut answer = String::new();
        reader.read_line(&mut answer).ok();
        if !answer.trim().eq_ignore_ascii_case("y") {
            println!("Keeping existing config.yaml.");
            println!("\nSetup complete! Run `hermes` to start an interactive session.");
            return Ok(());
        }
    }

    let mut config_content = String::from("# Hermes Agent Configuration\n\n");
    config_content.push_str(&format!("model: {}\n", model));
    config_content.push_str(&format!("personality: {}\n", personality));
    config_content.push_str("max_turns: 50\n\n");

    if !api_key.is_empty() {
        config_content.push_str("llm_providers:\n");
        config_content.push_str("  openai:\n");
        config_content.push_str(&format!("    api_key: {}\n", api_key));
    }

    std::fs::write(&config_path, &config_content)
        .map_err(|e| AgentError::Io(format!("Failed to write config: {}", e)))?;
    println!("\n  ✓ Wrote config.yaml");

    // 6. Write default profile
    let default_profile = config_dir.join("profiles").join("default.yaml");
    if !default_profile.exists() {
        let profile_content = format!(
            "# Default Hermes Profile\nname: default\nmodel: {}\npersonality: {}\n",
            model, personality,
        );
        std::fs::write(&default_profile, profile_content)
            .map_err(|e| AgentError::Io(format!("Failed to write profile: {}", e)))?;
        println!("  ✓ Created default profile");
    }

    println!("\nSetup complete! Run `hermes` to start an interactive session.");
    println!("Run `hermes doctor` to check system requirements.");
    Ok(())
}

/// Handle `hermes doctor`.
async fn run_doctor(cli: Cli) -> Result<(), AgentError> {
    println!("Hermes Agent — System Check");
    println!("===========================\n");

    // Check config
    let config_dir = hermes_config::hermes_home();
    print!("Config directory ({})... ", config_dir.display());
    if config_dir.exists() {
        println!("✓");
    } else {
        println!("✗ (run `hermes setup`)");
    }

    // Check config.yaml
    let config_path = config_dir.join("config.yaml");
    print!("config.yaml... ");
    if config_path.exists() {
        println!("✓");
    } else {
        println!("✗ (run `hermes setup`)");
    }

    // Check API keys via environment
    let api_checks = [
        ("OPENAI_API_KEY", "OpenAI"),
        ("ANTHROPIC_API_KEY", "Anthropic"),
        ("OPENROUTER_API_KEY", "OpenRouter"),
        ("EXA_API_KEY", "Exa (web search)"),
        ("FIRECRAWL_API_KEY", "Firecrawl (web extract)"),
    ];

    println!("\nAPI Keys:");
    for (env_var, name) in &api_checks {
        print!("  {} ({})... ", name, env_var);
        if std::env::var(env_var).is_ok() {
            println!("✓");
        } else {
            println!("✗ (not set)");
        }
    }

    // Check external tools
    println!("\nExternal tools:");
    let tool_checks = [("docker", "Docker"), ("ssh", "SSH"), ("git", "Git")];

    for (cmd, name) in &tool_checks {
        print!("  {}... ", name);
        match tokio::process::Command::new("which")
            .arg(cmd)
            .output()
            .await
        {
            Ok(output) if output.status.success() => println!("✓"),
            _ => println!("✗ (not found)"),
        }
    }

    // Try loading config
    println!("\nConfiguration:");
    print!("  Loading config... ");
    match load_config(cli.config_dir.as_deref()) {
        Ok(config) => {
            println!("✓");
            println!(
                "  Model: {}",
                config.model.as_deref().unwrap_or("(default)")
            );
            println!("  Max turns: {}", config.max_turns);
            let platform_count = config.platforms.iter().filter(|(_, p)| p.enabled).count();
            println!("  Enabled platforms: {}", platform_count);
        }
        Err(e) => {
            println!("✗ ({})", e);
        }
    }

    println!("\nDone.");
    Ok(())
}

/// Handle `hermes update`.
async fn run_update() -> Result<(), AgentError> {
    println!("Hermes Agent v{}", env!("CARGO_PKG_VERSION"));
    println!("{}", hermes_cli::update::check_for_updates().await?);
    Ok(())
}

/// Handle `hermes status`.
async fn run_status(cli: Cli) -> Result<(), AgentError> {
    println!("Hermes Agent — Status");
    println!("=====================\n");

    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!(
        "Python baseline: v2026.4.16 ({})",
        "1dd6b5d5fb94cac59e93388f9aeee6bc365b8f42"
    );

    let config =
        load_config(cli.config_dir.as_deref()).map_err(|e| AgentError::Config(e.to_string()))?;

    println!(
        "Model:   {}",
        config.model.as_deref().unwrap_or("(default: gpt-4o)")
    );
    println!(
        "Personality: {}",
        config.personality.as_deref().unwrap_or("(none)")
    );
    println!("Max turns: {}", config.max_turns);

    let enabled_platforms: Vec<&String> = config
        .platforms
        .iter()
        .filter(|(_, pc)| pc.enabled)
        .map(|(name, _)| name)
        .collect();
    if enabled_platforms.is_empty() {
        println!("Platforms: (none enabled)");
    } else {
        println!(
            "Platforms: {}",
            enabled_platforms
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let config_dir = hermes_config::hermes_home();
    println!("\nConfig dir: {}", config_dir.display());

    println!(
        "Nous managed tools: {}",
        if hermes_config::managed_nous_tools_enabled() {
            "available"
        } else {
            "unavailable"
        }
    );

    println!("\nTool capability backends:");
    let web_ready = hermes_config::is_managed_tool_gateway_ready(
        "firecrawl",
        hermes_config::ResolveOptions::default(),
    );
    let image_ready = hermes_config::is_managed_tool_gateway_ready(
        "fal-queue",
        hermes_config::ResolveOptions::default(),
    );
    let tts_ready = hermes_config::is_managed_tool_gateway_ready(
        "openai-audio",
        hermes_config::ResolveOptions::default(),
    );
    let browser_ready = hermes_config::is_managed_tool_gateway_ready(
        "browser-use",
        hermes_config::ResolveOptions::default(),
    );
    println!(
        "  web:       use_gateway={} managed_ready={}",
        config.web.use_gateway, web_ready
    );
    println!(
        "  image_gen: use_gateway={} managed_ready={}",
        config.image_gen.use_gateway, image_ready
    );
    println!(
        "  tts:       use_gateway={} managed_ready={}",
        config.tts.use_gateway, tts_ready
    );
    println!(
        "  browser:   use_gateway={} managed_ready={} cloud_provider={}",
        config.browser.use_gateway,
        browser_ready,
        config.browser.cloud_provider.as_deref().unwrap_or("auto")
    );

    println!("\nExecution backend:");
    println!("  terminal.backend={:?}", config.terminal.backend);
    if matches!(
        config.terminal.backend,
        hermes_config::TerminalBackendType::Modal
    ) {
        let direct_modal = std::env::var("MODAL_API_TOKEN")
            .ok()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        let modal_state = hermes_config::resolve_modal_backend_state(
            config.terminal.modal_mode.as_deref(),
            direct_modal,
            hermes_config::is_managed_tool_gateway_ready(
                "modal",
                hermes_config::ResolveOptions::default(),
            ),
        );
        println!(
            "  modal_mode={} selected={}",
            modal_state.mode.as_str(),
            modal_state
                .selected_backend
                .map(|s| s.as_str())
                .unwrap_or("none")
        );
    }

    // Check for active sessions
    let sessions_dir = config_dir.join("sessions");
    if sessions_dir.exists() {
        let count = std::fs::read_dir(&sessions_dir)
            .map(|entries| entries.filter_map(|e| e.ok()).count())
            .unwrap_or(0);
        println!("Saved sessions: {}", count);
    }

    // Check for profiles
    let profiles_dir = config_dir.join("profiles");
    if profiles_dir.exists() {
        let profiles: Vec<String> = std::fs::read_dir(&profiles_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "yaml" || ext == "yml")
                            .unwrap_or(false)
                    })
                    .filter_map(|e| {
                        e.path()
                            .file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                    })
                    .collect()
            })
            .unwrap_or_default();
        if profiles.is_empty() {
            println!("Profiles: (none)");
        } else {
            println!("Profiles: {}", profiles.join(", "));
        }
    }

    Ok(())
}

/// Handle `hermes logs`.
async fn run_logs(cli: Cli, lines: u32, follow: bool) -> Result<(), AgentError> {
    let config_dir = cli
        .config_dir
        .as_deref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(hermes_config::hermes_home);
    let log_file = config_dir.join("logs").join("hermes.log");

    if !log_file.exists() {
        println!("No log file found at: {}", log_file.display());
        println!("Logs are written here during interactive sessions.");
        return Ok(());
    }

    if follow {
        println!("Tailing {}... (Ctrl+C to stop)\n", log_file.display());
        let mut child = tokio::process::Command::new("tail")
            .args(["-f", "-n", &lines.to_string()])
            .arg(&log_file)
            .spawn()
            .map_err(|e| AgentError::Io(format!("Failed to tail log file: {}", e)))?;

        tokio::select! {
            status = child.wait() => {
                match status {
                    Ok(s) if !s.success() => {
                        eprintln!("tail exited with status: {}", s);
                    }
                    Err(e) => {
                        eprintln!("Error waiting for tail: {}", e);
                    }
                    _ => {}
                }
            }
            _ = tokio::signal::ctrl_c() => {
                child.kill().await.ok();
                println!("\nStopped tailing logs.");
            }
        }
    } else {
        let content = std::fs::read_to_string(&log_file)
            .map_err(|e| AgentError::Io(format!("Failed to read log file: {}", e)))?;
        let all_lines: Vec<&str> = content.lines().collect();
        let start = all_lines.len().saturating_sub(lines as usize);
        for line in &all_lines[start..] {
            println!("{}", line);
        }
        println!(
            "\n(Showing last {} of {} lines from {})",
            all_lines.len() - start,
            all_lines.len(),
            log_file.display()
        );
    }
    Ok(())
}

/// Handle `hermes profile [action] [name]`.
async fn run_profile(
    cli: Cli,
    action: Option<String>,
    name: Option<String>,
) -> Result<(), AgentError> {
    let config_dir = cli
        .config_dir
        .as_deref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(hermes_config::hermes_home);
    let profiles_dir = config_dir.join("profiles");

    match action.as_deref() {
        None => {
            // Show current profile
            let config = load_config(cli.config_dir.as_deref())
                .map_err(|e| AgentError::Config(e.to_string()))?;
            println!("Current profile:");
            println!(
                "  Model:       {}",
                config.model.as_deref().unwrap_or("gpt-4o")
            );
            println!(
                "  Personality: {}",
                config.personality.as_deref().unwrap_or("default")
            );
            println!("  Max turns:   {}", config.max_turns);
            println!("\nUse `hermes profile list` to see all profiles.");
        }
        Some("list") => {
            if !profiles_dir.exists() {
                println!("No profiles directory found. Run `hermes setup` first.");
                return Ok(());
            }
            let entries: Vec<String> = std::fs::read_dir(&profiles_dir)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .filter(|e| {
                            e.path()
                                .extension()
                                .map(|ext| ext == "yaml" || ext == "yml")
                                .unwrap_or(false)
                        })
                        .filter_map(|e| {
                            e.path()
                                .file_stem()
                                .map(|s| s.to_string_lossy().into_owned())
                        })
                        .collect()
                })
                .unwrap_or_default();

            if entries.is_empty() {
                println!("No profiles found. Create one with `hermes profile create <name>`.");
            } else {
                println!("Available profiles:");
                for name in &entries {
                    println!("  • {}", name);
                }
            }
        }
        Some("create") => {
            let name = name.ok_or_else(|| {
                AgentError::Config(
                    "Missing profile name. Usage: hermes profile create <name>".into(),
                )
            })?;

            std::fs::create_dir_all(&profiles_dir)
                .map_err(|e| AgentError::Io(format!("Failed to create profiles dir: {}", e)))?;

            let profile_path = profiles_dir.join(format!("{}.yaml", name));
            if profile_path.exists() {
                println!(
                    "Profile '{}' already exists at {}",
                    name,
                    profile_path.display()
                );
                return Ok(());
            }

            let content = format!(
                "# Hermes Profile: {}\nname: {}\nmodel: openai:gpt-4o\npersonality: default\nmax_turns: 50\n",
                name, name
            );
            std::fs::write(&profile_path, content)
                .map_err(|e| AgentError::Io(format!("Failed to write profile: {}", e)))?;
            println!("Created profile '{}' at {}", name, profile_path.display());
            println!(
                "Edit it to customize, then switch with `hermes profile switch {}`.",
                name
            );
        }
        Some("switch") => {
            let name = name.ok_or_else(|| {
                AgentError::Config(
                    "Missing profile name. Usage: hermes profile switch <name>".into(),
                )
            })?;

            let profile_path = profiles_dir.join(format!("{}.yaml", &name));
            if !profile_path.exists() {
                // Also try .yml
                let alt = profiles_dir.join(format!("{}.yml", &name));
                if !alt.exists() {
                    println!("Profile '{}' not found. Available profiles:", name);
                    if let Ok(rd) = std::fs::read_dir(&profiles_dir) {
                        for entry in rd.filter_map(|e| e.ok()) {
                            if let Some(stem) = entry.path().file_stem() {
                                println!("  • {}", stem.to_string_lossy());
                            }
                        }
                    }
                    return Ok(());
                }
            }
            println!("Switched to profile: {}", name);
            println!("(Profile loading will be applied on next `hermes` session)");
        }
        Some(other) => {
            println!(
                "Unknown profile action: '{}'. Use 'list', 'create', or 'switch'.",
                other
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hermes_config::session::SessionConfig;
    use hermes_config::PlatformConfig;
    use hermes_gateway::dm::DmManager;
    use hermes_gateway::{Gateway, SessionManager};

    fn make_platform(enabled: bool, token: Option<&str>) -> PlatformConfig {
        let mut cfg = PlatformConfig {
            enabled,
            ..Default::default()
        };
        if let Some(t) = token {
            cfg.token = Some(t.to_string());
        }
        cfg
    }

    fn make_gateway() -> Arc<Gateway> {
        Arc::new(Gateway::new(
            Arc::new(SessionManager::new(SessionConfig::default())),
            DmManager::with_pair_behavior(),
            hermes_gateway::gateway::GatewayConfig::default(),
        ))
    }

    #[test]
    fn auth_provider_aliases_cover_primary_chains() {
        assert_eq!(normalize_auth_provider("tg"), "telegram");
        assert_eq!(normalize_auth_provider("wechat"), "weixin");
        assert_eq!(normalize_auth_provider("wx"), "weixin");
        assert_eq!(normalize_auth_provider("api-server"), "api_server");
        assert_eq!(normalize_auth_provider("mm"), "mattermost");
    }

    #[test]
    fn gateway_auth_provider_keys_include_primary_platforms() {
        for key in ["telegram", "weixin", "discord", "slack"] {
            let mapped = gateway_platform_provider_key(key);
            if key == "telegram" || key == "weixin" {
                assert!(mapped.is_none(), "{key} handled by dedicated auth flow");
            } else {
                assert_eq!(mapped, Some(key));
            }
        }
    }

    #[test]
    fn gateway_requirement_check_flags_missing_required_fields() {
        let mut config = hermes_config::GatewayConfig::default();
        config
            .platforms
            .insert("telegram".to_string(), make_platform(true, None));
        config
            .platforms
            .insert("qqbot".to_string(), make_platform(true, None));
        let issues = hermes_gateway::gateway_requirement_issues(&config);
        assert!(issues.iter().any(|s| s.contains("telegram")));
        assert!(issues.iter().any(|s| s.contains("qqbot")));
    }

    #[test]
    fn gateway_requirement_check_accepts_complete_qqbot_and_wecom_callback() {
        let mut config = hermes_config::GatewayConfig::default();

        let mut qqbot = make_platform(true, None);
        qqbot
            .extra
            .insert("app_id".to_string(), serde_json::json!("qq-app"));
        qqbot
            .extra
            .insert("client_secret".to_string(), serde_json::json!("qq-secret"));
        config.platforms.insert("qqbot".to_string(), qqbot);

        let mut wecom_cb = make_platform(true, Some("cb-token"));
        wecom_cb
            .extra
            .insert("corp_id".to_string(), serde_json::json!("wwcorp"));
        wecom_cb
            .extra
            .insert("corp_secret".to_string(), serde_json::json!("corp-secret"));
        wecom_cb
            .extra
            .insert("agent_id".to_string(), serde_json::json!("1000002"));
        wecom_cb.extra.insert(
            "encoding_aes_key".to_string(),
            serde_json::json!("abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG"),
        );
        config
            .platforms
            .insert("wecom_callback".to_string(), wecom_cb);

        assert!(hermes_gateway::gateway_requirement_issues(&config).is_empty());
    }

    #[tokio::test]
    async fn register_gateway_adapters_registers_primary_platforms_when_config_is_complete() {
        let mut config = hermes_config::GatewayConfig::default();

        let mut telegram = make_platform(true, Some("tg-token"));
        telegram
            .extra
            .insert("polling".to_string(), serde_json::json!(false));
        config.platforms.insert("telegram".to_string(), telegram);

        let mut weixin = make_platform(true, Some("wx-token"));
        weixin
            .extra
            .insert("account_id".to_string(), serde_json::json!("wxid_abc"));
        config.platforms.insert("weixin".to_string(), weixin);

        config.platforms.insert(
            "discord".to_string(),
            make_platform(true, Some("discord-token")),
        );
        config
            .platforms
            .insert("slack".to_string(), make_platform(true, Some("xoxb-slack")));

        let gateway = make_gateway();
        let mut sidecar_tasks = Vec::new();
        register_gateway_adapters(&config, gateway.clone(), &mut sidecar_tasks)
            .await
            .expect("primary platform registration should succeed");

        let mut names = gateway.adapter_names().await;
        names.sort();
        assert!(names.contains(&"telegram".to_string()));
        assert!(names.contains(&"weixin".to_string()));
        assert!(names.contains(&"discord".to_string()));
        assert!(names.contains(&"slack".to_string()));

        for task in sidecar_tasks {
            task.abort();
        }
    }

    #[tokio::test]
    async fn register_gateway_adapters_skips_primary_platforms_when_required_credentials_missing() {
        let mut config = hermes_config::GatewayConfig::default();
        config
            .platforms
            .insert("telegram".to_string(), make_platform(true, None));
        config
            .platforms
            .insert("weixin".to_string(), make_platform(true, None));
        config
            .platforms
            .insert("discord".to_string(), make_platform(true, None));
        config
            .platforms
            .insert("slack".to_string(), make_platform(true, None));

        let gateway = make_gateway();
        let mut sidecar_tasks = Vec::new();
        register_gateway_adapters(&config, gateway.clone(), &mut sidecar_tasks)
            .await
            .expect("missing credentials should be handled gracefully");

        assert!(
            gateway.adapter_names().await.is_empty(),
            "no primary adapter should register when required credentials are missing"
        );
        for task in sidecar_tasks {
            task.abort();
        }
    }

    #[tokio::test]
    async fn register_gateway_adapters_registers_qqbot_and_wecom_callback() {
        let mut config = hermes_config::GatewayConfig::default();

        let mut qqbot = make_platform(true, None);
        qqbot
            .extra
            .insert("app_id".to_string(), serde_json::json!("qq-app"));
        qqbot
            .extra
            .insert("client_secret".to_string(), serde_json::json!("qq-secret"));
        config.platforms.insert("qqbot".to_string(), qqbot);

        let mut wecom_cb = make_platform(true, None);
        wecom_cb
            .extra
            .insert("corp_id".to_string(), serde_json::json!("wwcorp"));
        wecom_cb
            .extra
            .insert("corp_secret".to_string(), serde_json::json!("corp-secret"));
        wecom_cb
            .extra
            .insert("agent_id".to_string(), serde_json::json!("1000002"));
        wecom_cb
            .extra
            .insert("token".to_string(), serde_json::json!("cb-token"));
        wecom_cb.extra.insert(
            "encoding_aes_key".to_string(),
            serde_json::json!("abcdefghijklmnopqrstuvwxyz0123456789ABCDEFG"),
        );
        config
            .platforms
            .insert("wecom_callback".to_string(), wecom_cb);

        let gateway = make_gateway();
        let mut sidecar_tasks = Vec::new();
        register_gateway_adapters(&config, gateway.clone(), &mut sidecar_tasks)
            .await
            .expect("qqbot and wecom_callback should register");

        let names = gateway.adapter_names().await;
        assert!(names.contains(&"qqbot".to_string()));
        assert!(names.contains(&"wecom_callback".to_string()));
    }
}

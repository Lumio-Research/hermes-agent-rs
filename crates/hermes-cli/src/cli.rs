//! CLI argument parsing.
//!
//! Defines the command-line interface for the hermes binary.
//!
//! ## Command tree (v0.2)
//!
//! ```text
//! hermes                          # interactive REPL (default)
//! hermes chat                     # single-shot query
//! hermes serve                    # API server + gateway + cron
//! hermes cloud                    # cloud task workflows (list/exec/status)
//! hermes gateway                  # platform gateway only (no API)
//! hermes config                   # configuration management
//! hermes model                    # model / provider management
//! hermes auth                     # authentication (login/logout/status)
//! hermes tools                    # tool management
//! hermes mcp                      # MCP server management
//! hermes skills                   # skill management
//! hermes plugins                  # plugin management
//! hermes sessions                 # session management (incl. export/dump)
//! hermes memory                   # memory provider management (incl. setup)
//! hermes cron                     # cron job management
//! hermes acp                      # Agent Communication Protocol
//! hermes setup                    # interactive setup wizard
//! hermes doctor                   # dependency & config health check
//! hermes status                   # running status
//! hermes logs                     # recent logs
//! hermes profile                  # profile management
//! hermes backup / import          # backup & restore
//! hermes update / version         # update & version info
//! hermes completion               # shell completions
//! hermes uninstall                # uninstall helper
//! ```

use clap::{Parser, Subcommand};

// ---------------------------------------------------------------------------
// CliCommand
// ---------------------------------------------------------------------------

/// Top-level subcommands for the hermes CLI.
#[derive(Debug, Clone, Subcommand)]
pub enum CliCommand {
    // ── Core ────────────────────────────────────────────────────────
    /// Start an interactive session (default when no subcommand is given).
    #[command(name = "hermes")]
    Hermes,

    /// Start an interactive chat session.
    Chat {
        /// Single-shot query (non-interactive).
        #[arg(short, long)]
        query: Option<String>,
        /// Preload a skill before chatting.
        #[arg(long)]
        preload_skill: Option<String>,
        /// Skip confirmation for dangerous tools.
        #[arg(long)]
        yolo: bool,
    },

    /// Start or manage unified runtime (API server + gateway + cron).
    ///
    /// Examples:
    ///   hermes serve                    — start all subsystems
    ///   hermes serve --no-gateway       — API + cron only
    ///   hermes serve --no-cron          — API + gateway only
    ///   hermes serve stop               — stop the running server
    Serve {
        /// Action: "start", "stop", or "status" (defaults to "start").
        action: Option<String>,
        /// Host for API server bind address.
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
        /// Port for API server bind address.
        #[arg(long, default_value = "3000")]
        port: u16,
        /// Disable gateway (platform adapters) subsystem.
        #[arg(long)]
        no_gateway: bool,
        /// Disable cron subsystem.
        #[arg(long)]
        no_cron: bool,
    },

    /// Interact with cloud tasks from the terminal.
    ///
    /// Examples:
    ///   hermes cloud login --email me@example.com    — store cloud bearer
    ///   hermes cloud whoami                          — show signed-in identity
    ///   hermes cloud logout                          — drop stored bearer
    ///   hermes cloud                                 — list recent cloud agents
    ///   hermes cloud list --limit 20                 — list recent cloud agents
    ///   hermes cloud exec --agent-id <id> "Fix lint errors"
    ///   hermes cloud status --agent-id <id>          — get cloud agent status
    ///   hermes cloud logs --agent-id <id>            — tail messages (Ctrl-C to stop)
    ///   hermes cloud logs --agent-id <id> --once     — print existing messages and exit
    Cloud {
        /// Action: "list", "exec", "status", "logs", "login", "logout", or
        /// "whoami" (defaults to "list").
        action: Option<String>,
        /// Target cloud agent id (required for status, optional for exec).
        #[arg(long)]
        agent_id: Option<String>,
        /// Prompt used with "exec".
        prompt: Option<String>,
        /// Target cloud environment id (parity placeholder).
        #[arg(long)]
        env: Option<String>,
        /// Best-of-N attempt count for cloud execution (1-4, parity placeholder).
        #[arg(long)]
        attempts: Option<u8>,
        /// Email used for `login` (will prompt if missing).
        #[arg(long)]
        email: Option<String>,
        /// Password used for `login` (will prompt if missing).
        #[arg(long)]
        password: Option<String>,
        /// Override base URL for `login`. Defaults to HERMES_CLOUD_API_URL or
        /// the URL stored from a previous login.
        #[arg(long = "url")]
        url_override: Option<String>,
        /// Treat `login` request as a registration (POST /api/v1/auth/register).
        #[arg(long)]
        register: bool,
        /// For `logs`: print existing messages once and exit (no follow).
        #[arg(long)]
        once: bool,
        /// For `logs`: poll interval in seconds (defaults to 2).
        #[arg(long = "interval", default_value_t = 2)]
        poll_interval_secs: u64,
        /// Max rows for "list".
        #[arg(long, default_value_t = 20)]
        limit: u32,
        /// Print raw JSON output.
        #[arg(long)]
        json: bool,
    },

    /// Start or manage the platform gateway (no API server).
    ///
    /// Examples:
    ///   hermes gateway start            — start the gateway
    ///   hermes gateway status           — check gateway status
    ///   hermes gateway setup            — interactive platform setup
    ///   hermes gateway setup whatsapp   — WhatsApp-specific setup
    Gateway {
        /// Action: "start", "stop", "status", or "setup".
        action: Option<String>,
        /// Platform name for setup (e.g. "whatsapp", "telegram").
        #[arg(long)]
        platform: Option<String>,
    },

    // ── Configuration & Model ───────────────────────────────────────
    /// Configuration management.
    ///
    /// Examples:
    ///   hermes config                   — show full configuration
    ///   hermes config get model         — get a specific config key
    ///   hermes config set model gpt-4o  — set a config key
    ///   hermes config set region us-east-1 — set API region
    Config {
        /// Action: "get", "set", "show", "path", "edit", "check", "migrate".
        action: Option<String>,
        /// Configuration key.
        key: Option<String>,
        /// Configuration value (used with "set" action).
        value: Option<String>,
    },

    /// Model and provider management.
    ///
    /// Examples:
    ///   hermes model                    — show current model
    ///   hermes model openai:gpt-4o      — switch model
    ///   hermes model provider list      — list available providers
    ///   hermes model provider set openai — switch provider
    Model {
        /// Provider:model identifier, or sub-action ("provider").
        provider_model: Option<String>,
        /// Second positional for sub-actions (e.g. "list", "set").
        sub_action: Option<String>,
        /// Third positional for sub-action arguments.
        sub_arg: Option<String>,
    },

    /// Authentication management (login, logout, status).
    ///
    /// Examples:
    ///   hermes auth login               — login to default provider
    ///   hermes auth login lumio          — login to Lumio
    ///   hermes auth logout openai       — logout from OpenAI
    ///   hermes auth status              — show auth status
    Auth {
        /// Action: "login", "logout", "status".
        action: Option<String>,
        /// Provider: openai/anthropic/lumio/copilot/telegram/weixin.
        provider: Option<String>,
        /// For Weixin login: prefer QR flow.
        #[arg(long)]
        qr: bool,
        /// Model to use after login (for Lumio).
        #[arg(short, long)]
        model: Option<String>,
    },

    // ── Tools & Extensions ──────────────────────────────────────────
    /// List or manage available tools.
    Tools {
        /// Action: "list", "enable <name>", or "disable <name>".
        action: Option<String>,
    },

    /// MCP server management.
    Mcp {
        /// Action: serve/add/remove/list/test/configure.
        action: Option<String>,
        /// Server name or URL.
        #[arg(long)]
        server: Option<String>,
    },

    /// Skills management.
    Skills {
        /// Action: browse/search/install/inspect/list/check/update/audit/uninstall/publish/snapshot/tap/config.
        action: Option<String>,
        /// Skill name or search query.
        name: Option<String>,
        /// Additional argument (e.g. tap URL, snapshot path).
        #[arg(long)]
        extra: Option<String>,
    },

    /// Plugin management.
    Plugins {
        /// Action: install/update/remove/list/enable/disable.
        action: Option<String>,
        /// Plugin name.
        name: Option<String>,
        /// Git branch, tag, or commit to checkout after clone.
        #[arg(long = "ref")]
        git_ref: Option<String>,
        /// Allow clone from hosts outside the default allowlist.
        #[arg(long)]
        allow_untrusted_git_host: bool,
    },

    // ── Data & Sessions ─────────────────────────────────────────────
    /// Session management (list, export, delete, stats, dump).
    ///
    /// Examples:
    ///   hermes sessions                 — list sessions
    ///   hermes sessions export --id abc — export a session
    ///   hermes sessions stats           — usage analytics
    Sessions {
        /// Action: list/export/delete/prune/stats/rename/browse/dump.
        action: Option<String>,
        /// Session ID.
        #[arg(long)]
        id: Option<String>,
        /// New name (for rename) or output path (for export/dump).
        #[arg(long)]
        name: Option<String>,
        /// Output path (for export/dump).
        #[arg(long)]
        output: Option<String>,
    },

    /// Memory provider management.
    ///
    /// Examples:
    ///   hermes memory                   — show memory status
    ///   hermes memory setup             — run setup wizard
    ///   hermes memory setup redis       — configure redis
    ///   hermes memory off               — disable memory provider
    Memory {
        /// Action: setup/status/off.
        action: Option<String>,
        /// Provider name (e.g. "redis", "qdrant", "mem0").
        provider: Option<String>,
    },

    /// Cron job management.
    Cron {
        /// Action: list/create/delete/pause/resume/run/history.
        action: Option<String>,
        /// Job id.
        #[arg(long)]
        id: Option<String>,
        /// Cron schedule (create), e.g. "0 9 * * *".
        #[arg(long)]
        schedule: Option<String>,
        /// Prompt text (create).
        #[arg(long)]
        prompt: Option<String>,
    },

    /// ACP (Agent Communication Protocol) server.
    Acp {
        /// Action: start/status.
        action: Option<String>,
    },

    // ── System ──────────────────────────────────────────────────────
    /// Run the interactive setup wizard.
    Setup,

    /// Check dependencies and configuration health.
    Doctor,

    /// Show running status (active sessions, model, uptime).
    Status,

    /// Show recent logs.
    Logs {
        /// Number of recent log entries to show (default: 20).
        #[arg(default_value = "20")]
        lines: u32,
        /// Tail the log file in real-time.
        #[arg(short, long)]
        follow: bool,
    },

    /// Profile management (list, switch, create).
    Profile {
        /// Action: "list", "create", "switch", or omitted to show current.
        action: Option<String>,
        /// Profile name.
        name: Option<String>,
    },

    /// Backup configuration and sessions.
    Backup {
        /// Output path for backup archive.
        output: Option<String>,
    },

    /// Import configuration from backup.
    Import {
        /// Path to backup archive.
        path: String,
    },

    /// Check for updates.
    Update,

    /// Show version information.
    Version,

    /// Generate shell completion scripts.
    Completion {
        /// Shell type: bash/zsh/fish/powershell/elvish.
        shell: Option<String>,
    },

    /// Uninstall helper (removes ~/.hermes by default).
    Uninstall {
        /// Confirm destructive cleanup.
        #[arg(long)]
        yes: bool,
    },
}

// ---------------------------------------------------------------------------
// Cli
// ---------------------------------------------------------------------------

/// Hermes Agent CLI.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "hermes",
    version,
    about = "Hermes Agent — autonomous AI agent with tool use",
    long_about = "Hermes Agent is an autonomous AI agent that can use tools, execute code, \
                  and interact with various platforms. Start an interactive session with `hermes` \
                  or use subcommands for specific tasks."
)]
pub struct Cli {
    /// The subcommand to execute. Defaults to starting an interactive session.
    #[command(subcommand)]
    pub command: Option<CliCommand>,

    /// Enable verbose / debug logging.
    #[arg(short = 'v', long, global = true)]
    pub verbose: bool,

    /// Override the configuration directory path.
    #[arg(short = 'C', long, global = true)]
    pub config_dir: Option<String>,

    /// Override the default model (e.g. "openai:gpt-4o").
    #[arg(short = 'm', long, global = true)]
    pub model: Option<String>,

    /// Override the personality / persona.
    #[arg(short = 'p', long, global = true)]
    pub personality: Option<String>,
}

impl Cli {
    /// Return the effective command, defaulting to `CliCommand::Hermes`.
    pub fn effective_command(&self) -> CliCommand {
        self.command.clone().unwrap_or(CliCommand::Hermes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn cli_parse_default() {
        let cli = Cli::try_parse_from(vec!["hermes"]).unwrap();
        assert!(cli.command.is_none());
        assert!(!cli.verbose);
    }

    #[test]
    fn cli_parse_model() {
        let cli = Cli::try_parse_from(vec!["hermes", "model", "openai:gpt-4o"]).unwrap();
        match cli.command {
            Some(CliCommand::Model { provider_model, .. }) => {
                assert_eq!(provider_model.as_deref(), Some("openai:gpt-4o"));
            }
            _ => panic!("Expected Model command"),
        }
    }

    #[test]
    fn cli_parse_config_set() {
        let cli = Cli::try_parse_from(vec!["hermes", "config", "set", "model", "gpt-4o"]).unwrap();
        match cli.command {
            Some(CliCommand::Config { action, key, value }) => {
                assert_eq!(action.as_deref(), Some("set"));
                assert_eq!(key.as_deref(), Some("model"));
                assert_eq!(value.as_deref(), Some("gpt-4o"));
            }
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn cli_parse_serve_defaults() {
        let cli = Cli::try_parse_from(vec!["hermes", "serve"]).unwrap();
        match cli.command {
            Some(CliCommand::Serve {
                action,
                host,
                port,
                no_gateway,
                no_cron,
            }) => {
                assert!(action.is_none());
                assert_eq!(host, "0.0.0.0");
                assert_eq!(port, 3000);
                assert!(!no_gateway);
                assert!(!no_cron);
            }
            _ => panic!("Expected Serve command"),
        }
    }

    #[test]
    fn cli_parse_serve_with_flags() {
        let cli = Cli::try_parse_from(vec![
            "hermes",
            "serve",
            "start",
            "--host",
            "127.0.0.1",
            "--port",
            "9090",
            "--no-gateway",
            "--no-cron",
        ])
        .unwrap();
        match cli.command {
            Some(CliCommand::Serve {
                action,
                host,
                port,
                no_gateway,
                no_cron,
            }) => {
                assert_eq!(action.as_deref(), Some("start"));
                assert_eq!(host, "127.0.0.1");
                assert_eq!(port, 9090);
                assert!(no_gateway);
                assert!(no_cron);
            }
            _ => panic!("Expected Serve command"),
        }
    }

    #[test]
    fn cli_parse_cloud_exec() {
        let cli = Cli::try_parse_from(vec![
            "hermes",
            "cloud",
            "exec",
            "--agent-id",
            "agent-123",
            "--env",
            "staging",
            "--attempts",
            "2",
            "run regression checks",
        ])
        .unwrap();
        match cli.command {
            Some(CliCommand::Cloud {
                action,
                agent_id,
                prompt,
                env,
                attempts,
                limit,
                json,
                ..
            }) => {
                assert_eq!(action.as_deref(), Some("exec"));
                assert_eq!(agent_id.as_deref(), Some("agent-123"));
                assert_eq!(prompt.as_deref(), Some("run regression checks"));
                assert_eq!(env.as_deref(), Some("staging"));
                assert_eq!(attempts, Some(2));
                assert_eq!(limit, 20);
                assert!(!json);
            }
            _ => panic!("Expected Cloud command"),
        }
    }

    #[test]
    fn cli_parse_auth_login() {
        let cli = Cli::try_parse_from(vec!["hermes", "auth", "login", "openai"]).unwrap();
        match cli.command {
            Some(CliCommand::Auth {
                action, provider, ..
            }) => {
                assert_eq!(action.as_deref(), Some("login"));
                assert_eq!(provider.as_deref(), Some("openai"));
            }
            _ => panic!("Expected Auth command"),
        }
    }

    #[test]
    fn cli_parse_memory_with_provider() {
        let cli = Cli::try_parse_from(vec!["hermes", "memory", "setup", "redis"]).unwrap();
        match cli.command {
            Some(CliCommand::Memory { action, provider }) => {
                assert_eq!(action.as_deref(), Some("setup"));
                assert_eq!(provider.as_deref(), Some("redis"));
            }
            _ => panic!("Expected Memory command"),
        }
    }

    #[test]
    fn cli_parse_sessions_export() {
        let cli = Cli::try_parse_from(vec!["hermes", "sessions", "export", "--id", "abc"]).unwrap();
        match cli.command {
            Some(CliCommand::Sessions { action, id, .. }) => {
                assert_eq!(action.as_deref(), Some("export"));
                assert_eq!(id.as_deref(), Some("abc"));
            }
            _ => panic!("Expected Sessions command"),
        }
    }

    #[test]
    fn removed_commands_are_rejected() {
        assert!(Cli::try_parse_from(vec!["hermes", "login"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "logout"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "dashboard"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "dump"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "insights"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "clipboard"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "whatsapp"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "claw"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "lumio"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "region"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "subscription"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "codex-models"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "runtime-provider"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "memory-setup"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "pairing"]).is_err());
        assert!(Cli::try_parse_from(vec!["hermes", "webhook"]).is_err());
    }
}

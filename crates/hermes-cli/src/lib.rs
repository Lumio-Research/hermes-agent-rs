#![allow(
    clippy::bind_instead_of_map,
    clippy::collapsible_if,
    clippy::double_ended_iterator_last,
    clippy::field_reassign_with_default,
    clippy::items_after_test_module,
    clippy::needless_borrow,
    clippy::needless_borrows_for_generic_args,
    clippy::nonminimal_bool,
    clippy::print_literal,
    clippy::redundant_closure,
    clippy::type_complexity,
    clippy::unwrap_or_default,
    clippy::useless_format,
    unreachable_patterns
)]
//! # hermes-cli
//!
//! CLI/TUI interface for Hermes Agent.
//!
//! This crate implements the terminal user interface (Requirement 9):
//! - Interactive REPL with streaming output (9.1, 9.5)
//! - Slash command support with auto-completion (9.2)
//! - Ctrl+C interrupt for tool execution (9.3)
//! - Message history display (9.4)
//! - CLI argument parsing via clap (9.7)
//! - Theme/skin engine (9.8)

pub mod app;
pub mod auth;
pub mod banner;
pub mod checklist;
pub mod claw_migrate;
pub mod cli;
pub mod commands;
pub mod copilot_auth;
pub mod doctor;
pub mod env_loader;
pub mod gateway_cmd;
pub mod live_messaging;
pub mod lumio;
pub mod mcp_config;
pub mod model_switch;
pub mod pairing_store;
pub mod profiles;
pub mod providers;
pub mod setup;
pub mod skills_config;
pub mod skin_engine;
pub mod theme;
pub mod tools_config;
pub mod tui;
pub mod update;
pub mod webhook_delivery;

// Re-export primary types
pub use app::App;
pub use checklist::{curses_checklist, ChecklistResult};
pub use claw_migrate::{run_migration, MigrateOptions, MigrationResult};
pub use cli::{Cli, CliCommand};
pub use commands::CommandResult;
pub use theme::Theme;
pub use tui::{Event, InputMode, ToolOutputSection, Tui};

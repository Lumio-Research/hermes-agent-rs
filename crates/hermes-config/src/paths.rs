//! Path management for the hermes home directory and its files.

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// hermes_home
// ---------------------------------------------------------------------------

/// Return the hermes home directory.
///
/// - If the `HERMES_HOME` environment variable is set, use that.
/// - Otherwise default to `~/.hermes` (i.e. [`user_home_dir`]`/.hermes`).
///
/// We intentionally do **not** use macOS `Library/Application Support/…`
/// here so CLI, gateway, dashboard, and docs all agree on one layout.
pub fn hermes_home() -> PathBuf {
    if let Ok(home) = std::env::var("HERMES_HOME") {
        PathBuf::from(home)
    } else {
        user_home_dir().join(".hermes")
    }
}

/// Best-effort user home (`$HOME` / `%USERPROFILE%`), for `~/.hermes`.
fn user_home_dir() -> PathBuf {
    directories::UserDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
        .or_else(|| std::env::var("USERPROFILE").ok().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Hermes state root directory (same rule as CLI `--config-dir` / gateway data).
///
/// If `config_dir_override` is set, that path is used; otherwise [`hermes_home`].
/// Use this for `cron/`, `webhooks.json`, and other machine-local state so CLI
/// and gateway stay aligned.
pub fn state_dir(config_dir_override: Option<&Path>) -> PathBuf {
    config_dir_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(hermes_home)
}

// ---------------------------------------------------------------------------
// Derived paths
// ---------------------------------------------------------------------------

/// `$hermes_home/config.yaml`
pub fn config_path() -> PathBuf {
    hermes_home().join("config.yaml")
}

/// `$hermes_home/cli-config.yaml`
pub fn cli_config_path() -> PathBuf {
    hermes_home().join("cli-config.yaml")
}

/// `$hermes_home/gateway.json`
pub fn gateway_json_path() -> PathBuf {
    hermes_home().join("gateway.json")
}

/// PID file written by `hermes gateway start` (same directory as `config.yaml`).
pub fn gateway_pid_path() -> PathBuf {
    hermes_home().join("gateway.pid")
}

/// Gateway PID file under an explicit Hermes home directory (e.g. `HERMES_HOME` or `-C`).
pub fn gateway_pid_path_in(home: impl AsRef<std::path::Path>) -> PathBuf {
    home.as_ref().join("gateway.pid")
}

/// `$hermes_home/MEMORY.md`
pub fn memory_path() -> PathBuf {
    hermes_home().join("MEMORY.md")
}

/// `$hermes_home/USER.md`
pub fn user_path() -> PathBuf {
    hermes_home().join("USER.md")
}

/// `$hermes_home/skills/`
pub fn skills_dir() -> PathBuf {
    hermes_home().join("skills")
}

/// `$hermes_home/sessions/`
pub fn sessions_dir() -> PathBuf {
    hermes_home().join("sessions")
}

/// `$hermes_home/cron/`
pub fn cron_dir() -> PathBuf {
    hermes_home().join("cron")
}

/// `$hermes_home/.env`
pub fn env_path() -> PathBuf {
    hermes_home().join(".env")
}

/// `$hermes_home/auth.json` — credential store written by `hermes auth login`.
///
/// Mirrors Python's `tools.managed_tool_gateway.auth_json_path()`. Used by
/// the managed-tool-gateway resolver to read provider OAuth tokens.
pub fn auth_json_path() -> PathBuf {
    hermes_home().join("auth.json")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hermes_home_respects_env() {
        // If HERMES_HOME is not set, we should get ~/.hermes
        let home = hermes_home();
        // Just ensure it's a valid path (not empty)
        assert!(!home.as_os_str().is_empty());
    }

    /// Combined test for all path helpers.
    ///
    /// Environment-variable mutations are not thread-safe, so we test
    /// both "derived paths" and "explicit home" in a single test to
    /// avoid races with parallel test threads.
    #[test]
    fn derived_paths_and_explicit_home() {
        // Acquire the workspace-wide env-var lock to prevent races with
        // managed_gateway tests that also mutate HERMES_HOME.
        let _g = crate::managed_gateway::test_lock::lock();

        let original = std::env::var("HERMES_HOME").ok();

        // -- Part 1: derived paths are consistent --
        std::env::set_var("HERMES_HOME", "/tmp/hermes-path-test");
        let home = hermes_home();
        assert_eq!(home, PathBuf::from("/tmp/hermes-path-test"));
        assert_eq!(state_dir(None), home);
        assert_eq!(config_path(), home.join("config.yaml"));
        assert_eq!(cli_config_path(), home.join("cli-config.yaml"));
        assert_eq!(gateway_json_path(), home.join("gateway.json"));
        assert_eq!(gateway_pid_path(), home.join("gateway.pid"));
        assert_eq!(memory_path(), home.join("MEMORY.md"));
        assert_eq!(user_path(), home.join("USER.md"));
        assert_eq!(skills_dir(), home.join("skills"));
        assert_eq!(sessions_dir(), home.join("sessions"));
        assert_eq!(cron_dir(), home.join("cron"));
        assert_eq!(env_path(), home.join(".env"));

        // -- Part 2: explicit home override --
        std::env::set_var("HERMES_HOME", "/tmp/test-hermes");
        assert_eq!(hermes_home(), PathBuf::from("/tmp/test-hermes"));
        assert_eq!(config_path(), PathBuf::from("/tmp/test-hermes/config.yaml"));

        // Restore
        match original {
            Some(v) => std::env::set_var("HERMES_HOME", v),
            None => std::env::remove_var("HERMES_HOME"),
        }
    }
}

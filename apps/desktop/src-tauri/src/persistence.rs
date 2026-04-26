//! JSON file persistence for sessions, projects, and config.

use std::path::Path;
use crate::{AppConfig, Project, Session};

const SESSIONS_FILE: &str = "sessions.json";
const PROJECTS_FILE: &str = "projects.json";
const CONFIG_FILE: &str = "config.json";

pub fn load_all(data_dir: &Path) -> (Vec<Session>, Vec<Project>, AppConfig) {
    let sessions = load_json::<Vec<Session>>(&data_dir.join(SESSIONS_FILE)).unwrap_or_default();
    let projects = load_json::<Vec<Project>>(&data_dir.join(PROJECTS_FILE)).unwrap_or_default();
    let config = load_json::<AppConfig>(&data_dir.join(CONFIG_FILE)).unwrap_or_else(|| AppConfig {
        api_base: "http://127.0.0.1:8787".into(),
        default_model: String::new(), // Empty = use hermes config default
        theme: "dark".into(),
        mode: "local".into(), // Default to local mode
    });
    (sessions, projects, config)
}

pub fn save_all(data_dir: &Path, sessions: &[Session], projects: &[Project], config: &AppConfig) {
    save_json(&data_dir.join(SESSIONS_FILE), sessions);
    save_json(&data_dir.join(PROJECTS_FILE), projects);
    save_json(&data_dir.join(CONFIG_FILE), config);
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_json<T: serde::Serialize + ?Sized>(path: &Path, value: &T) {
    if let Ok(data) = serde_json::to_string_pretty(value) {
        let _ = std::fs::write(path, data);
    }
}

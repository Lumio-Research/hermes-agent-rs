use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

mod persistence;
mod hermes_client;
mod local_agent;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub status: String,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub project: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub api_base: String,
    pub default_model: String,
    pub theme: String,
    /// "local" or "remote"
    pub mode: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamDelta {
    pub session_id: String,
    pub delta_type: String, // "text" | "thinking" | "tool_start" | "tool_complete" | "status" | "activity" | "done"
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct AppState {
    pub sessions: Mutex<Vec<Session>>,
    pub projects: Mutex<Vec<Project>>,
    pub config: Mutex<AppConfig>,
    pub data_dir: PathBuf,
}

impl AppState {
    fn new(data_dir: PathBuf) -> Self {
        let (sessions, projects, config) = persistence::load_all(&data_dir);
        Self {
            sessions: Mutex::new(sessions),
            projects: Mutex::new(projects),
            config: Mutex::new(config),
            data_dir,
        }
    }

    fn persist(&self) {
        let sessions = self.sessions.lock().unwrap().clone();
        let projects = self.projects.lock().unwrap().clone();
        let config = self.config.lock().unwrap().clone();
        persistence::save_all(&self.data_dir, &sessions, &projects, &config);
    }
}

// ---------------------------------------------------------------------------
// Commands — Sessions
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_sessions(state: State<AppState>) -> Vec<Session> {
    state.sessions.lock().unwrap().clone()
}

#[tauri::command]
fn create_session(state: State<AppState>, title: String, project: Option<String>) -> Session {
    let now = chrono::Utc::now().to_rfc3339();
    let session = Session {
        id: Uuid::new_v4().to_string(),
        title,
        project,
        messages: vec![],
        created_at: now.clone(),
        updated_at: now,
    };
    state.sessions.lock().unwrap().insert(0, session.clone());
    state.persist();
    session
}

#[tauri::command]
fn delete_session(state: State<AppState>, session_id: String) -> bool {
    let mut sessions = state.sessions.lock().unwrap();
    let before = sessions.len();
    sessions.retain(|s| s.id != session_id);
    let deleted = sessions.len() < before;
    drop(sessions);
    if deleted { state.persist(); }
    deleted
}

#[tauri::command]
fn rename_session(state: State<AppState>, session_id: String, title: String) -> bool {
    let mut sessions = state.sessions.lock().unwrap();
    if let Some(s) = sessions.iter_mut().find(|s| s.id == session_id) {
        s.title = title;
        s.updated_at = chrono::Utc::now().to_rfc3339();
        drop(sessions);
        state.persist();
        return true;
    }
    false
}

#[tauri::command]
fn get_session_messages(state: State<AppState>, session_id: String) -> Vec<ChatMessage> {
    state.sessions.lock().unwrap()
        .iter()
        .find(|s| s.id == session_id)
        .map(|s| s.messages.clone())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Commands — Send message (hybrid: local or remote)
// ---------------------------------------------------------------------------

#[tauri::command]
async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    content: String,
) -> Result<ChatMessage, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let user_msg = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".into(),
        content: content.clone(),
        timestamp: now.clone(),
        model: None,
        tool_calls: None,
    };

    // Add user message and auto-title
    {
        let mut sessions = state.sessions.lock().unwrap();
        if let Some(session) = sessions.iter_mut().find(|s| s.id == session_id) {
            session.messages.push(user_msg.clone());
            session.updated_at = now.clone();
            if session.messages.len() == 1 {
                session.title = content.chars().take(30).collect::<String>()
                    + if content.len() > 30 { "..." } else { "" };
            }
        }
    }
    state.persist();

    let mode = state.config.lock().unwrap().mode.clone();
    let api_base = state.config.lock().unwrap().api_base.clone();
    let model = state.config.lock().unwrap().default_model.clone();
    let sid = session_id.clone();

    let reply = if mode == "local" {
        // --- LOCAL MODE: run Agent directly with streaming ---
        let app_clone = app.clone();
        let sid_clone = sid.clone();

        match local_agent::run_agent_streaming(&content, &model, move |delta_type, text, tool_name| {
            eprintln!("[hermes-app] EMIT stream-delta: type={}, len={}", delta_type, text.len());
            let result = app_clone.emit("stream-delta", StreamDelta {
                session_id: sid_clone.clone(),
                delta_type: delta_type.to_string(),
                content: text.to_string(),
                tool_name: tool_name.map(|s| s.to_string()),
            });
            if let Err(e) = result {
                eprintln!("[hermes-app] emit error: {}", e);
            }
        }).await {
            Ok(full_reply) => {
                let _ = app.emit("stream-delta", StreamDelta {
                    session_id: sid.clone(),
                    delta_type: "done".into(),
                    content: String::new(),
                    tool_name: None,
                });
                full_reply
            }
            Err(e) => {
                tracing::warn!("Local agent error: {}", e);
                format!("⚠️ 本地 Agent 执行出错: {}\n\n可以在设置中切换到远程模式。", e)
            }
        }
    } else {
        // --- REMOTE MODE: call hermes-http via WebSocket streaming ---
        let app_clone = app.clone();
        let sid_clone = sid.clone();

        match hermes_client::send_message_stream(&api_base, &sid, &content, move |event_type, text, tool_name| {
            let _ = app_clone.emit("stream-delta", StreamDelta {
                session_id: sid_clone.clone(),
                delta_type: event_type.to_string(),
                content: text.to_string(),
                tool_name: tool_name.map(|s| s.to_string()),
            });
        }).await {
            Ok(response) => {
                let _ = app.emit("stream-delta", StreamDelta {
                    session_id: sid.clone(),
                    delta_type: "done".into(),
                    content: String::new(),
                    tool_name: None,
                });
                response
            }
            Err(e) => {
                // Fall back to HTTP POST
                tracing::warn!("WebSocket streaming failed ({}), trying HTTP", e);
                match hermes_client::send_message(&api_base, &sid, &content).await {
                    Ok(response) => {
                        let _ = app.emit("stream-delta", StreamDelta {
                            session_id: sid.clone(),
                            delta_type: "done".into(),
                            content: response.clone(),
                            tool_name: None,
                        });
                        response
                    }
                    Err(e2) => {
                        format!(
                            "⚠️ 远程后端连接失败 (`{}`)\n\n错误: {}\n\n可以在设置中切换到本地模式。",
                            api_base, e2
                        )
                    }
                }
            }
        }
    };

    // Add assistant message
    let assistant_msg = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".into(),
        content: reply,
        timestamp: chrono::Utc::now().to_rfc3339(),
        model: Some(model),
        tool_calls: None,
    };

    {
        let mut sessions = state.sessions.lock().unwrap();
        if let Some(session) = sessions.iter_mut().find(|s| s.id == session_id) {
            session.messages.push(assistant_msg);
            session.updated_at = chrono::Utc::now().to_rfc3339();
        }
    }
    state.persist();

    Ok(user_msg)
}

// ---------------------------------------------------------------------------
// Commands — Projects
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_projects(state: State<AppState>) -> Vec<Project> {
    state.projects.lock().unwrap().clone()
}

#[tauri::command]
fn add_project(state: State<AppState>, name: String, path: String) -> Project {
    let project = Project { id: Uuid::new_v4().to_string(), name, path };
    state.projects.lock().unwrap().push(project.clone());
    state.persist();
    project
}

#[tauri::command]
fn remove_project(state: State<AppState>, project_id: String) -> bool {
    let mut projects = state.projects.lock().unwrap();
    let before = projects.len();
    projects.retain(|p| p.id != project_id);
    let removed = projects.len() < before;
    drop(projects);
    if removed { state.persist(); }
    removed
}

// ---------------------------------------------------------------------------
// Commands — Config
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_config(state: State<AppState>) -> AppConfig {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn update_config(state: State<AppState>, config: AppConfig) -> AppConfig {
    *state.config.lock().unwrap() = config.clone();
    state.persist();
    config
}

// ---------------------------------------------------------------------------
// Commands — Automation
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_automation_tasks() -> Vec<AutomationTask> {
    vec![
        AutomationTask { id: "1".into(), title: "Summarize yesterday's git activity for standup.".into(), description: "Status reports".into(), icon: "📝".into(), category: "Status reports".into() },
        AutomationTask { id: "2".into(), title: "Synthesize this week's PRs, rollouts, incidents, and reviews into a weekly update.".into(), description: "Status reports".into(), icon: "📋".into(), category: "Status reports".into() },
        AutomationTask { id: "3".into(), title: "Summarize last week's PRs by teammate and theme; highlight risks.".into(), description: "Status reports".into(), icon: "🖥️".into(), category: "Status reports".into() },
        AutomationTask { id: "4".into(), title: "Draft weekly release notes from merged PRs (include links when available).".into(), description: "Release prep".into(), icon: "📮".into(), category: "Release prep".into() },
        AutomationTask { id: "5".into(), title: "Before tagging, verify changelog, migrations, feature flags, and tests.".into(), description: "Release prep".into(), icon: "✅".into(), category: "Release prep".into() },
        AutomationTask { id: "6".into(), title: "Update the changelog with this week's highlights and key PR links.".into(), description: "Release prep".into(), icon: "✏️".into(), category: "Release prep".into() },
        AutomationTask { id: "7".into(), title: "Triage new issues: label, assign, and flag anything blocking.".into(), description: "Incidents & triage".into(), icon: "🔍".into(), category: "Incidents & triage".into() },
        AutomationTask { id: "8".into(), title: "Summarize open incidents and their current status.".into(), description: "Incidents & triage".into(), icon: "💬".into(), category: "Incidents & triage".into() },
    ]
}

// ---------------------------------------------------------------------------
// Commands — Utility
// ---------------------------------------------------------------------------

#[tauri::command]
async fn check_backend_health(state: State<'_, AppState>) -> Result<bool, String> {
    let mode = state.config.lock().unwrap().mode.clone();
    if mode == "local" {
        return Ok(true); // Local mode is always "connected"
    }
    let api_base = state.config.lock().unwrap().api_base.clone();
    hermes_client::health_check(&api_base).await
}

// ---------------------------------------------------------------------------
// App setup
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize hermes config (loads .env, SOUL.md, etc.)
    // Force HERMES_HOME to ~/.hermes if not set
    if std::env::var("HERMES_HOME").is_err() {
        if let Some(home) = dirs::home_dir() {
            let hermes_home = home.join(".hermes");
            if hermes_home.exists() {
                std::env::set_var("HERMES_HOME", hermes_home.to_string_lossy().as_ref());
            }
        }
    }
    hermes_config::loader::load_dotenv();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            std::fs::create_dir_all(&data_dir).ok();
            app.manage(AppState::new(data_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_sessions,
            create_session,
            delete_session,
            rename_session,
            get_session_messages,
            send_message,
            get_projects,
            add_project,
            remove_project,
            get_config,
            update_config,
            get_automation_tasks,
            check_backend_health,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hermes App");
}

//! Training / benchmark loops.
//!
//! This module provides:
//! - [`HermesBaseEnv`] — trait for training environments (SWE-bench, web research, etc.)
//! - [`HermesEpisode`] — a single training episode (task + trajectory)
//! - [`Trajectory`] — recorded sequence of (observation, action, reward) tuples
//! - [`parsers`] — tool call parsers for various LLM output formats
//! - [`benchmarks`] — benchmark-specific environment implementations

pub mod benchmarks;
pub mod parsers;

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Trajectory recording
// ---------------------------------------------------------------------------

/// A single step in a trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryStep {
    /// Step index (0-based).
    pub step: usize,
    /// Observation presented to the agent (may be truncated for storage).
    pub observation: String,
    /// Action taken by the agent (tool call JSON or text response).
    pub action: String,
    /// Tool name if this was a tool call.
    pub tool_name: Option<String>,
    /// Tool parameters if this was a tool call.
    pub tool_params: Option<serde_json::Value>,
    /// Tool result if this was a tool call.
    pub tool_result: Option<String>,
    /// Reward signal (0.0 for intermediate steps, final reward on last step).
    pub reward: f64,
    /// Whether this step terminated the episode.
    pub done: bool,
    /// Timestamp.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Token usage for this step.
    pub tokens_input: u64,
    pub tokens_output: u64,
}

/// A complete trajectory for one episode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trajectory {
    pub task_id: String,
    pub model: String,
    pub steps: Vec<TrajectoryStep>,
    pub total_reward: f64,
    pub success: bool,
    pub duration: Duration,
    pub total_tokens_input: u64,
    pub total_tokens_output: u64,
    pub total_cost_usd: f64,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Trajectory {
    pub fn new(task_id: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            model: model.into(),
            steps: Vec::new(),
            total_reward: 0.0,
            success: false,
            duration: Duration::ZERO,
            total_tokens_input: 0,
            total_tokens_output: 0,
            total_cost_usd: 0.0,
            metadata: HashMap::new(),
        }
    }

    pub fn add_step(&mut self, step: TrajectoryStep) {
        self.total_tokens_input += step.tokens_input;
        self.total_tokens_output += step.tokens_output;
        self.total_reward += step.reward;
        if step.done {
            self.success = step.reward > 0.0;
        }
        self.steps.push(step);
    }

    pub fn num_steps(&self) -> usize {
        self.steps.len()
    }
}

// ---------------------------------------------------------------------------
// Environment traits
// ---------------------------------------------------------------------------

/// Configuration for a training environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvConfig {
    /// Dataset identifier (e.g. HuggingFace slug).
    pub dataset_id: String,
    /// Model to use for the agent.
    pub model: String,
    /// Maximum steps per episode.
    pub max_steps: usize,
    /// Timeout per episode.
    pub timeout: Duration,
    /// Tools available to the agent.
    pub tools: Vec<String>,
    /// Extra config (benchmark-specific).
    #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Default for EnvConfig {
    fn default() -> Self {
        Self {
            dataset_id: String::new(),
            model: "anthropic:claude-3-5-sonnet-20241022".into(),
            max_steps: 30,
            timeout: Duration::from_secs(300),
            tools: vec![
                "terminal".into(),
                "read_file".into(),
                "write_file".into(),
                "search_files".into(),
                "patch".into(),
            ],
            extra: HashMap::new(),
        }
    }
}

/// A single task from a training dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvTask {
    pub task_id: String,
    pub instruction: String,
    pub category: Option<String>,
    /// Ground truth for verification (test script, expected output, etc.).
    pub ground_truth: Option<serde_json::Value>,
    /// Extra context (repo URL, Docker image, etc.).
    #[serde(default)]
    pub context: HashMap<String, serde_json::Value>,
}

/// Placeholder for a training or benchmark episode.
pub trait HermesEpisode: Send {
    /// Stable task id for logging.
    fn task_id(&self) -> &str;
}

/// Environment that loads tasks, runs the agent loop, emits trajectories.
///
/// Implementations may wrap Docker, local shell, or remote runners.
#[async_trait]
pub trait HermesBaseEnv: Send + Sync {
    /// Dataset / benchmark identifier.
    fn dataset_id(&self) -> &str;

    /// Load all tasks from the dataset.
    async fn load_tasks(&self) -> Result<Vec<EnvTask>, Box<dyn std::error::Error + Send + Sync>>;

    /// Set up the environment for a specific task (e.g. clone repo, start Docker).
    async fn setup_task(&self, task: &EnvTask) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Tear down the environment after a task.
    async fn teardown_task(&self, task: &EnvTask) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Verify the agent's output for a task.
    async fn verify(
        &self,
        task: &EnvTask,
        trajectory: &Trajectory,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync>>;
}

// ---------------------------------------------------------------------------
// Dataset loading via Python subprocess (fallback)
// ---------------------------------------------------------------------------

/// Load a HuggingFace dataset via Python subprocess.
///
/// This avoids reimplementing the `datasets` library in Rust. The Python
/// script outputs JSON lines to stdout.
pub async fn load_hf_dataset_via_python(
    dataset_id: &str,
    split: &str,
    max_samples: Option<usize>,
) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error + Send + Sync>> {
    let limit = max_samples.unwrap_or(usize::MAX);
    let script = format!(
        r#"
import json, sys
try:
    from datasets import load_dataset
    ds = load_dataset("{dataset_id}", split="{split}")
    for i, item in enumerate(ds):
        if i >= {limit}:
            break
        print(json.dumps(dict(item), default=str))
except Exception as e:
    print(json.dumps({{"error": str(e)}}), file=sys.stderr)
    sys.exit(1)
"#,
    );

    let output = tokio::process::Command::new("python3")
        .args(["-c", &script])
        .output()
        .await
        .map_err(|e| format!("Failed to run python3: {e}. Is Python installed?"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Python dataset load failed: {stderr}").into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let items: Vec<serde_json::Value> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trajectory_recording() {
        let mut traj = Trajectory::new("task-1", "test-model");
        traj.add_step(TrajectoryStep {
            step: 0,
            observation: "Initial state".into(),
            action: "ls -la".into(),
            tool_name: Some("terminal".into()),
            tool_params: Some(serde_json::json!({"command": "ls -la"})),
            tool_result: Some("file1.txt\nfile2.txt".into()),
            reward: 0.0,
            done: false,
            timestamp: chrono::Utc::now(),
            tokens_input: 100,
            tokens_output: 50,
        });
        traj.add_step(TrajectoryStep {
            step: 1,
            observation: "file1.txt\nfile2.txt".into(),
            action: "Done".into(),
            tool_name: None,
            tool_params: None,
            tool_result: None,
            reward: 1.0,
            done: true,
            timestamp: chrono::Utc::now(),
            tokens_input: 80,
            tokens_output: 20,
        });

        assert_eq!(traj.num_steps(), 2);
        assert_eq!(traj.total_reward, 1.0);
        assert!(traj.success);
        assert_eq!(traj.total_tokens_input, 180);
        assert_eq!(traj.total_tokens_output, 70);
    }

    #[test]
    fn trajectory_serialization() {
        let traj = Trajectory::new("task-1", "model-a");
        let json = serde_json::to_string(&traj).unwrap();
        let parsed: Trajectory = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.task_id, "task-1");
        assert_eq!(parsed.model, "model-a");
    }

    #[test]
    fn env_config_default() {
        let cfg = EnvConfig::default();
        assert_eq!(cfg.max_steps, 30);
        assert!(cfg.tools.contains(&"terminal".to_string()));
    }
}

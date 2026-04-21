//! Terminal-Bench 2 environment.
//!
//! Evaluates agent ability to perform terminal/shell tasks: file manipulation,
//! system administration, scripting, etc.
//!
//! Dataset: `NousResearch/terminal-bench-2`

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::super::{load_hf_dataset_via_python, EnvTask, HermesBaseEnv, Trajectory};

/// Terminal-Bench 2 task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalBenchTask {
    pub task_id: String,
    pub instruction: String,
    pub category: String,
    /// Expected output or verification script.
    pub expected: String,
    /// Setup commands to run before the task.
    #[serde(default)]
    pub setup_commands: Vec<String>,
    /// Verification commands (exit 0 = pass).
    #[serde(default)]
    pub verify_commands: Vec<String>,
    /// Difficulty level.
    #[serde(default)]
    pub difficulty: String,
}

/// Terminal-Bench 2 configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalBenchConfig {
    pub dataset: String,
    pub split: String,
    pub max_tasks: Option<usize>,
    pub work_dir: PathBuf,
    pub timeout: Duration,
}

impl Default for TerminalBenchConfig {
    fn default() -> Self {
        Self {
            dataset: "NousResearch/terminal-bench-2".into(),
            split: "test".into(),
            max_tasks: None,
            work_dir: PathBuf::from("/tmp/hermes-terminal-bench"),
            timeout: Duration::from_secs(120),
        }
    }
}

/// Terminal-Bench 2 environment.
pub struct TerminalBenchEnv {
    config: TerminalBenchConfig,
}

impl TerminalBenchEnv {
    pub fn new(config: TerminalBenchConfig) -> Self {
        Self { config }
    }

    fn parse_task(row: &serde_json::Value) -> Option<TerminalBenchTask> {
        Some(TerminalBenchTask {
            task_id: row
                .get("task_id")
                .or_else(|| row.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            instruction: row
                .get("instruction")
                .or_else(|| row.get("prompt"))
                .and_then(|v| v.as_str())?
                .to_string(),
            category: row
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("general")
                .to_string(),
            expected: row
                .get("expected")
                .or_else(|| row.get("answer"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            setup_commands: row
                .get("setup_commands")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            verify_commands: row
                .get("verify_commands")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            difficulty: row
                .get("difficulty")
                .and_then(|v| v.as_str())
                .unwrap_or("medium")
                .to_string(),
        })
    }
}

#[async_trait]
impl HermesBaseEnv for TerminalBenchEnv {
    fn dataset_id(&self) -> &str {
        &self.config.dataset
    }

    async fn load_tasks(&self) -> Result<Vec<EnvTask>, Box<dyn std::error::Error + Send + Sync>> {
        let rows = load_hf_dataset_via_python(
            &self.config.dataset,
            &self.config.split,
            self.config.max_tasks,
        )
        .await?;

        let tasks: Vec<EnvTask> = rows
            .iter()
            .filter_map(|row| {
                let tb_task = Self::parse_task(row)?;
                Some(EnvTask {
                    task_id: tb_task.task_id.clone(),
                    instruction: tb_task.instruction.clone(),
                    category: Some(tb_task.category.clone()),
                    ground_truth: Some(serde_json::to_value(&tb_task).ok()?),
                    context: HashMap::new(),
                })
            })
            .collect();

        tracing::info!(count = tasks.len(), "Loaded Terminal-Bench 2 tasks");
        Ok(tasks)
    }

    async fn setup_task(&self, task: &EnvTask) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let task_dir = self.config.work_dir.join(&task.task_id);
        tokio::fs::create_dir_all(&task_dir).await?;

        // Run setup commands
        if let Some(gt) = &task.ground_truth {
            let tb_task: TerminalBenchTask = serde_json::from_value(gt.clone())?;
            for cmd in &tb_task.setup_commands {
                let output = tokio::process::Command::new("sh")
                    .args(["-c", cmd])
                    .current_dir(&task_dir)
                    .output()
                    .await?;
                if !output.status.success() {
                    tracing::warn!(cmd = cmd, "Setup command failed");
                }
            }
        }
        Ok(())
    }

    async fn teardown_task(&self, task: &EnvTask) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let task_dir = self.config.work_dir.join(&task.task_id);
        if task_dir.exists() {
            tokio::fs::remove_dir_all(&task_dir).await?;
        }
        Ok(())
    }

    async fn verify(
        &self,
        task: &EnvTask,
        _trajectory: &Trajectory,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        let task_dir = self.config.work_dir.join(&task.task_id);

        if let Some(gt) = &task.ground_truth {
            let tb_task: TerminalBenchTask = serde_json::from_value(gt.clone())?;

            // Run verification commands
            if !tb_task.verify_commands.is_empty() {
                for cmd in &tb_task.verify_commands {
                    let output = tokio::time::timeout(
                        Duration::from_secs(30),
                        tokio::process::Command::new("sh")
                            .args(["-c", cmd])
                            .current_dir(&task_dir)
                            .output(),
                    )
                    .await;

                    match output {
                        Ok(Ok(o)) if o.status.success() => continue,
                        _ => return Ok(0.0),
                    }
                }
                return Ok(1.0);
            }
        }

        Ok(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_terminal_bench_task() {
        let row = serde_json::json!({
            "task_id": "tb2_001",
            "instruction": "Create a file named test.txt with content 'hello'",
            "category": "file_ops",
            "expected": "hello",
            "difficulty": "easy",
        });
        let task = TerminalBenchEnv::parse_task(&row).unwrap();
        assert_eq!(task.task_id, "tb2_001");
        assert_eq!(task.category, "file_ops");
        assert_eq!(task.difficulty, "easy");
    }

    #[test]
    fn default_config() {
        let cfg = TerminalBenchConfig::default();
        assert!(cfg.dataset.contains("terminal-bench"));
    }
}

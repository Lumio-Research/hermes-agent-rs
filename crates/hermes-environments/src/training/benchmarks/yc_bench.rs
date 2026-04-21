//! YC Bench environment.
//!
//! Evaluates agent ability to complete practical engineering/product tasks
//! inspired by Y Combinator startup scenarios.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::super::{load_hf_dataset_via_python, EnvTask, HermesBaseEnv, Trajectory};

/// YC Bench task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YcBenchTask {
    pub task_id: String,
    pub instruction: String,
    pub category: String,
    /// Verification criteria (checklist items).
    #[serde(default)]
    pub criteria: Vec<String>,
    /// Setup script.
    #[serde(default)]
    pub setup_script: String,
    /// Verification script (exit 0 = pass).
    #[serde(default)]
    pub verify_script: String,
}

/// YC Bench configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YcBenchConfig {
    pub dataset: String,
    pub split: String,
    pub max_tasks: Option<usize>,
    pub work_dir: PathBuf,
    pub timeout: Duration,
}

impl Default for YcBenchConfig {
    fn default() -> Self {
        Self {
            dataset: "NousResearch/yc-bench".into(),
            split: "test".into(),
            max_tasks: None,
            work_dir: PathBuf::from("/tmp/hermes-yc-bench"),
            timeout: Duration::from_secs(300),
        }
    }
}

/// YC Bench environment.
pub struct YcBenchEnv {
    config: YcBenchConfig,
}

impl YcBenchEnv {
    pub fn new(config: YcBenchConfig) -> Self {
        Self { config }
    }

    fn parse_task(row: &serde_json::Value) -> Option<YcBenchTask> {
        Some(YcBenchTask {
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
            criteria: row
                .get("criteria")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            setup_script: row
                .get("setup_script")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            verify_script: row
                .get("verify_script")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
    }
}

#[async_trait]
impl HermesBaseEnv for YcBenchEnv {
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
                let yc_task = Self::parse_task(row)?;
                Some(EnvTask {
                    task_id: yc_task.task_id.clone(),
                    instruction: yc_task.instruction.clone(),
                    category: Some(yc_task.category.clone()),
                    ground_truth: Some(serde_json::to_value(&yc_task).ok()?),
                    context: HashMap::new(),
                })
            })
            .collect();

        tracing::info!(count = tasks.len(), "Loaded YC Bench tasks");
        Ok(tasks)
    }

    async fn setup_task(
        &self,
        task: &EnvTask,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let task_dir = self.config.work_dir.join(&task.task_id);
        tokio::fs::create_dir_all(&task_dir).await?;

        if let Some(gt) = &task.ground_truth {
            let yc_task: YcBenchTask = serde_json::from_value(gt.clone())?;
            if !yc_task.setup_script.is_empty() {
                let output = tokio::process::Command::new("sh")
                    .args(["-c", &yc_task.setup_script])
                    .current_dir(&task_dir)
                    .output()
                    .await?;
                if !output.status.success() {
                    tracing::warn!(task_id = %task.task_id, "Setup script failed");
                }
            }
        }
        Ok(())
    }

    async fn teardown_task(
        &self,
        task: &EnvTask,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
            let yc_task: YcBenchTask = serde_json::from_value(gt.clone())?;
            if !yc_task.verify_script.is_empty() {
                let output = tokio::time::timeout(
                    Duration::from_secs(60),
                    tokio::process::Command::new("sh")
                        .args(["-c", &yc_task.verify_script])
                        .current_dir(&task_dir)
                        .output(),
                )
                .await;

                return match output {
                    Ok(Ok(o)) if o.status.success() => Ok(1.0),
                    _ => Ok(0.0),
                };
            }
        }
        Ok(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_yc_bench_task() {
        let row = serde_json::json!({
            "task_id": "yc_001",
            "instruction": "Build a REST API for user management",
            "category": "backend",
            "criteria": ["Has /users endpoint", "Returns JSON"],
        });
        let task = YcBenchEnv::parse_task(&row).unwrap();
        assert_eq!(task.task_id, "yc_001");
        assert_eq!(task.criteria.len(), 2);
    }

    #[test]
    fn default_config() {
        let cfg = YcBenchConfig::default();
        assert!(cfg.dataset.contains("yc-bench"));
    }
}

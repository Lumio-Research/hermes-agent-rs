//! SWE-bench environment.
//!
//! Loads tasks from the SWE-bench dataset (via Python subprocess for HuggingFace
//! `datasets` library), sets up a git repo per task, runs the agent, and verifies
//! by executing the task's test suite.
//!
//! Dataset: `princeton-nlp/SWE-bench_Verified` (or `SWE-bench_Lite`)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::super::{load_hf_dataset_via_python, EnvTask, HermesBaseEnv, Trajectory};

/// SWE-bench task metadata (matches HuggingFace dataset columns).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweBenchTask {
    pub instance_id: String,
    pub repo: String,
    pub base_commit: String,
    pub problem_statement: String,
    #[serde(default)]
    pub hints_text: String,
    #[serde(default)]
    pub test_patch: String,
    #[serde(default)]
    pub patch: String,
    #[serde(default)]
    pub version: String,
}

/// SWE-bench environment configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweBenchConfig {
    /// HuggingFace dataset id.
    pub dataset: String,
    /// Dataset split (e.g. "test").
    pub split: String,
    /// Maximum number of tasks to load.
    pub max_tasks: Option<usize>,
    /// Working directory for cloned repos.
    pub work_dir: PathBuf,
    /// Whether to use Docker for isolation.
    pub use_docker: bool,
    /// Timeout per task.
    pub timeout: Duration,
}

impl Default for SweBenchConfig {
    fn default() -> Self {
        Self {
            dataset: "princeton-nlp/SWE-bench_Verified".into(),
            split: "test".into(),
            max_tasks: None,
            work_dir: PathBuf::from("/tmp/hermes-swe-bench"),
            use_docker: false,
            timeout: Duration::from_secs(600),
        }
    }
}

/// SWE-bench environment.
pub struct SweBenchEnv {
    config: SweBenchConfig,
}

impl SweBenchEnv {
    pub fn new(config: SweBenchConfig) -> Self {
        Self { config }
    }

    /// Parse a raw HuggingFace dataset row into a SweBenchTask.
    fn parse_task(row: &serde_json::Value) -> Option<SweBenchTask> {
        Some(SweBenchTask {
            instance_id: row.get("instance_id")?.as_str()?.to_string(),
            repo: row.get("repo")?.as_str()?.to_string(),
            base_commit: row.get("base_commit")?.as_str()?.to_string(),
            problem_statement: row.get("problem_statement")?.as_str()?.to_string(),
            hints_text: row
                .get("hints_text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            test_patch: row
                .get("test_patch")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            patch: row
                .get("patch")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            version: row
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
    }

    /// Clone the repo and checkout the base commit for a task.
    async fn setup_repo(
        &self,
        task: &SweBenchTask,
    ) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        let repo_dir = self.config.work_dir.join(&task.instance_id);
        if repo_dir.exists() {
            tokio::fs::remove_dir_all(&repo_dir).await?;
        }
        tokio::fs::create_dir_all(&repo_dir).await?;

        let repo_url = format!("https://github.com/{}.git", task.repo);

        // Shallow clone + checkout base commit
        let output = tokio::process::Command::new("git")
            .args([
                "clone",
                "--depth",
                "50",
                &repo_url,
                repo_dir.to_str().unwrap(),
            ])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git clone failed: {stderr}").into());
        }

        let output = tokio::process::Command::new("git")
            .args(["checkout", &task.base_commit])
            .current_dir(&repo_dir)
            .output()
            .await?;

        if !output.status.success() {
            tracing::warn!(
                instance_id = %task.instance_id,
                "Could not checkout base commit, using HEAD"
            );
        }

        Ok(repo_dir)
    }

    /// Apply the test patch and run tests to verify the agent's changes.
    async fn run_tests(
        &self,
        repo_dir: &Path,
        task: &SweBenchTask,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if task.test_patch.is_empty() {
            tracing::warn!(instance_id = %task.instance_id, "No test patch available");
            return Ok(false);
        }

        // Apply test patch
        let test_patch_path = repo_dir.join("__test_patch.diff");
        tokio::fs::write(&test_patch_path, &task.test_patch).await?;

        let output = tokio::process::Command::new("git")
            .args(["apply", "--check", "__test_patch.diff"])
            .current_dir(repo_dir)
            .output()
            .await?;

        if output.status.success() {
            tokio::process::Command::new("git")
                .args(["apply", "__test_patch.diff"])
                .current_dir(repo_dir)
                .output()
                .await?;
        }

        // Try running tests (Python projects typically use pytest)
        let test_output = tokio::time::timeout(
            Duration::from_secs(120),
            tokio::process::Command::new("python3")
                .args(["-m", "pytest", "--tb=short", "-q"])
                .current_dir(repo_dir)
                .output(),
        )
        .await;

        match test_output {
            Ok(Ok(output)) => Ok(output.status.success()),
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "Test execution failed");
                Ok(false)
            }
            Err(_) => {
                tracing::warn!("Test execution timed out");
                Ok(false)
            }
        }
    }
}

#[async_trait]
impl HermesBaseEnv for SweBenchEnv {
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
                let swe_task = Self::parse_task(row)?;
                Some(EnvTask {
                    task_id: swe_task.instance_id.clone(),
                    instruction: swe_task.problem_statement.clone(),
                    category: Some(swe_task.repo.clone()),
                    ground_truth: Some(serde_json::to_value(&swe_task).ok()?),
                    context: {
                        let mut ctx = HashMap::new();
                        ctx.insert(
                            "repo".into(),
                            serde_json::Value::String(swe_task.repo.clone()),
                        );
                        ctx.insert(
                            "base_commit".into(),
                            serde_json::Value::String(swe_task.base_commit.clone()),
                        );
                        ctx
                    },
                })
            })
            .collect();

        tracing::info!(count = tasks.len(), dataset = %self.config.dataset, "Loaded SWE-bench tasks");
        Ok(tasks)
    }

    async fn setup_task(
        &self,
        task: &EnvTask,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(gt) = &task.ground_truth {
            let swe_task: SweBenchTask = serde_json::from_value(gt.clone())?;
            self.setup_repo(&swe_task).await?;
        }
        Ok(())
    }

    async fn teardown_task(
        &self,
        task: &EnvTask,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let repo_dir = self.config.work_dir.join(&task.task_id);
        if repo_dir.exists() {
            tokio::fs::remove_dir_all(&repo_dir).await?;
        }
        Ok(())
    }

    async fn verify(
        &self,
        task: &EnvTask,
        _trajectory: &Trajectory,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        let repo_dir = self.config.work_dir.join(&task.task_id);
        if let Some(gt) = &task.ground_truth {
            let swe_task: SweBenchTask = serde_json::from_value(gt.clone())?;
            let passed = self.run_tests(&repo_dir, &swe_task).await?;
            Ok(if passed { 1.0 } else { 0.0 })
        } else {
            Ok(0.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_swe_bench_task() {
        let row = serde_json::json!({
            "instance_id": "django__django-12345",
            "repo": "django/django",
            "base_commit": "abc123",
            "problem_statement": "Fix the bug in models.py",
            "test_patch": "diff --git a/test.py ...",
            "patch": "diff --git a/models.py ...",
        });
        let task = SweBenchEnv::parse_task(&row).unwrap();
        assert_eq!(task.instance_id, "django__django-12345");
        assert_eq!(task.repo, "django/django");
        assert!(task.problem_statement.contains("Fix the bug"));
    }

    #[test]
    fn default_config() {
        let cfg = SweBenchConfig::default();
        assert!(cfg.dataset.contains("SWE-bench"));
        assert_eq!(cfg.split, "test");
    }
}

//! Mixture of Agents — multi-provider parallel prompt + aggregation.
//!
//! This tool sends the same prompt to multiple LLM providers in parallel,
//! collects their responses, and then uses an aggregator model to synthesize
//! a final answer from all responses.
//!
//! Strategies:
//! - **aggregate** (default): Send to N models, then have an aggregator model
//!   read all responses and produce a unified answer.
//! - **majority_vote**: Send to N models, pick the most common answer.
//! - **best_of_n**: Send to N models, have the aggregator pick the best one.
//!
//! Architecture:
//! ```text
//!   prompt ──┬──→ Provider A ──→ response_a ──┐
//!            ├──→ Provider B ──→ response_b ──┤──→ Aggregator ──→ final answer
//!            └──→ Provider C ──→ response_c ──┘
//! ```
//!
//! The tool does NOT hold its own LLM clients. Instead it accepts an
//! `MoaBackend` trait that the agent layer injects (backed by the real
//! `GenericProvider` / `CredentialPool` infrastructure).

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use hermes_core::{tool_schema, JsonSchema, ToolError, ToolHandler, ToolSchema};

// ---------------------------------------------------------------------------
// Backend trait
// ---------------------------------------------------------------------------

/// A single model's response in the MoA pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoaResponse {
    /// Provider:model identifier (e.g. "openai:gpt-4o").
    pub model: String,
    /// The model's text response.
    pub text: String,
    /// Latency in milliseconds.
    pub latency_ms: u64,
    /// Estimated cost in USD (0.0 if unknown).
    pub cost_usd: f64,
    /// Token usage if available.
    pub tokens_used: Option<u64>,
}

/// Backend trait for making LLM calls. The agent layer provides a real
/// implementation backed by `GenericProvider` / `CredentialPool`.
#[async_trait]
pub trait MoaBackend: Send + Sync {
    /// Send a prompt to a specific model and return the response.
    async fn query_model(
        &self,
        model: &str,
        system_prompt: Option<&str>,
        user_prompt: &str,
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<MoaResponse, ToolError>;
}

// ---------------------------------------------------------------------------
// Aggregation strategies
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MoaStrategy {
    /// Have an aggregator model synthesize all responses into one.
    Aggregate,
    /// Pick the most common answer (simple text equality).
    MajorityVote,
    /// Have the aggregator pick the single best response.
    BestOfN,
}

impl Default for MoaStrategy {
    fn default() -> Self {
        Self::Aggregate
    }
}

/// Configuration for the MoA pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoaConfig {
    /// Models to query in parallel (e.g. ["openai:gpt-4o", "anthropic:claude-3-5-sonnet", "openrouter:google/gemini-pro"]).
    pub models: Vec<String>,
    /// Strategy for combining responses.
    pub strategy: MoaStrategy,
    /// Model to use for aggregation (only used with Aggregate / BestOfN).
    /// Defaults to the first model in `models`.
    pub aggregator_model: Option<String>,
    /// Temperature for the worker models.
    pub temperature: Option<f64>,
    /// Temperature for the aggregator model.
    pub aggregator_temperature: Option<f64>,
    /// Max tokens per worker response.
    pub max_tokens: Option<u32>,
    /// Timeout per model call in seconds.
    pub timeout_secs: u64,
}

impl Default for MoaConfig {
    fn default() -> Self {
        Self {
            models: vec![
                "openai:gpt-4o".into(),
                "anthropic:claude-3-5-sonnet-20241022".into(),
            ],
            strategy: MoaStrategy::Aggregate,
            aggregator_model: None,
            temperature: Some(0.7),
            aggregator_temperature: Some(0.3),
            max_tokens: Some(4096),
            timeout_secs: 60,
        }
    }
}

// ---------------------------------------------------------------------------
// MoA execution engine
// ---------------------------------------------------------------------------

/// Result of a full MoA pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoaResult {
    pub final_answer: String,
    pub strategy: MoaStrategy,
    pub responses: Vec<MoaResponse>,
    pub aggregator_model: Option<String>,
    pub total_latency_ms: u64,
    pub total_cost_usd: f64,
    pub total_tokens: u64,
}

/// Run the MoA pipeline: parallel query → aggregate.
pub async fn run_mixture_of_agents(
    backend: &dyn MoaBackend,
    config: &MoaConfig,
    prompt: &str,
    system_prompt: Option<&str>,
) -> Result<MoaResult, ToolError> {
    if config.models.is_empty() {
        return Err(ToolError::InvalidParams(
            "mixture_of_agents requires at least one model".into(),
        ));
    }

    let start = Instant::now();
    let timeout = Duration::from_secs(config.timeout_secs);

    // Phase 1: Query all models in parallel
    let futures: Vec<_> = config
        .models
        .iter()
        .map(|model| {
            let model = model.clone();
            let prompt = prompt.to_string();
            let system = system_prompt.map(|s| s.to_string());
            let temp = config.temperature;
            let max_tok = config.max_tokens;
            async move {
                let result = backend
                    .query_model(
                        &model,
                        system.as_deref(),
                        &prompt,
                        temp,
                        max_tok,
                    )
                    .await;
                (model, result)
            }
        })
        .collect();

    let results = tokio::time::timeout(timeout, futures::future::join_all(futures))
        .await
        .map_err(|_| {
            ToolError::ExecutionFailed(format!(
                "MoA timed out after {}s waiting for model responses",
                config.timeout_secs
            ))
        })?;

    // Collect successful responses, log failures
    let mut responses: Vec<MoaResponse> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for (model, result) in results {
        match result {
            Ok(resp) => responses.push(resp),
            Err(e) => {
                tracing::warn!(model = %model, error = %e, "MoA: model query failed");
                errors.push(format!("{model}: {e}"));
            }
        }
    }

    if responses.is_empty() {
        return Err(ToolError::ExecutionFailed(format!(
            "All MoA models failed: {}",
            errors.join("; ")
        )));
    }

    // Phase 2: Aggregate
    let final_answer = match config.strategy {
        MoaStrategy::MajorityVote => majority_vote(&responses),
        MoaStrategy::Aggregate => {
            let aggregator = config
                .aggregator_model
                .as_deref()
                .unwrap_or(&config.models[0]);
            aggregate_responses(
                backend,
                aggregator,
                prompt,
                &responses,
                config.aggregator_temperature,
                config.max_tokens,
            )
            .await?
        }
        MoaStrategy::BestOfN => {
            let aggregator = config
                .aggregator_model
                .as_deref()
                .unwrap_or(&config.models[0]);
            best_of_n(
                backend,
                aggregator,
                prompt,
                &responses,
                config.aggregator_temperature,
                config.max_tokens,
            )
            .await?
        }
    };

    let total_cost: f64 = responses.iter().map(|r| r.cost_usd).sum();
    let total_tokens: u64 = responses.iter().filter_map(|r| r.tokens_used).sum();
    let total_latency = start.elapsed().as_millis() as u64;

    Ok(MoaResult {
        final_answer,
        strategy: config.strategy.clone(),
        responses,
        aggregator_model: config.aggregator_model.clone(),
        total_latency_ms: total_latency,
        total_cost_usd: total_cost,
        total_tokens,
    })
}

/// Simple majority vote: pick the most common response text.
/// Falls back to the first response if all are unique.
fn majority_vote(responses: &[MoaResponse]) -> String {
    use std::collections::HashMap;
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for r in responses {
        *counts.entry(r.text.trim()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(text, _)| text.to_string())
        .unwrap_or_default()
}

/// Aggregate strategy: send all responses to an aggregator model.
async fn aggregate_responses(
    backend: &dyn MoaBackend,
    aggregator_model: &str,
    original_prompt: &str,
    responses: &[MoaResponse],
    temperature: Option<f64>,
    max_tokens: Option<u32>,
) -> Result<String, ToolError> {
    let responses_text = responses
        .iter()
        .enumerate()
        .map(|(i, r)| format!("--- Response {} (from {}) ---\n{}", i + 1, r.model, r.text))
        .collect::<Vec<_>>()
        .join("\n\n");

    let aggregation_prompt = format!(
        "You have been given the following question/prompt:\n\n\
         {original_prompt}\n\n\
         Multiple AI models have provided the following responses:\n\n\
         {responses_text}\n\n\
         Please synthesize these responses into a single, comprehensive, and accurate answer. \
         Combine the best insights from each response, resolve any contradictions by favoring \
         the most well-reasoned arguments, and produce a clear final answer."
    );

    let system = "You are an expert aggregator. Your job is to synthesize multiple AI responses \
                  into one optimal answer. Be thorough but concise.";

    let resp = backend
        .query_model(
            aggregator_model,
            Some(system),
            &aggregation_prompt,
            temperature,
            max_tokens,
        )
        .await?;

    Ok(resp.text)
}

/// Best-of-N strategy: have the aggregator pick the single best response.
async fn best_of_n(
    backend: &dyn MoaBackend,
    aggregator_model: &str,
    original_prompt: &str,
    responses: &[MoaResponse],
    temperature: Option<f64>,
    max_tokens: Option<u32>,
) -> Result<String, ToolError> {
    let responses_text = responses
        .iter()
        .enumerate()
        .map(|(i, r)| format!("--- Response {} (from {}) ---\n{}", i + 1, r.model, r.text))
        .collect::<Vec<_>>()
        .join("\n\n");

    let selection_prompt = format!(
        "You have been given the following question/prompt:\n\n\
         {original_prompt}\n\n\
         Multiple AI models have provided the following responses:\n\n\
         {responses_text}\n\n\
         Please select the BEST response and return it verbatim. Choose based on accuracy, \
         completeness, clarity, and helpfulness. Output ONLY the selected response text, \
         nothing else."
    );

    let system = "You are a response quality judge. Select the single best response and \
                  return it exactly as written.";

    let resp = backend
        .query_model(
            aggregator_model,
            Some(system),
            &selection_prompt,
            temperature,
            max_tokens,
        )
        .await?;

    Ok(resp.text)
}

// ---------------------------------------------------------------------------
// Tool Handler
// ---------------------------------------------------------------------------

/// Tool handler for mixture-of-agents. The LLM invokes this to run a
/// multi-model query with aggregation.
pub struct MixtureOfAgentsHandler {
    backend: Arc<dyn MoaBackend>,
    default_config: MoaConfig,
}

impl MixtureOfAgentsHandler {
    pub fn new(backend: Arc<dyn MoaBackend>, config: MoaConfig) -> Self {
        Self {
            backend,
            default_config: config,
        }
    }

    /// Build config from tool params, falling back to defaults.
    fn build_config(&self, params: &Value) -> MoaConfig {
        let mut cfg = self.default_config.clone();

        if let Some(models) = params.get("models").and_then(|v| v.as_array()) {
            let parsed: Vec<String> = models
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if !parsed.is_empty() {
                cfg.models = parsed;
            }
        }

        if let Some(strategy) = params.get("strategy").and_then(|v| v.as_str()) {
            cfg.strategy = match strategy {
                "majority_vote" => MoaStrategy::MajorityVote,
                "best_of_n" => MoaStrategy::BestOfN,
                _ => MoaStrategy::Aggregate,
            };
        }

        if let Some(agg) = params.get("aggregator_model").and_then(|v| v.as_str()) {
            cfg.aggregator_model = Some(agg.to_string());
        }

        if let Some(temp) = params.get("temperature").and_then(|v| v.as_f64()) {
            cfg.temperature = Some(temp);
        }

        if let Some(max) = params.get("max_tokens").and_then(|v| v.as_u64()) {
            cfg.max_tokens = Some(max as u32);
        }

        if let Some(timeout) = params.get("timeout_secs").and_then(|v| v.as_u64()) {
            cfg.timeout_secs = timeout;
        }

        cfg
    }
}

#[async_trait]
impl ToolHandler for MixtureOfAgentsHandler {
    async fn execute(&self, params: Value) -> Result<String, ToolError> {
        let prompt = params
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if prompt.is_empty() {
            return Err(ToolError::InvalidParams("Missing 'prompt'".into()));
        }

        let system_prompt = params.get("system_prompt").and_then(|v| v.as_str());
        let config = self.build_config(&params);

        tracing::info!(
            models = ?config.models,
            strategy = ?config.strategy,
            "Running mixture-of-agents"
        );

        let result =
            run_mixture_of_agents(self.backend.as_ref(), &config, prompt, system_prompt).await?;

        Ok(json!({
            "status": "completed",
            "strategy": format!("{:?}", result.strategy),
            "final_answer": result.final_answer,
            "models_queried": result.responses.iter().map(|r| &r.model).collect::<Vec<_>>(),
            "model_responses": result.responses.iter().map(|r| json!({
                "model": r.model,
                "text_preview": if r.text.len() > 200 {
                    format!("{}...", &r.text[..200])
                } else {
                    r.text.clone()
                },
                "latency_ms": r.latency_ms,
                "cost_usd": r.cost_usd,
            })).collect::<Vec<_>>(),
            "aggregator_model": result.aggregator_model,
            "total_latency_ms": result.total_latency_ms,
            "total_cost_usd": result.total_cost_usd,
            "total_tokens": result.total_tokens,
        })
        .to_string())
    }

    fn schema(&self) -> ToolSchema {
        let mut props = IndexMap::new();
        props.insert(
            "prompt".into(),
            json!({
                "type": "string",
                "description": "The prompt to send to all models"
            }),
        );
        props.insert(
            "system_prompt".into(),
            json!({
                "type": "string",
                "description": "Optional system prompt for all models"
            }),
        );
        props.insert(
            "models".into(),
            json!({
                "type": "array",
                "items": {"type": "string"},
                "description": "List of provider:model identifiers (e.g. ['openai:gpt-4o', 'anthropic:claude-3-5-sonnet']). Defaults to configured models."
            }),
        );
        props.insert(
            "strategy".into(),
            json!({
                "type": "string",
                "description": "Aggregation strategy",
                "enum": ["aggregate", "majority_vote", "best_of_n"],
                "default": "aggregate"
            }),
        );
        props.insert(
            "aggregator_model".into(),
            json!({
                "type": "string",
                "description": "Model to use for aggregation (defaults to first model in list)"
            }),
        );
        props.insert(
            "temperature".into(),
            json!({
                "type": "number",
                "description": "Temperature for worker models (0.0-2.0)",
                "default": 0.7
            }),
        );
        props.insert(
            "max_tokens".into(),
            json!({
                "type": "integer",
                "description": "Max tokens per worker response"
            }),
        );
        props.insert(
            "timeout_secs".into(),
            json!({
                "type": "integer",
                "description": "Timeout in seconds for the entire pipeline",
                "default": 60
            }),
        );
        tool_schema(
            "mixture_of_agents",
            "Run a mixture-of-agents workflow: send the same prompt to multiple LLM providers \
             in parallel, then aggregate their responses into a single high-quality answer. \
             Strategies: 'aggregate' (synthesize all), 'majority_vote' (pick most common), \
             'best_of_n' (pick the best one).",
            JsonSchema::object(props, vec!["prompt".into()]),
        )
    }
}

// ---------------------------------------------------------------------------
// Stub backend (for registration when no real providers are configured)
// ---------------------------------------------------------------------------

/// A stub backend that returns a clear error. Used for tool registration
/// so the schema is always visible; real execution requires the agent layer
/// to inject a proper `MoaBackend`.
pub struct StubMoaBackend;

#[async_trait]
impl MoaBackend for StubMoaBackend {
    async fn query_model(
        &self,
        model: &str,
        _system_prompt: Option<&str>,
        _user_prompt: &str,
        _temperature: Option<f64>,
        _max_tokens: Option<u32>,
    ) -> Result<MoaResponse, ToolError> {
        Err(ToolError::ExecutionFailed(format!(
            "MoA backend not configured. Cannot query model '{model}'. \
             The agent layer must inject a real MoaBackend with provider credentials."
        )))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Mock backend that returns deterministic responses.
    struct MockMoaBackend {
        call_count: AtomicU32,
    }

    impl MockMoaBackend {
        fn new() -> Self {
            Self {
                call_count: AtomicU32::new(0),
            }
        }
    }

    #[async_trait]
    impl MoaBackend for MockMoaBackend {
        async fn query_model(
            &self,
            model: &str,
            _system_prompt: Option<&str>,
            user_prompt: &str,
            _temperature: Option<f64>,
            _max_tokens: Option<u32>,
        ) -> Result<MoaResponse, ToolError> {
            let n = self.call_count.fetch_add(1, Ordering::Relaxed);
            Ok(MoaResponse {
                model: model.to_string(),
                text: format!("Response from {model} to: {user_prompt} (call #{n})"),
                latency_ms: 100 + n as u64 * 50,
                cost_usd: 0.001 * (n as f64 + 1.0),
                tokens_used: Some(100 + n as u64 * 20),
            })
        }
    }

    /// Mock backend where all models return the same text (for majority vote).
    struct UnanimousMoaBackend;

    #[async_trait]
    impl MoaBackend for UnanimousMoaBackend {
        async fn query_model(
            &self,
            model: &str,
            _system_prompt: Option<&str>,
            _user_prompt: &str,
            _temperature: Option<f64>,
            _max_tokens: Option<u32>,
        ) -> Result<MoaResponse, ToolError> {
            Ok(MoaResponse {
                model: model.to_string(),
                text: "The answer is 42.".to_string(),
                latency_ms: 100,
                cost_usd: 0.001,
                tokens_used: Some(50),
            })
        }
    }

    /// Mock backend that fails for specific models.
    struct PartialFailMoaBackend;

    #[async_trait]
    impl MoaBackend for PartialFailMoaBackend {
        async fn query_model(
            &self,
            model: &str,
            _system_prompt: Option<&str>,
            _user_prompt: &str,
            _temperature: Option<f64>,
            _max_tokens: Option<u32>,
        ) -> Result<MoaResponse, ToolError> {
            if model.contains("fail") {
                return Err(ToolError::ExecutionFailed(format!(
                    "{model} intentionally failed"
                )));
            }
            Ok(MoaResponse {
                model: model.to_string(),
                text: "Success response".to_string(),
                latency_ms: 100,
                cost_usd: 0.001,
                tokens_used: Some(50),
            })
        }
    }

    #[tokio::test]
    async fn test_parallel_query_and_aggregate() {
        let backend = Arc::new(MockMoaBackend::new());
        let config = MoaConfig {
            models: vec!["model-a".into(), "model-b".into(), "model-c".into()],
            strategy: MoaStrategy::Aggregate,
            aggregator_model: Some("model-a".into()),
            timeout_secs: 10,
            ..Default::default()
        };

        let result = run_mixture_of_agents(backend.as_ref(), &config, "What is 2+2?", None)
            .await
            .unwrap();

        // 3 worker calls + 1 aggregator call = 4 total
        assert_eq!(backend.call_count.load(Ordering::Relaxed), 4);
        assert_eq!(result.responses.len(), 3);
        assert!(!result.final_answer.is_empty());
        assert!(result.total_cost_usd > 0.0);
        assert!(result.total_tokens > 0);
    }

    #[tokio::test]
    async fn test_majority_vote() {
        let backend = Arc::new(UnanimousMoaBackend);
        let config = MoaConfig {
            models: vec!["a".into(), "b".into(), "c".into()],
            strategy: MoaStrategy::MajorityVote,
            timeout_secs: 10,
            ..Default::default()
        };

        let result = run_mixture_of_agents(backend.as_ref(), &config, "question", None)
            .await
            .unwrap();

        assert_eq!(result.final_answer, "The answer is 42.");
        assert_eq!(result.responses.len(), 3);
    }

    #[tokio::test]
    async fn test_best_of_n() {
        let backend = Arc::new(MockMoaBackend::new());
        let config = MoaConfig {
            models: vec!["model-a".into(), "model-b".into()],
            strategy: MoaStrategy::BestOfN,
            aggregator_model: Some("model-a".into()),
            timeout_secs: 10,
            ..Default::default()
        };

        let result = run_mixture_of_agents(backend.as_ref(), &config, "question", None)
            .await
            .unwrap();

        assert_eq!(result.responses.len(), 2);
        assert!(!result.final_answer.is_empty());
    }

    #[tokio::test]
    async fn test_partial_failure_still_succeeds() {
        let backend = Arc::new(PartialFailMoaBackend);
        let config = MoaConfig {
            models: vec!["good-model".into(), "fail-model".into()],
            strategy: MoaStrategy::MajorityVote,
            timeout_secs: 10,
            ..Default::default()
        };

        let result = run_mixture_of_agents(backend.as_ref(), &config, "question", None)
            .await
            .unwrap();

        // Only the good model's response should be present
        assert_eq!(result.responses.len(), 1);
        assert_eq!(result.responses[0].model, "good-model");
    }

    #[tokio::test]
    async fn test_all_fail_returns_error() {
        let backend = Arc::new(PartialFailMoaBackend);
        let config = MoaConfig {
            models: vec!["fail-a".into(), "fail-b".into()],
            strategy: MoaStrategy::Aggregate,
            timeout_secs: 10,
            ..Default::default()
        };

        let err = run_mixture_of_agents(backend.as_ref(), &config, "question", None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("All MoA models failed"));
    }

    #[tokio::test]
    async fn test_empty_models_returns_error() {
        let backend = Arc::new(MockMoaBackend::new());
        let config = MoaConfig {
            models: vec![],
            ..Default::default()
        };

        let err = run_mixture_of_agents(backend.as_ref(), &config, "question", None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("at least one model"));
    }

    #[tokio::test]
    async fn test_handler_missing_prompt() {
        let handler = MixtureOfAgentsHandler::new(
            Arc::new(StubMoaBackend),
            MoaConfig::default(),
        );
        let err = handler.execute(json!({})).await.unwrap_err();
        assert!(err.to_string().contains("Missing 'prompt'"));
    }

    #[tokio::test]
    async fn test_handler_schema() {
        let handler = MixtureOfAgentsHandler::new(
            Arc::new(StubMoaBackend),
            MoaConfig::default(),
        );
        let schema = handler.schema();
        assert_eq!(schema.name, "mixture_of_agents");
        let desc = &schema.description;
        assert!(desc.contains("mixture-of-agents"));
        assert!(desc.contains("aggregate"));
    }

    #[tokio::test]
    async fn test_handler_builds_config_from_params() {
        let backend = Arc::new(UnanimousMoaBackend);
        let handler = MixtureOfAgentsHandler::new(backend, MoaConfig::default());

        let result = handler
            .execute(json!({
                "prompt": "test",
                "models": ["a", "b"],
                "strategy": "majority_vote",
                "temperature": 0.5,
                "timeout_secs": 5
            }))
            .await
            .unwrap();

        let v: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(v["status"], "completed");
        assert_eq!(v["strategy"], "MajorityVote");
    }

    #[test]
    fn test_majority_vote_picks_most_common() {
        let responses = vec![
            MoaResponse {
                model: "a".into(),
                text: "yes".into(),
                latency_ms: 0,
                cost_usd: 0.0,
                tokens_used: None,
            },
            MoaResponse {
                model: "b".into(),
                text: "no".into(),
                latency_ms: 0,
                cost_usd: 0.0,
                tokens_used: None,
            },
            MoaResponse {
                model: "c".into(),
                text: "yes".into(),
                latency_ms: 0,
                cost_usd: 0.0,
                tokens_used: None,
            },
        ];
        assert_eq!(majority_vote(&responses), "yes");
    }

    #[tokio::test]
    async fn test_stub_backend_errors() {
        let backend = StubMoaBackend;
        let err = backend
            .query_model("test", None, "hello", None, None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not configured"));
    }
}

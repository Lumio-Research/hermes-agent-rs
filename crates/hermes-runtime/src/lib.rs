//! Unified runtime builder for composing Hermes subsystems.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use hermes_agent::agent_builder::{bridge_tool_registry, build_provider};
use hermes_agent::session_persistence::SessionPersistence;
use hermes_agent::LocalAgentService;
use hermes_bus::messages::{
    AgentResponse, AgentStreamChunk, BusMessage, SessionQueryAction, SessionResponse,
};
use hermes_bus::{BusTransport, InProcessTransport, RemoteAgentService};
use hermes_config::GatewayConfig;
use hermes_core::traits::{AgentOverrides, AgentService};
use hermes_core::{AgentError, GatewayError, Message, MessageRole};
use hermes_cron::{CronCompletionEvent, CronRunner, CronScheduler, FileJobPersistence};
use hermes_environments::LocalBackend;
use hermes_gateway::gateway::GatewayConfig as RuntimeGatewayConfig;
use hermes_gateway::hook_payloads;
use hermes_gateway::hooks::HookRegistry;
use hermes_gateway::{DmManager, Gateway, GatewayRuntimeContext, SessionManager};
use hermes_skills::{FileSkillStore, SkillManager};
use hermes_tools::ToolRegistry;
use tokio::sync::broadcast;

/// Builder-pattern runtime composition for Dashboard / platform adapters / cron.
#[derive(Debug, Clone)]
pub struct RuntimeBuilder {
    config: GatewayConfig,
    dashboard_addr: Option<SocketAddr>,
    enable_platforms: bool,
    enable_cron: bool,
}

impl RuntimeBuilder {
    /// Create a new runtime builder from loaded gateway config.
    pub fn new(config: GatewayConfig) -> Self {
        Self {
            config,
            dashboard_addr: None,
            enable_platforms: false,
            enable_cron: false,
        }
    }

    /// Enable dashboard HTTP server on a specific bind address.
    pub fn with_dashboard(mut self, addr: SocketAddr) -> Self {
        self.dashboard_addr = Some(addr);
        self
    }

    /// Enable platform adapter runtime.
    pub fn with_platforms(mut self) -> Self {
        self.enable_platforms = true;
        self
    }

    /// Enable cron scheduler runtime.
    pub fn with_cron(mut self) -> Self {
        self.enable_cron = true;
        self
    }

    /// Start all configured subsystems.
    pub async fn run(self) -> Result<(), AgentError> {
        if self.dashboard_addr.is_none() && !self.enable_platforms && !self.enable_cron {
            return Ok(());
        }

        let config = Arc::new(self.config.clone());
        let hermes_home = config
            .home_dir
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(hermes_config::hermes_home);

        let tool_registry = Arc::new(ToolRegistry::new());
        let terminal_backend: Arc<dyn hermes_core::TerminalBackend> =
            Arc::new(LocalBackend::default());
        let skill_store = Arc::new(FileSkillStore::new(FileSkillStore::default_dir()));
        let skill_provider: Arc<dyn hermes_core::SkillProvider> =
            Arc::new(SkillManager::new(skill_store));
        hermes_tools::register_builtin_tools(&tool_registry, terminal_backend, skill_provider);
        let mut sidecar_tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new();

        let session_persistence = Arc::new(SessionPersistence::new(&hermes_home));
        let _ = session_persistence.ensure_db();
        let local_agent_service: Arc<dyn AgentService> = Arc::new(LocalAgentService::new(
            config.clone(),
            tool_registry.clone(),
            session_persistence,
        ));
        let runtime_gateway_running = Arc::new(AtomicBool::new(false));
        let (bus_client, bus_server) = InProcessTransport::new(512);
        sidecar_tasks.push(tokio::spawn(run_bus_agent_service_loop(
            local_agent_service.clone(),
            bus_server,
        )));
        let agent_service: Arc<dyn AgentService> =
            Arc::new(RemoteAgentService::new(Arc::new(bus_client)));

        let mut dashboard_task: Option<tokio::task::JoinHandle<Result<(), AgentError>>> = None;
        if let Some(addr) = self.dashboard_addr {
            let dashboard_state = hermes_dashboard::HttpServerState::build_with_agent_service(
                (*config).clone(),
                agent_service.clone(),
            )
            .await?
            .with_runtime_gateway_running(runtime_gateway_running.clone());
            dashboard_task = Some(tokio::spawn(async move {
                hermes_dashboard::run_server_with_state(addr, dashboard_state).await
            }));
        }

        let mut gateway: Option<Arc<Gateway>> = None;
        if self.enable_platforms {
            let requirement_issues = gateway_requirement_issues(&config);
            if !requirement_issues.is_empty() {
                let mut msg = String::from("Gateway requirement check failed:\n");
                for issue in requirement_issues {
                    msg.push_str("  - ");
                    msg.push_str(&issue);
                    msg.push('\n');
                }
                msg.push_str("请先执行 `hermes gateway setup` 或 `hermes auth login <provider>` 修复后再启动。");
                return Err(AgentError::Config(msg));
            }
            let runtime_gateway_config = RuntimeGatewayConfig {
                streaming_enabled: config.streaming.enabled,
                ..RuntimeGatewayConfig::default()
            };
            let session_manager = Arc::new(SessionManager::new(config.session.clone()));
            let dm_manager = DmManager::with_pair_behavior();
            let gw = Arc::new(Gateway::new(session_manager, dm_manager, runtime_gateway_config));
            let mut hook_registry = HookRegistry::new();
            hook_registry.register_builtins();
            hook_registry.discover_and_load(&hermes_home.join("hooks"));
            hook_registry.set_execution_limits(Some(16));
            gw.set_hook_registry(Arc::new(hook_registry)).await;

            let agent_service_for_gateway = agent_service.clone();
            gw.set_message_handler_with_context(Arc::new(move |messages, ctx: GatewayRuntimeContext| {
                let svc = agent_service_for_gateway.clone();
                Box::pin(async move {
                    runtime_send_with_agent_service(svc, &messages, &ctx).await
                })
            }))
            .await;

            let summary = hermes_gateway::platform_registry::register_platforms(
                &gw,
                &config,
                &mut sidecar_tasks,
            )
            .await?;
            let enabled_refs: Vec<&str> = summary.registered.iter().map(String::as_str).collect();
            gw.emit_hook_event(
                "gateway:startup",
                hook_payloads::gateway_startup(&enabled_refs),
            )
            .await;
            gw.start_all()
                .await
                .map_err(|e| AgentError::Gateway(e.to_string()))?;
            runtime_gateway_running.store(true, Ordering::Relaxed);
            {
                let gw_reconnect = gw.clone();
                sidecar_tasks.push(tokio::spawn(async move {
                    gw_reconnect.platform_reconnect_watcher(20).await;
                }));
                let gw_expiry = gw.clone();
                sidecar_tasks.push(tokio::spawn(async move {
                    gw_expiry.session_expiry_watcher(300).await;
                }));
                let gw_cleanup = gw.clone();
                sidecar_tasks.push(tokio::spawn(async move {
                    gw_cleanup.gateway_cleanup_watcher(300).await;
                }));
            }
            gateway = Some(gw);
        }

        let mut cron_scheduler: Option<Arc<CronScheduler>> = None;
        if self.enable_cron {
            let cron_dir = hermes_home.join("cron");
            std::fs::create_dir_all(&cron_dir)
                .map_err(|e| AgentError::Io(format!("cron dir {}: {}", cron_dir.display(), e)))?;
            let default_model = config.model.clone().unwrap_or_else(|| "gpt-4o".to_string());
            let cron_persistence = Arc::new(FileJobPersistence::with_dir(cron_dir));
            let cron_llm = build_provider(&config, &default_model);
            let agent_registry = Arc::new(bridge_tool_registry(&tool_registry));
            let cron_runner = Arc::new(CronRunner::new(cron_llm, agent_registry));
            let mut scheduler = CronScheduler::new(cron_persistence, cron_runner);
            let (cron_tx, cron_rx) = broadcast::channel::<CronCompletionEvent>(64);
            scheduler.set_completion_broadcast(cron_tx);
            scheduler
                .load_persisted_jobs()
                .await
                .map_err(|e| AgentError::Config(format!("cron load: {e}")))?;
            scheduler.start().await;
            cron_scheduler = Some(Arc::new(scheduler));

            let webhooks_path = hermes_home.join("webhooks.json");
            sidecar_tasks.push(tokio::spawn(async move {
                run_cron_webhook_delivery_loop(cron_rx, webhooks_path).await;
            }));
        }

        if let Some(task) = dashboard_task.as_mut() {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                joined = task => {
                    match joined {
                        Ok(Ok(())) => return Ok(()),
                        Ok(Err(e)) => return Err(e),
                        Err(e) => return Err(AgentError::Io(format!("dashboard task join: {}", e))),
                    }
                }
            }
        } else {
            tokio::signal::ctrl_c()
                .await
                .map_err(|e| AgentError::Io(format!("ctrl-c wait failed: {}", e)))?;
        }

        if let Some(scheduler) = cron_scheduler {
            scheduler.stop().await;
        }
        if let Some(gw) = gateway {
            gw.stop_all()
                .await
                .map_err(|e| AgentError::Gateway(e.to_string()))?;
            runtime_gateway_running.store(false, Ordering::Relaxed);
        }
        for task in sidecar_tasks {
            task.abort();
        }
        if let Some(task) = dashboard_task {
            task.abort();
        }

        Ok(())
    }
}

fn gateway_requirement_issues(config: &GatewayConfig) -> Vec<String> {
    let mut issues = Vec::new();
    let check = |enabled: bool, cond: bool| enabled && !cond;

    if let Some(p) = config.platforms.get("telegram") {
        if check(p.enabled, platform_token_or_extra(p).is_some()) {
            issues.push("telegram.enabled=true 但缺少 token".to_string());
        }
    }
    if let Some(p) = config.platforms.get("weixin") {
        let account_id = extra_string(p, "account_id").is_some();
        let token = platform_token_or_extra(p).is_some();
        if check(p.enabled, account_id && token) {
            issues.push("weixin.enabled=true 但缺少 account_id 或 token".to_string());
        }
    }
    if let Some(p) = config.platforms.get("discord") {
        if check(p.enabled, platform_token_or_extra(p).is_some()) {
            issues.push("discord.enabled=true 但缺少 token".to_string());
        }
    }
    if let Some(p) = config.platforms.get("slack") {
        if check(p.enabled, platform_token_or_extra(p).is_some()) {
            issues.push("slack.enabled=true 但缺少 token".to_string());
        }
    }
    if let Some(p) = config
        .platforms
        .get("qqbot")
        .or_else(|| config.platforms.get("qq"))
    {
        let app_id = extra_string(p, "app_id").is_some();
        let secret = extra_string(p, "client_secret").is_some();
        if check(p.enabled, app_id && secret) {
            issues.push("qqbot.enabled=true 但缺少 app_id 或 client_secret".to_string());
        }
    }
    if let Some(p) = config.platforms.get("wecom_callback") {
        let ready = extra_string(p, "corp_id").is_some()
            && extra_string(p, "corp_secret").is_some()
            && extra_string(p, "agent_id").is_some()
            && platform_token_or_extra(p)
                .or_else(|| extra_string(p, "token"))
                .is_some()
            && extra_string(p, "encoding_aes_key").is_some();
        if check(p.enabled, ready) {
            issues.push(
                "wecom_callback.enabled=true 但缺少 corp_id/corp_secret/agent_id/token/encoding_aes_key"
                    .to_string(),
            );
        }
    }
    issues
}

fn platform_token_or_extra(platform_cfg: &hermes_config::PlatformConfig) -> Option<String> {
    platform_cfg
        .token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| {
            platform_cfg
                .extra
                .get("token")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
        })
}

fn extra_string(platform_cfg: &hermes_config::PlatformConfig, key: &str) -> Option<String> {
    platform_cfg
        .extra
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

async fn run_bus_agent_service_loop(
    service: Arc<dyn AgentService>,
    transport: InProcessTransport,
) {
    loop {
        let incoming = match transport.receive().await {
            Ok(msg) => msg,
            Err(_) => break,
        };
        match incoming {
            BusMessage::AgentRequest(req) => {
                if req.stream {
                    let tx = transport.clone();
                    let rid = req.request_id.clone();
                    let sid = req.session_id.clone();
                    let on_chunk = Arc::new(move |chunk: hermes_core::StreamChunk| {
                        let tx = tx.clone();
                        let rid = rid.clone();
                        let sid = sid.clone();
                        tokio::spawn(async move {
                            let _ = tx
                                .send(BusMessage::AgentStreamChunk(AgentStreamChunk {
                                    request_id: rid,
                                    session_id: sid,
                                    chunk,
                                }))
                                .await;
                        });
                    });
                    let result = service
                        .send_message_stream(
                            &req.session_id,
                            &req.text,
                            req.overrides.clone(),
                            on_chunk,
                        )
                        .await;
                    let response = match result {
                        Ok(reply) => AgentResponse {
                            request_id: req.request_id,
                            session_id: req.session_id,
                            text: reply.text,
                            message_count: reply.message_count,
                            error: None,
                            done: true,
                        },
                        Err(err) => AgentResponse {
                            request_id: req.request_id,
                            session_id: req.session_id,
                            text: String::new(),
                            message_count: 0,
                            error: Some(err.to_string()),
                            done: true,
                        },
                    };
                    let _ = transport.send(BusMessage::AgentResponse(response)).await;
                } else {
                    let result = service
                        .send_message(&req.session_id, &req.text, req.overrides.clone())
                        .await;
                    let response = match result {
                        Ok(reply) => AgentResponse {
                            request_id: req.request_id,
                            session_id: req.session_id,
                            text: reply.text,
                            message_count: reply.message_count,
                            error: None,
                            done: true,
                        },
                        Err(err) => AgentResponse {
                            request_id: req.request_id,
                            session_id: req.session_id,
                            text: String::new(),
                            message_count: 0,
                            error: Some(err.to_string()),
                            done: true,
                        },
                    };
                    let _ = transport.send(BusMessage::AgentResponse(response)).await;
                }
            }
            BusMessage::SessionQuery(query) => {
                let response = match query.action {
                    SessionQueryAction::GetMessages => {
                        match service.get_session_messages(&query.session_id).await {
                            Ok(messages) => SessionResponse {
                                request_id: query.request_id,
                                sessions: Vec::new(),
                                total: 0,
                                messages,
                                error: None,
                            },
                            Err(err) => SessionResponse {
                                request_id: query.request_id,
                                sessions: Vec::new(),
                                total: 0,
                                messages: Vec::new(),
                                error: Some(err.to_string()),
                            },
                        }
                    }
                    SessionQueryAction::ResetSession => match service.reset_session(&query.session_id).await {
                        Ok(()) => SessionResponse {
                            request_id: query.request_id,
                            sessions: Vec::new(),
                            total: 0,
                            messages: Vec::new(),
                            error: None,
                        },
                        Err(err) => SessionResponse {
                            request_id: query.request_id,
                            sessions: Vec::new(),
                            total: 0,
                            messages: Vec::new(),
                            error: Some(err.to_string()),
                        },
                    },
                };
                let _ = transport.send(BusMessage::SessionResponse(response)).await;
            }
            _ => {}
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct RuntimeWebhookStore {
    #[serde(default)]
    webhooks: Vec<RuntimeWebhookEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct RuntimeWebhookEntry {
    url: String,
}

fn load_webhook_urls(path: &Path) -> Result<Vec<String>, AgentError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read(path)
        .map_err(|e| AgentError::Io(format!("read webhooks {}: {}", path.display(), e)))?;
    let store: RuntimeWebhookStore = serde_json::from_slice(&raw)
        .map_err(|e| AgentError::Config(format!("parse webhooks {}: {}", path.display(), e)))?;
    Ok(store
        .webhooks
        .into_iter()
        .map(|w| w.url.trim().to_string())
        .filter(|u| !u.is_empty())
        .collect())
}

async fn run_cron_webhook_delivery_loop(
    mut rx: broadcast::Receiver<CronCompletionEvent>,
    webhooks_json: std::path::PathBuf,
) {
    use tokio::sync::broadcast::error::RecvError;

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("runtime cron webhooks: HTTP client build failed: {e}");
            return;
        }
    };

    loop {
        let ev = match rx.recv().await {
            Ok(ev) => ev,
            Err(RecvError::Lagged(n)) => {
                tracing::debug!(n, "runtime cron webhook receiver lagged; skipped messages");
                continue;
            }
            Err(RecvError::Closed) => break,
        };

        let urls = match load_webhook_urls(&webhooks_json) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("runtime cron webhooks: failed to load store: {e}");
                continue;
            }
        };

        for url in urls {
            if let Err(e) = client.post(&url).json(&ev).send().await {
                tracing::warn!(url = %url, "runtime cron webhook delivery failed: {e}");
            }
        }
    }
}

async fn runtime_send_with_agent_service(
    service: Arc<dyn AgentService>,
    messages: &[Message],
    ctx: &GatewayRuntimeContext,
) -> Result<String, GatewayError> {
    let text = messages
        .iter()
        .rev()
        .find(|m| m.role == MessageRole::User)
        .and_then(|m| m.content.clone())
        .unwrap_or_default();
    let overrides = AgentOverrides {
        model: ctx.model.clone(),
        personality: ctx.personality.clone(),
    };
    let reply = service
        .send_message(&ctx.session_key, &text, overrides)
        .await
        .map_err(|e| GatewayError::Platform(e.to_string()))?;
    Ok(reply.text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_methods_toggle_flags() {
        let cfg = GatewayConfig::default();
        let addr: SocketAddr = "127.0.0.1:3001".parse().expect("addr");
        let rb = RuntimeBuilder::new(cfg)
            .with_dashboard(addr)
            .with_platforms()
            .with_cron();
        assert_eq!(rb.dashboard_addr, Some(addr));
        assert!(rb.enable_platforms);
        assert!(rb.enable_cron);
    }

    #[tokio::test]
    async fn run_without_enabled_subsystems_returns_ok() {
        let cfg = GatewayConfig::default();
        let rb = RuntimeBuilder::new(cfg);
        let res = rb.run().await;
        assert!(res.is_ok());
    }
}


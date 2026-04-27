//! Unified runtime builder for composing Hermes subsystems.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use hermes_agent::agent_builder::{bridge_tool_registry, build_provider};
use hermes_agent::session_persistence::SessionPersistence;
use hermes_agent::{install_plugin_tools_into_registry, LocalAgentService, PluginManager};
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

/// Empty in-process [`PluginManager`] wrapped for CLI / default runtime wiring.
///
/// `hermes gateway start`, `hermes web`, and `hermes serve` pass this into
/// [`RuntimeBuilder::with_plugin_manager`]. Custom binaries may `Arc::clone` a
/// shared instance, lock, and [`PluginManager::register`] before [`RuntimeBuilder::run`]
/// so [`install_plugin_tools_into_registry`] merges tools on startup.
pub fn empty_plugin_manager() -> Arc<Mutex<PluginManager>> {
    Arc::new(Mutex::new(PluginManager::new()))
}

/// Builder-pattern runtime composition for Dashboard / platform adapters / cron.
///
/// For gateway slash extensions, register [`hermes_gateway::register_slash_command_extension`]
/// before [`RuntimeBuilder::run`] in the same process so the gateway sees them on startup.
#[derive(Clone)]
pub struct RuntimeBuilder {
    config: GatewayConfig,
    dashboard_addr: Option<SocketAddr>,
    enable_platforms: bool,
    enable_cron: bool,
    /// Optional in-process plugins: tools merged into the shared [`ToolRegistry`] after
    /// built-ins; [`PluginManager`] is also passed to [`LocalAgentService`] for hook callbacks.
    plugin_manager: Option<Arc<Mutex<PluginManager>>>,
}

impl std::fmt::Debug for RuntimeBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeBuilder")
            .field("config", &self.config)
            .field("dashboard_addr", &self.dashboard_addr)
            .field("enable_platforms", &self.enable_platforms)
            .field("enable_cron", &self.enable_cron)
            .field(
                "plugin_manager",
                &self.plugin_manager.as_ref().map(|_| "<PluginManager>"),
            )
            .finish()
    }
}

impl RuntimeBuilder {
    /// Create a new runtime builder from loaded gateway config.
    pub fn new(config: GatewayConfig) -> Self {
        Self {
            config,
            dashboard_addr: None,
            enable_platforms: false,
            enable_cron: false,
            plugin_manager: None,
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

    /// Attach an in-process [`PluginManager`] (tools + hook callbacks for the local agent).
    pub fn with_plugin_manager(mut self, plugin_manager: Arc<Mutex<PluginManager>>) -> Self {
        self.plugin_manager = Some(plugin_manager);
        self
    }

    /// Start all configured subsystems.
    pub async fn run(self) -> Result<(), AgentError> {
        if self.dashboard_addr.is_none() && !self.enable_platforms && !self.enable_cron {
            return Ok(());
        }

        // Preflight gateway platform credentials **before** spawning bus/dashboard/cron work
        // (see `.kiro/specs/.../gateway-requirements-single-source-rfc.md` Phase A).
        if self.enable_platforms {
            let requirement_issues = hermes_gateway::gateway_requirement_issues(&self.config);
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
        if let Some(pm) = self.plugin_manager.as_ref() {
            let guard = pm.lock().map_err(|_| {
                AgentError::Io("plugin manager mutex poisoned before tool install".to_string())
            })?;
            install_plugin_tools_into_registry(&tool_registry, &*guard);
        }
        let mut sidecar_tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new();

        let session_persistence = Arc::new(SessionPersistence::new(&hermes_home));
        let _ = session_persistence.ensure_db();
        let local_agent_service: Arc<dyn AgentService> = Arc::new(
            if let Some(pm) = self.plugin_manager.clone() {
                LocalAgentService::new_with_plugin_manager(
                    config.clone(),
                    tool_registry.clone(),
                    session_persistence,
                    pm,
                )
            } else {
                LocalAgentService::new(
                    config.clone(),
                    tool_registry.clone(),
                    session_persistence,
                )
            },
        );
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
            let dashboard_state = hermes_server::HttpServerState::build_with_agent_service(
                (*config).clone(),
                agent_service.clone(),
            )
            .await?
            .with_runtime_gateway_running(runtime_gateway_running.clone());
            dashboard_task = Some(tokio::spawn(async move {
                hermes_server::run_server_with_state(addr, dashboard_state).await
            }));
        }

        let mut gateway: Option<Arc<Gateway>> = None;
        if self.enable_platforms {
            let runtime_gateway_config = RuntimeGatewayConfig {
                streaming_enabled: config.streaming.enabled,
                ..RuntimeGatewayConfig::default()
            };
            let session_manager = Arc::new(SessionManager::new(config.session.clone()));

            // Collect allowed_users / admin_users from all platform configs
            // and feed them into the DmManager so the gateway's auth gate
            // actually recognises users listed in config.yaml.
            let mut authorized_users = std::collections::HashSet::new();
            let mut admin_users = std::collections::HashSet::new();
            let mut any_pair = false;
            for (_name, plat_cfg) in &config.platforms {
                for u in &plat_cfg.allowed_users {
                    authorized_users.insert(u.clone());
                }
                for u in &plat_cfg.admin_users {
                    admin_users.insert(u.clone());
                }
                if plat_cfg.unauthorized_dm_behavior == hermes_config::UnauthorizedDmBehavior::Pair {
                    any_pair = true;
                }
            }
            let unauthorized_behavior = if any_pair {
                hermes_config::UnauthorizedDmBehavior::Pair
            } else {
                hermes_config::UnauthorizedDmBehavior::Ignore
            };
            let dm_manager = DmManager::new(
                authorized_users,
                admin_users,
                unauthorized_behavior,
            );
            let gw = Arc::new(Gateway::new(
                session_manager,
                dm_manager,
                runtime_gateway_config,
            ));
            let mut hook_registry = HookRegistry::new();
            hook_registry.register_builtins();
            hook_registry.discover_and_load(&hermes_home.join("hooks"));
            hook_registry.set_execution_limits(Some(16));
            gw.set_hook_registry(Arc::new(hook_registry)).await;

            let agent_service_for_gateway = agent_service.clone();
            gw.set_message_handler_with_context(Arc::new(
                move |messages, ctx: GatewayRuntimeContext| {
                    let svc = agent_service_for_gateway.clone();
                    Box::pin(
                        async move { runtime_send_with_agent_service(svc, &messages, &ctx).await },
                    )
                },
            ))
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

async fn run_bus_agent_service_loop(service: Arc<dyn AgentService>, transport: InProcessTransport) {
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
                    SessionQueryAction::ResetSession => {
                        match service.reset_session(&query.session_id).await {
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
                        }
                    }
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

    #[test]
    fn builder_with_plugin_manager_stores_registry() {
        let cfg = GatewayConfig::default();
        let pm = Arc::new(Mutex::new(PluginManager::new()));
        let rb = RuntimeBuilder::new(cfg).with_plugin_manager(pm);
        assert!(rb.plugin_manager.is_some());
    }

    #[tokio::test]
    async fn run_without_enabled_subsystems_returns_ok() {
        let cfg = GatewayConfig::default();
        let rb = RuntimeBuilder::new(cfg);
        let res = rb.run().await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn run_with_empty_plugin_manager_and_no_subsystems_returns_ok() {
        let cfg = GatewayConfig::default();
        let rb = RuntimeBuilder::new(cfg).with_plugin_manager(empty_plugin_manager());
        let res = rb.run().await;
        assert!(res.is_ok());
    }
}

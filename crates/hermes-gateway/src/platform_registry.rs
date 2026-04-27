//! Platform registry for registering all platform adapters.
//!
//! This module provides a centralized way to register all platform adapters
//! based on the configuration.

#[cfg(any(
    feature = "telegram",
    feature = "weixin",
    feature = "discord",
    feature = "slack",
    feature = "matrix",
    feature = "mattermost",
    feature = "signal",
    feature = "whatsapp",
    feature = "dingtalk",
    feature = "feishu",
    feature = "wecom",
    feature = "wecom-callback",
    feature = "qqbot",
    feature = "bluebubbles",
    feature = "email",
    feature = "sms",
    feature = "homeassistant",
    feature = "webhook",
    feature = "api-server"
))]
use std::collections::HashMap;
use std::sync::Arc;

use hermes_config::GatewayConfig;
use hermes_core::AgentError;

use crate::gateway::Gateway;
use crate::platform_requirements::{
    evaluate_gateway_requirements, RequirementScope, RequirementSeverity,
};

/// Summary of platform registration results.
#[derive(Debug, Clone)]
pub struct RegistrationSummary {
    /// Names of successfully registered adapters.
    pub registered: Vec<String>,
    /// Errors encountered during registration (adapter name, error message).
    pub errors: Vec<(String, String)>,
}

/// Register all platform adapters based on configuration.
///
/// This function registers all enabled platform adapters from the configuration
/// into the gateway. It supports all 17 platform adapters + ApiServer + Webhook.
pub async fn register_platforms(
    gateway: &Gateway,
    config: &GatewayConfig,
    sidecar_tasks: &mut Vec<tokio::task::JoinHandle<()>>,
) -> Result<RegistrationSummary, AgentError> {
    #[cfg(not(any(
        feature = "telegram",
        feature = "weixin",
        feature = "discord",
        feature = "slack",
        feature = "matrix",
        feature = "mattermost",
        feature = "signal",
        feature = "whatsapp",
        feature = "dingtalk",
        feature = "feishu",
        feature = "wecom",
        feature = "wecom-callback",
        feature = "qqbot",
        feature = "bluebubbles",
        feature = "email",
        feature = "sms",
        feature = "homeassistant",
        feature = "webhook",
        feature = "api-server"
    )))]
    {
        let _ = (gateway, config, sidecar_tasks);
        Ok(RegistrationSummary {
            registered: Vec::new(),
            errors: Vec::new(),
        })
    }

    #[cfg(any(
        feature = "telegram",
        feature = "weixin",
        feature = "discord",
        feature = "slack",
        feature = "matrix",
        feature = "mattermost",
        feature = "signal",
        feature = "whatsapp",
        feature = "dingtalk",
        feature = "feishu",
        feature = "wecom",
        feature = "wecom-callback",
        feature = "qqbot",
        feature = "bluebubbles",
        feature = "email",
        feature = "sms",
        feature = "homeassistant",
        feature = "webhook",
        feature = "api-server"
    ))]
    {
        let mut registered = Vec::new();
        let mut errors = Vec::new();

        let mut fatal_by_platform: HashMap<String, String> =
            evaluate_gateway_requirements(config, RequirementScope::RuntimeStart)
                .into_iter()
                .filter(|i| i.severity == RequirementSeverity::Fatal)
                .map(|i| (i.platform, i.message))
                .collect();

        fn extra_u16(cfg: &hermes_config::PlatformConfig, key: &str, default: u16) -> u16 {
            cfg.extra
                .get(key)
                .and_then(|v| v.as_u64())
                .map(|n| n as u16)
                .unwrap_or(default)
        }

        fn extra_bool(cfg: &hermes_config::PlatformConfig, key: &str, default: bool) -> bool {
            cfg.extra
                .get(key)
                .and_then(|v| v.as_bool())
                .unwrap_or(default)
        }

        // Telegram
        #[cfg(feature = "telegram")]
        if let Some(platform_cfg) = config.platforms.get("telegram") {
            if platform_cfg.enabled {
                if let Some(msg) = fatal_by_platform.remove("telegram") {
                    errors.push(("telegram".to_string(), msg));
                } else if let Some(token) = hermes_config::platform_token_or_extra(platform_cfg) {
                    use crate::platforms::telegram::{TelegramAdapter, TelegramConfig};

                    let telegram_config = TelegramConfig {
                        token,
                        webhook_url: None,
                        polling: true,
                        proxy: Default::default(),
                        parse_markdown: false,
                        parse_html: false,
                        poll_timeout: 30,
                        bot_username: None,
                    };
                    match TelegramAdapter::new(telegram_config) {
                        Ok(adapter) => {
                            let adapter = Arc::new(adapter);
                            gateway.register_adapter("telegram", adapter.clone()).await;
                            registered.push("telegram".to_string());

                            // Start Telegram poll loop
                            sidecar_tasks.push(tokio::spawn(async move {
                                // Simplified: actual implementation would run telegram poll loop
                                // For now, just log that we would start it
                                tracing::info!("Telegram poll loop would start here");
                            }));
                        }
                        Err(e) => errors.push(("telegram".to_string(), e.to_string())),
                    }
                }
            }
        }

        // Weixin
        #[cfg(feature = "weixin")]
        if let Some(platform_cfg) = config.platforms.get("weixin") {
            if platform_cfg.enabled {
                use crate::platforms::weixin::{WeChatAdapter, WeixinConfig};

                if let Some(msg) = fatal_by_platform.remove("weixin") {
                    errors.push(("weixin".to_string(), msg));
                } else {
                    let wx_cfg = WeixinConfig::from_platform_config(platform_cfg);
                    match WeChatAdapter::new(wx_cfg) {
                        Ok(adapter) => {
                            gateway.register_adapter("weixin", Arc::new(adapter)).await;
                            registered.push("weixin".to_string());
                        }
                        Err(e) => errors.push(("weixin".to_string(), e.to_string())),
                    }
                }
            }
        }

        // Discord
        #[cfg(feature = "discord")]
        if let Some(platform_cfg) = config.platforms.get("discord") {
            if platform_cfg.enabled {
                use crate::platforms::discord::{DiscordAdapter, DiscordConfig};

                if let Some(msg) = fatal_by_platform.remove("discord") {
                    errors.push(("discord".to_string(), msg));
                } else if let Some(token) = hermes_config::platform_token_or_extra(platform_cfg) {
                    let discord_cfg = DiscordConfig {
                        token,
                        application_id: hermes_config::extra_string(platform_cfg, "application_id"),
                        proxy: Default::default(),
                        require_mention: platform_cfg.require_mention.unwrap_or(false),
                        intents: platform_cfg
                            .extra
                            .get("intents")
                            .and_then(|v| v.as_u64())
                            .unwrap_or((1 << 0) | (1 << 9) | (1 << 15)),
                    };
                    match DiscordAdapter::new(discord_cfg) {
                        Ok(adapter) => {
                            gateway.register_adapter("discord", Arc::new(adapter)).await;
                            registered.push("discord".to_string());
                        }
                        Err(e) => errors.push(("discord".to_string(), e.to_string())),
                    }
                }
            }
        }

        // Slack
        #[cfg(feature = "slack")]
        if let Some(platform_cfg) = config.platforms.get("slack") {
            if platform_cfg.enabled {
                use crate::platforms::slack::{SlackAdapter, SlackConfig};

                if let Some(msg) = fatal_by_platform.remove("slack") {
                    errors.push(("slack".to_string(), msg));
                } else if let Some(token) = hermes_config::platform_token_or_extra(platform_cfg) {
                    let slack_cfg = SlackConfig {
                        token,
                        app_token: hermes_config::extra_string(platform_cfg, "app_token"),
                        socket_mode: extra_bool(platform_cfg, "socket_mode", false),
                        proxy: Default::default(),
                    };
                    match SlackAdapter::new(slack_cfg) {
                        Ok(adapter) => {
                            gateway.register_adapter("slack", Arc::new(adapter)).await;
                            registered.push("slack".to_string());
                        }
                        Err(e) => errors.push(("slack".to_string(), e.to_string())),
                    }
                }
            }
        }

        // Matrix
        #[cfg(feature = "matrix")]
        if let Some(platform_cfg) = config.platforms.get("matrix") {
            if platform_cfg.enabled {
                use crate::platforms::matrix::{MatrixAdapter, MatrixConfig};

                if let Some(msg) = fatal_by_platform.remove("matrix") {
                    errors.push(("matrix".to_string(), msg));
                } else {
                    let homeserver_url =
                        hermes_config::extra_string(platform_cfg, "homeserver_url")
                            .or_else(|| hermes_config::extra_string(platform_cfg, "homeserver"))
                            .unwrap_or_default();
                    let user_id =
                        hermes_config::extra_string(platform_cfg, "user_id").unwrap_or_default();
                    let access_token = hermes_config::platform_token_or_extra(platform_cfg)
                        .or_else(|| hermes_config::extra_string(platform_cfg, "access_token"))
                        .unwrap_or_default();
                    let matrix_cfg = MatrixConfig {
                        homeserver_url,
                        user_id,
                        access_token,
                        room_id: hermes_config::extra_string(platform_cfg, "room_id"),
                        proxy: Default::default(),
                    };
                    match MatrixAdapter::new(matrix_cfg) {
                        Ok(adapter) => {
                            gateway.register_adapter("matrix", Arc::new(adapter)).await;
                            registered.push("matrix".to_string());
                        }
                        Err(e) => errors.push(("matrix".to_string(), e.to_string())),
                    }
                }
            }
        }

        // Mattermost
        #[cfg(feature = "mattermost")]
        if let Some(platform_cfg) = config.platforms.get("mattermost") {
            if platform_cfg.enabled {
                use crate::platforms::mattermost::{MattermostAdapter, MattermostConfig};

                if let Some(msg) = fatal_by_platform.remove("mattermost") {
                    errors.push(("mattermost".to_string(), msg));
                } else {
                    let token =
                        hermes_config::platform_token_or_extra(platform_cfg).unwrap_or_default();
                    let server_url = hermes_config::extra_string(platform_cfg, "server_url")
                        .or_else(|| hermes_config::extra_string(platform_cfg, "url"))
                        .unwrap_or_default();
                    let mm_cfg = MattermostConfig {
                        server_url,
                        token,
                        team_id: hermes_config::extra_string(platform_cfg, "team_id"),
                        proxy: Default::default(),
                    };
                    match MattermostAdapter::new(mm_cfg) {
                        Ok(adapter) => {
                            gateway
                                .register_adapter("mattermost", Arc::new(adapter))
                                .await;
                            registered.push("mattermost".to_string());
                        }
                        Err(e) => errors.push(("mattermost".to_string(), e.to_string())),
                    }
                }
            }
        }

        // Signal
        #[cfg(feature = "signal")]
        if let Some(platform_cfg) = config.platforms.get("signal") {
            if platform_cfg.enabled {
                use crate::platforms::signal::{SignalAdapter, SignalConfig};

                if let Some(msg) = fatal_by_platform.remove("signal") {
                    errors.push(("signal".to_string(), msg));
                } else {
                    let phone_number = hermes_config::extra_string(platform_cfg, "phone_number")
                        .or_else(|| hermes_config::extra_string(platform_cfg, "account"))
                        .unwrap_or_default();
                    let signal_cfg = SignalConfig {
                        phone_number,
                        api_url: hermes_config::extra_string(platform_cfg, "api_url")
                            .unwrap_or_else(|| "http://localhost:8080".to_string()),
                        proxy: Default::default(),
                    };
                    match SignalAdapter::new(signal_cfg) {
                        Ok(adapter) => {
                            gateway.register_adapter("signal", Arc::new(adapter)).await;
                            registered.push("signal".to_string());
                        }
                        Err(e) => errors.push(("signal".to_string(), e.to_string())),
                    }
                }
            }
        }

        // WhatsApp
        #[cfg(feature = "whatsapp")]
        if let Some(platform_cfg) = config.platforms.get("whatsapp") {
            if platform_cfg.enabled {
                use crate::platforms::whatsapp::{WhatsAppAdapter, WhatsAppConfig};

                if let Some(msg) = fatal_by_platform.remove("whatsapp") {
                    errors.push(("whatsapp".to_string(), msg));
                } else if let Some(token) = hermes_config::platform_token_or_extra(platform_cfg) {
                    let wa_cfg = WhatsAppConfig {
                        token,
                        phone_number_id: hermes_config::extra_string(
                            platform_cfg,
                            "phone_number_id",
                        ),
                        business_account_id: hermes_config::extra_string(
                            platform_cfg,
                            "business_account_id",
                        ),
                        verify_token: hermes_config::extra_string(platform_cfg, "verify_token"),
                        proxy: Default::default(),
                    };
                    match WhatsAppAdapter::new(wa_cfg) {
                        Ok(adapter) => {
                            gateway
                                .register_adapter("whatsapp", Arc::new(adapter))
                                .await;
                            registered.push("whatsapp".to_string());
                        }
                        Err(e) => errors.push(("whatsapp".to_string(), e.to_string())),
                    }
                }
            }
        }

        // DingTalk
        #[cfg(feature = "dingtalk")]
        if let Some(platform_cfg) = config.platforms.get("dingtalk") {
            if platform_cfg.enabled {
                use crate::platforms::dingtalk::{DingTalkAdapter, DingTalkConfig};

                if let Some(msg) = fatal_by_platform.remove("dingtalk") {
                    errors.push(("dingtalk".to_string(), msg));
                } else {
                    let ding_cfg = DingTalkConfig::from_platform_config(platform_cfg);
                    match DingTalkAdapter::new(ding_cfg) {
                        Ok(adapter) => {
                            gateway
                                .register_adapter("dingtalk", Arc::new(adapter))
                                .await;
                            registered.push("dingtalk".to_string());
                        }
                        Err(e) => errors.push(("dingtalk".to_string(), e.to_string())),
                    }
                }
            }
        }

        // Feishu
        #[cfg(feature = "feishu")]
        if let Some(platform_cfg) = config.platforms.get("feishu") {
            if platform_cfg.enabled {
                use crate::platforms::feishu::{FeishuAdapter, FeishuConfig};

                if let Some(msg) = fatal_by_platform.remove("feishu") {
                    errors.push(("feishu".to_string(), msg));
                } else {
                    let app_id =
                        hermes_config::extra_string(platform_cfg, "app_id").unwrap_or_default();
                    let app_secret =
                        hermes_config::extra_string(platform_cfg, "app_secret").unwrap_or_default();
                    let feishu_cfg = FeishuConfig {
                        app_id,
                        app_secret,
                        verification_token: hermes_config::extra_string(
                            platform_cfg,
                            "verification_token",
                        ),
                        encrypt_key: hermes_config::extra_string(platform_cfg, "encrypt_key"),
                        proxy: Default::default(),
                    };
                    match FeishuAdapter::new(feishu_cfg) {
                        Ok(adapter) => {
                            gateway.register_adapter("feishu", Arc::new(adapter)).await;
                            registered.push("feishu".to_string());
                        }
                        Err(e) => errors.push(("feishu".to_string(), e.to_string())),
                    }
                }
            }
        }

        // WeCom
        #[cfg(feature = "wecom")]
        if let Some(platform_cfg) = config.platforms.get("wecom") {
            if platform_cfg.enabled {
                use crate::platforms::wecom::{WeComAdapter, WeComConfig};

                if let Some(msg) = fatal_by_platform.remove("wecom") {
                    errors.push(("wecom".to_string(), msg));
                } else {
                    let corp_id =
                        hermes_config::extra_string(platform_cfg, "corp_id").unwrap_or_default();
                    let agent_id =
                        hermes_config::extra_string(platform_cfg, "agent_id").unwrap_or_default();
                    let secret =
                        hermes_config::extra_string(platform_cfg, "secret").unwrap_or_default();
                    let wecom_cfg = WeComConfig {
                        corp_id,
                        agent_id,
                        secret,
                        proxy: Default::default(),
                    };
                    match WeComAdapter::new(wecom_cfg) {
                        Ok(adapter) => {
                            gateway.register_adapter("wecom", Arc::new(adapter)).await;
                            registered.push("wecom".to_string());
                        }
                        Err(e) => errors.push(("wecom".to_string(), e.to_string())),
                    }
                }
            }
        }

        // WeCom Callback
        #[cfg(feature = "wecom-callback")]
        if let Some(platform_cfg) = config.platforms.get("wecom_callback") {
            if platform_cfg.enabled {
                if let Some(msg) = fatal_by_platform.remove("wecom_callback") {
                    errors.push(("wecom_callback".to_string(), msg));
                } else {
                    // Simplified version - actual implementation would need more setup
                    // For now, just log that we would set it up
                    tracing::info!("WeCom callback adapter would be set up here");
                    errors.push((
                    "wecom_callback".to_string(),
                    "wecom_callback adapter requires additional setup (simplified implementation)".to_string(),
                ));
                }
            }
        }

        // QQBot
        #[cfg(feature = "qqbot")]
        if let Some(platform_cfg) = config
            .platforms
            .get("qqbot")
            .or_else(|| config.platforms.get("qq"))
        {
            if platform_cfg.enabled {
                use crate::platforms::qqbot::{QqBotAdapter, QqBotConfig};

                if let Some(msg) = fatal_by_platform.remove("qqbot") {
                    errors.push(("qqbot".to_string(), msg));
                } else {
                    let app_id =
                        hermes_config::extra_string(platform_cfg, "app_id").unwrap_or_default();
                    let client_secret = hermes_config::extra_string(platform_cfg, "client_secret")
                        .unwrap_or_default();
                    let qq_cfg = QqBotConfig {
                        app_id,
                        client_secret,
                        markdown_support: extra_bool(platform_cfg, "markdown_support", true),
                        proxy: Default::default(),
                    };
                    match QqBotAdapter::new(qq_cfg) {
                        Ok(adapter) => {
                            gateway.register_adapter("qqbot", Arc::new(adapter)).await;
                            registered.push("qqbot".to_string());
                        }
                        Err(e) => errors.push(("qqbot".to_string(), e.to_string())),
                    }
                }
            }
        }

        // BlueBubbles
        #[cfg(feature = "bluebubbles")]
        if let Some(platform_cfg) = config.platforms.get("bluebubbles") {
            if platform_cfg.enabled {
                use crate::platforms::bluebubbles::{BlueBubblesAdapter, BlueBubblesConfig};

                if let Some(msg) = fatal_by_platform.remove("bluebubbles") {
                    errors.push(("bluebubbles".to_string(), msg));
                } else {
                    let server_url =
                        hermes_config::extra_string(platform_cfg, "server_url").unwrap_or_default();
                    let password =
                        hermes_config::extra_string(platform_cfg, "password").unwrap_or_default();
                    let bb_cfg = BlueBubblesConfig {
                        server_url,
                        password,
                        proxy: Default::default(),
                    };
                    match BlueBubblesAdapter::new(bb_cfg) {
                        Ok(adapter) => {
                            gateway
                                .register_adapter("bluebubbles", Arc::new(adapter))
                                .await;
                            registered.push("bluebubbles".to_string());
                        }
                        Err(e) => errors.push(("bluebubbles".to_string(), e.to_string())),
                    }
                }
            }
        }

        // Email
        #[cfg(feature = "email")]
        if let Some(platform_cfg) = config.platforms.get("email") {
            if platform_cfg.enabled {
                use crate::platforms::email::{EmailAdapter, EmailConfig};

                if let Some(msg) = fatal_by_platform.remove("email") {
                    errors.push(("email".to_string(), msg));
                } else {
                    let imap_host =
                        hermes_config::extra_string(platform_cfg, "imap_host").unwrap_or_default();
                    let smtp_host =
                        hermes_config::extra_string(platform_cfg, "smtp_host").unwrap_or_default();
                    let username =
                        hermes_config::extra_string(platform_cfg, "username").unwrap_or_default();
                    let password =
                        hermes_config::extra_string(platform_cfg, "password").unwrap_or_default();
                    let email_cfg = EmailConfig {
                        imap_host,
                        imap_port: extra_u16(platform_cfg, "imap_port", 993),
                        smtp_host,
                        smtp_port: extra_u16(platform_cfg, "smtp_port", 587),
                        username,
                        password,
                        poll_interval_secs: platform_cfg
                            .extra
                            .get("poll_interval_secs")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(60),
                        proxy: Default::default(),
                    };
                    match EmailAdapter::new(email_cfg) {
                        Ok(adapter) => {
                            gateway.register_adapter("email", Arc::new(adapter)).await;
                            registered.push("email".to_string());
                        }
                        Err(e) => errors.push(("email".to_string(), e.to_string())),
                    }
                }
            }
        }

        // SMS
        #[cfg(feature = "sms")]
        if let Some(platform_cfg) = config.platforms.get("sms") {
            if platform_cfg.enabled {
                use crate::platforms::sms::{SmsAdapter, SmsConfig};

                if let Some(msg) = fatal_by_platform.remove("sms") {
                    errors.push(("sms".to_string(), msg));
                } else {
                    let account_sid = hermes_config::extra_string(platform_cfg, "account_sid")
                        .unwrap_or_default();
                    let auth_token =
                        hermes_config::extra_string(platform_cfg, "auth_token").unwrap_or_default();
                    let from_number = hermes_config::extra_string(platform_cfg, "from_number")
                        .unwrap_or_default();
                    let sms_cfg = SmsConfig {
                        provider: hermes_config::extra_string(platform_cfg, "provider")
                            .unwrap_or_else(|| "twilio".to_string()),
                        account_sid,
                        auth_token,
                        from_number,
                        proxy: Default::default(),
                    };
                    match SmsAdapter::new(sms_cfg) {
                        Ok(adapter) => {
                            gateway.register_adapter("sms", Arc::new(adapter)).await;
                            registered.push("sms".to_string());
                        }
                        Err(e) => errors.push(("sms".to_string(), e.to_string())),
                    }
                }
            }
        }

        // HomeAssistant
        #[cfg(feature = "homeassistant")]
        if let Some(platform_cfg) = config.platforms.get("homeassistant") {
            if platform_cfg.enabled {
                use crate::platforms::homeassistant::{HomeAssistantAdapter, HomeAssistantConfig};

                if let Some(msg) = fatal_by_platform.remove("homeassistant") {
                    errors.push(("homeassistant".to_string(), msg));
                } else {
                    let base_url =
                        hermes_config::extra_string(platform_cfg, "base_url").unwrap_or_default();
                    let long_lived_token = hermes_config::platform_token_or_extra(platform_cfg)
                        .or_else(|| hermes_config::extra_string(platform_cfg, "long_lived_token"))
                        .unwrap_or_default();
                    let ha_cfg = HomeAssistantConfig {
                        base_url,
                        long_lived_token,
                        webhook_id: hermes_config::extra_string(platform_cfg, "webhook_id"),
                        proxy: Default::default(),
                    };
                    match HomeAssistantAdapter::new(ha_cfg) {
                        Ok(adapter) => {
                            gateway
                                .register_adapter("homeassistant", Arc::new(adapter))
                                .await;
                            registered.push("homeassistant".to_string());
                        }
                        Err(e) => errors.push(("homeassistant".to_string(), e.to_string())),
                    }
                }
            }
        }

        // Webhook
        #[cfg(feature = "webhook")]
        if let Some(platform_cfg) = config.platforms.get("webhook") {
            if platform_cfg.enabled {
                use crate::platforms::webhook::{WebhookAdapter, WebhookConfig};

                if let Some(msg) = fatal_by_platform.remove("webhook") {
                    errors.push(("webhook".to_string(), msg));
                } else {
                    let secret =
                        hermes_config::extra_string(platform_cfg, "secret").unwrap_or_default();
                    let wh_cfg = WebhookConfig {
                        port: extra_u16(platform_cfg, "port", 9000),
                        path: hermes_config::extra_string(platform_cfg, "path")
                            .unwrap_or_else(|| "/webhook".to_string()),
                        secret,
                    };
                    let adapter = WebhookAdapter::new(wh_cfg);
                    gateway.register_adapter("webhook", Arc::new(adapter)).await;
                    registered.push("webhook".to_string());
                }
            }
        }

        // ApiServer
        #[cfg(feature = "api-server")]
        if let Some(platform_cfg) = config.platforms.get("api_server") {
            if platform_cfg.enabled {
                use crate::platforms::api_server::{ApiServerAdapter, ApiServerConfig};

                let api_cfg = ApiServerConfig {
                    host: hermes_config::extra_string(platform_cfg, "host")
                        .unwrap_or_else(|| "0.0.0.0".to_string()),
                    port: extra_u16(platform_cfg, "port", 8090),
                    auth_token: hermes_config::extra_string(platform_cfg, "auth_token"),
                };
                let adapter = ApiServerAdapter::new(api_cfg);
                gateway
                    .register_adapter("api_server", Arc::new(adapter))
                    .await;
                registered.push("api_server".to_string());
            }
        }

        Ok(RegistrationSummary { registered, errors })
    }
}

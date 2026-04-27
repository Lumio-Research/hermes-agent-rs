use std::sync::Arc;

use hermes_config::session::SessionConfig;
use hermes_config::GatewayConfig;
#[cfg(any(feature = "api-server", feature = "webhook"))]
use hermes_config::PlatformConfig;
use hermes_gateway::platform_registry::register_platforms;
use hermes_gateway::{DmManager, Gateway, SessionManager};

fn make_gateway() -> Gateway {
    let session_manager = Arc::new(SessionManager::new(SessionConfig::default()));
    let dm = DmManager::with_pair_behavior();
    Gateway::new(
        session_manager,
        dm,
        hermes_gateway::gateway::GatewayConfig::default(),
    )
}

#[tokio::test]
async fn register_platforms_with_all_disabled_returns_empty_summary() {
    let gateway = make_gateway();
    let config = GatewayConfig::default();
    let mut sidecar_tasks = Vec::new();

    let summary = register_platforms(&gateway, &config, &mut sidecar_tasks)
        .await
        .expect("registration should succeed");

    for handle in sidecar_tasks {
        handle.abort();
    }

    assert!(summary.registered.is_empty());
    assert!(summary.errors.is_empty());
    assert!(gateway.adapter_names().await.is_empty());
}

#[cfg(feature = "api-server")]
#[tokio::test]
async fn register_platforms_registers_enabled_api_server() {
    let gateway = make_gateway();
    let mut config = GatewayConfig::default();
    let mut api_cfg = PlatformConfig {
        enabled: true,
        ..PlatformConfig::default()
    };
    api_cfg
        .extra
        .insert("host".to_string(), serde_json::json!("127.0.0.1"));
    api_cfg
        .extra
        .insert("port".to_string(), serde_json::json!(0));
    config.platforms.insert("api_server".to_string(), api_cfg);

    let mut sidecar_tasks = Vec::new();
    let summary = register_platforms(&gateway, &config, &mut sidecar_tasks)
        .await
        .expect("registration should succeed");

    for handle in sidecar_tasks {
        handle.abort();
    }

    assert!(summary.errors.is_empty(), "unexpected registration errors");
    assert!(
        summary.registered.iter().any(|name| name == "api_server"),
        "api_server should be registered"
    );
    assert!(
        gateway
            .adapter_names()
            .await
            .iter()
            .any(|name| name == "api_server"),
        "gateway should expose api_server adapter"
    );
}

#[cfg(feature = "webhook")]
#[tokio::test]
async fn register_platforms_reports_webhook_missing_secret() {
    let gateway = make_gateway();
    let mut config = GatewayConfig::default();
    config.platforms.insert(
        "webhook".to_string(),
        PlatformConfig {
            enabled: true,
            ..PlatformConfig::default()
        },
    );
    let mut sidecar_tasks = Vec::new();

    let summary = register_platforms(&gateway, &config, &mut sidecar_tasks)
        .await
        .expect("registration should succeed");

    for handle in sidecar_tasks {
        handle.abort();
    }

    assert!(
        summary.errors.iter().any(|(name, msg)| {
            name == "webhook" && (msg.contains("secret") || msg.contains("缺少"))
        }),
        "missing webhook secret should be reported as registration error"
    );
    assert!(
        !summary.registered.iter().any(|name| name == "webhook"),
        "webhook should not be registered when secret is missing"
    );
}

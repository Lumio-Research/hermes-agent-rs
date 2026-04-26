use std::collections::HashSet;
use std::sync::Arc;

use hermes_config::session::SessionConfig;
use hermes_config::{GatewayConfig, PlatformConfig};
use hermes_gateway::platform_registry::register_platforms;
use hermes_gateway::{DmManager, Gateway, SessionManager};
use proptest::prelude::*;

fn make_gateway() -> Gateway {
    let session_manager = Arc::new(SessionManager::new(SessionConfig::default()));
    let dm = DmManager::with_pair_behavior();
    Gateway::new(session_manager, dm, hermes_gateway::gateway::GatewayConfig::default())
}

#[allow(unused_variables, unused_mut)]
fn expected_for_compiled_features(enabled: &HashSet<String>) -> HashSet<String> {
    let mut out = HashSet::new();
    #[cfg(feature = "api-server")]
    if enabled.contains("api_server") {
        out.insert("api_server".to_string());
    }
    #[cfg(feature = "webhook")]
    if enabled.contains("webhook") {
        out.insert("webhook".to_string());
    }
    #[cfg(feature = "telegram")]
    if enabled.contains("telegram") {
        out.insert("telegram".to_string());
    }
    out
}

proptest! {
    // Feature: unified-runtime-architecture, Property 4: Platform registration matches enabled config
    #[test]
    fn register_platforms_matches_enabled_set(
        enable_api_server in any::<bool>(),
        enable_webhook in any::<bool>(),
        enable_telegram in any::<bool>(),
    ) {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let actual = rt.block_on(async {
            let gateway = make_gateway();
            let mut cfg = GatewayConfig::default();
            let mut expected_enabled = HashSet::new();

            let mut api_cfg = PlatformConfig { enabled: enable_api_server, ..PlatformConfig::default() };
            api_cfg.extra.insert("host".into(), serde_json::json!("127.0.0.1"));
            api_cfg.extra.insert("port".into(), serde_json::json!(0));
            cfg.platforms.insert("api_server".into(), api_cfg);
            if enable_api_server {
                expected_enabled.insert("api_server".to_string());
            }

            let mut webhook_cfg = PlatformConfig { enabled: enable_webhook, ..PlatformConfig::default() };
            webhook_cfg.extra.insert("secret".into(), serde_json::json!("s"));
            cfg.platforms.insert("webhook".into(), webhook_cfg);
            if enable_webhook {
                expected_enabled.insert("webhook".to_string());
            }

            let telegram_cfg = PlatformConfig {
                enabled: enable_telegram,
                token: Some("t".to_string()),
                ..PlatformConfig::default()
            };
            cfg.platforms.insert("telegram".into(), telegram_cfg);
            if enable_telegram {
                expected_enabled.insert("telegram".to_string());
            }

            let mut sidecar = Vec::new();
            let summary = register_platforms(&gateway, &cfg, &mut sidecar)
                .await
                .expect("register");
            for handle in sidecar {
                handle.abort();
            }

            let expected = expected_for_compiled_features(&expected_enabled);
            (summary.registered.into_iter().collect::<HashSet<String>>(), expected)
        });
        prop_assert_eq!(actual.0, actual.1);
    }
}

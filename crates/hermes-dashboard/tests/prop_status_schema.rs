use std::collections::HashMap;

use hermes_dashboard::dashboard::types::StatusResponse;
use proptest::prelude::*;

proptest! {
    // Feature: unified-runtime-architecture, Property 7: REST API response schema stability
    #[test]
    fn status_response_has_required_field_types(
        active_sessions in any::<u32>(),
        gateway_running in any::<bool>(),
        pid in prop::option::of(any::<u32>()),
        home in ".*",
    ) {
        let response = StatusResponse {
            version: "0.1.1".to_string(),
            release_date: "".to_string(),
            hermes_home: home.clone(),
            config_path: format!("{home}/config.yaml"),
            env_path: format!("{home}/.env"),
            config_version: 1,
            latest_config_version: 1,
            active_sessions,
            gateway_running,
            gateway_pid: pid,
            gateway_state: if gateway_running { Some("running".to_string()) } else { None },
            gateway_health_url: None,
            gateway_exit_reason: None,
            gateway_updated_at: None,
            gateway_platforms: HashMap::new(),
        };

        let v = serde_json::to_value(response).expect("serialize");
        prop_assert!(v.get("version").and_then(|x| x.as_str()).is_some());
        prop_assert!(v.get("hermes_home").and_then(|x| x.as_str()).is_some());
        prop_assert!(v.get("config_path").and_then(|x| x.as_str()).is_some());
        prop_assert!(v.get("active_sessions").and_then(|x| x.as_u64()).is_some());
        prop_assert!(v.get("gateway_running").and_then(|x| x.as_bool()).is_some());
        prop_assert!(v.get("gateway_platforms").and_then(|x| x.as_object()).is_some());
    }
}

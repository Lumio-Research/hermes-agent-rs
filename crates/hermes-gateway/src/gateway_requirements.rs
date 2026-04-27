//! Static checks: enabled platforms must have required credentials before the
//! runtime starts the gateway (mirrors [`crate::platform_registry`] predicates).
//!
//! Credential rules live in [`crate::platform_requirements`] — this module keeps
//! the legacy `Vec<String>` API for existing callers.

use hermes_config::GatewayConfig;

use crate::platform_requirements::{
    evaluate_gateway_requirements, RequirementScope, RequirementSeverity,
};

/// Return human-readable issues for any **enabled** platform that is missing
/// required fields. Empty list means the static credential check passed.
///
/// Each block is behind the same `feature` flag as
/// [`crate::platform_registry::register_platforms`].
pub fn gateway_requirement_issues(config: &GatewayConfig) -> Vec<String> {
    evaluate_gateway_requirements(config, RequirementScope::RuntimeStart)
        .into_iter()
        .filter(|i| i.severity == RequirementSeverity::Fatal)
        .map(|i| i.message)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hermes_config::{GatewayConfig, PlatformConfig};

    fn make_platform(enabled: bool, token: Option<&str>) -> PlatformConfig {
        let mut cfg = PlatformConfig {
            enabled,
            ..Default::default()
        };
        if let Some(t) = token {
            cfg.token = Some(t.to_string());
        }
        cfg
    }

    #[test]
    fn matrix_incomplete_surfaces_issue_when_feature_on() {
        #[cfg(feature = "matrix")]
        {
            let mut config = GatewayConfig::default();
            let mut m = make_platform(true, None);
            m.extra.insert(
                "homeserver_url".to_string(),
                serde_json::json!("https://m.org"),
            );
            config.platforms.insert("matrix".to_string(), m);
            let issues = gateway_requirement_issues(&config);
            assert!(issues.iter().any(|s| s.contains("matrix")), "{issues:?}");
        }
    }

    #[test]
    fn matrix_complete_no_matrix_issue() {
        #[cfg(feature = "matrix")]
        {
            let mut config = GatewayConfig::default();
            let mut m = make_platform(true, Some("tok"));
            m.extra.insert(
                "homeserver_url".to_string(),
                serde_json::json!("https://m.org"),
            );
            m.extra
                .insert("user_id".to_string(), serde_json::json!("@u:m.org"));
            config.platforms.insert("matrix".to_string(), m);
            let issues = gateway_requirement_issues(&config);
            assert!(!issues.iter().any(|s| s.contains("matrix")), "{issues:?}");
        }
    }
}

use std::net::SocketAddr;

use hermes_config::load_config;
use hermes_core::AgentError;
use hermes_telemetry::init_telemetry_from_env;

#[tokio::main]
async fn main() -> Result<(), AgentError> {
    init_telemetry_from_env("hermes-server", "info");

    let config = load_config(None).map_err(|e| AgentError::Config(e.to_string()))?;
    let addr: SocketAddr = std::env::var("HERMES_SERVER_ADDR")
        .or_else(|_| std::env::var("HERMES_DASHBOARD_ADDR"))
        .or_else(|_| std::env::var("HERMES_HTTP_ADDR"))
        .unwrap_or_else(|_| "127.0.0.1:8787".to_string())
        .parse()
        .map_err(|e| AgentError::Config(format!("invalid server addr: {}", e)))?;

    hermes_server::run_server(addr, config).await
}

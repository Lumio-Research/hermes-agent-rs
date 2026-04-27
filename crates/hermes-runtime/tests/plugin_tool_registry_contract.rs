//! Contract: plugin tools installed into [`hermes_tools::ToolRegistry`] appear in the
//! agent bridge (same path `RuntimeBuilder::run` uses before `LocalAgentService`).

use std::sync::Arc;

use async_trait::async_trait;
use hermes_agent::agent_builder::bridge_tool_registry;
use hermes_agent::plugins::PluginContext;
use hermes_agent::{install_plugin_tools_into_registry, Plugin, PluginManager, PluginMeta};
use hermes_core::tool_schema::JsonSchema;
use hermes_core::{AgentError, ToolHandler, ToolSchema};
use hermes_tools::ToolRegistry;

struct EchoContractTool;
#[async_trait]
impl ToolHandler for EchoContractTool {
    async fn execute(&self, params: serde_json::Value) -> Result<String, hermes_core::ToolError> {
        Ok(params.to_string())
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema::new(
            "echo_plugin_contract",
            "contract test tool",
            JsonSchema::new("object"),
        )
    }
}

struct RegContractPlugin;
#[async_trait]
impl Plugin for RegContractPlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta {
            name: "contract".into(),
            version: "0.0.1".into(),
            description: "contract".into(),
            author: None,
        }
    }
    async fn initialize(&self) -> Result<(), AgentError> {
        Ok(())
    }
    async fn shutdown(&self) -> Result<(), AgentError> {
        Ok(())
    }
    fn register(&self, ctx: &mut PluginContext) {
        let h: Arc<dyn ToolHandler> = Arc::new(EchoContractTool);
        let schema = h.schema();
        ctx.register_tool(schema, h);
    }
}

#[test]
fn plugin_tools_reach_bridged_agent_registry() {
    let registry = ToolRegistry::new();
    let mut pm = PluginManager::new();
    pm.register(Arc::new(RegContractPlugin));
    install_plugin_tools_into_registry(&registry, &pm);

    let bridged = bridge_tool_registry(&registry);
    assert!(
        bridged.names().iter().any(|n| n == "echo_plugin_contract"),
        "bridged registry should list plugin tool; got {:?}",
        bridged.names()
    );
}

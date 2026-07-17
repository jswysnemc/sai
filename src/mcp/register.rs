use super::client::{call_server_tool, dynamic_tool_name, list_enabled_tools};
use crate::config::{AppConfig, McpServerConfig};
use crate::tools::{ToolRegistry, ToolSpec};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// 将已启用 MCP server 的工具注册到工具表。
pub fn register_mcp_tools(registry: &mut ToolRegistry, config: &AppConfig) {
    if !config.mcp.enabled || config.mcp.servers.is_empty() {
        return;
    }
    let servers = config.mcp.servers.clone();
    // 独立线程 + 独立 runtime，避免在已有 tokio runtime 内 block_on
    let tools = std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                eprintln!("[mcp] runtime error: {error}");
                return Vec::new();
            }
        };
        runtime.block_on(list_enabled_tools(&servers))
    })
    .join()
    .unwrap_or_else(|_| {
        eprintln!("[mcp] list tools thread panicked");
        Vec::new()
    });

    let server_index: Arc<HashMap<String, McpServerConfig>> = Arc::new(
        config
            .mcp
            .servers
            .iter()
            .cloned()
            .map(|server| (server.id.clone(), server))
            .collect(),
    );
    for tool in tools {
        let dynamic_name = dynamic_tool_name(&tool.server_id, &tool.name);
        if registry.contains(&dynamic_name) {
            continue;
        }
        let server_id = tool.server_id.clone();
        let remote_name = tool.name.clone();
        let servers = Arc::clone(&server_index);
        let description = if tool.description.trim().is_empty() {
            format!("MCP tool {remote_name} from server {server_id}")
        } else {
            format!("[MCP:{server_id}] {}", tool.description)
        };
        let parameters = if tool.input_schema.is_null() {
            json!({"type":"object","properties":{}})
        } else {
            tool.input_schema.clone()
        };
        registry.register(
            ToolSpec::new(
                dynamic_name,
                description,
                parameters,
                move |args: Value| {
                    let servers = Arc::clone(&servers);
                    let server_id = server_id.clone();
                    let remote_name = remote_name.clone();
                    async move {
                        let Some(server) = servers.get(&server_id) else {
                            anyhow::bail!("mcp server not found: {server_id}");
                        };
                        call_server_tool(server, &remote_name, args).await
                    }
                },
            )
            .writes(),
        );
    }
}

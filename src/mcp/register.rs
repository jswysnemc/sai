use super::client::{call_server_tool, dynamic_tool_name, list_server_tools_on_rt};
use super::tool_cache;
use crate::config::{AppConfig, McpServerConfig};
use crate::paths::SaiPaths;
use crate::tools::{ToolRegistry, ToolSpec};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// 将已启用 MCP server 的工具注册到工具表。
pub fn register_mcp_tools(registry: &mut ToolRegistry, config: &AppConfig, paths: &SaiPaths) {
    if !config.mcp.enabled || config.mcp.servers.is_empty() {
        return;
    }
    let servers = config.mcp.servers.clone();
    let cache_paths = paths.clone();
    // 在专用 MCP runtime 上列举工具（stdio Child 与连接池绑定该 runtime）
    // 从主 runtime 的同步路径调用时，放到独立线程避免嵌套 block_on
    let tools = std::thread::spawn(move || {
        super::client::block_on_mcp(async move {
            let mut all = Vec::new();
            for server in servers.iter().filter(|server| server.enabled) {
                match list_server_tools_on_rt(server).await {
                    Ok(mut tools) => {
                        if let Err(error) = tool_cache::store_server(&cache_paths, server, &tools) {
                            eprintln!("[mcp] cache tools for {}: {error}", server.id);
                        }
                        all.append(&mut tools);
                    }
                    Err(error) => eprintln!("[mcp] list tools for {}: {error}", server.id),
                }
            }
            all
        })
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
            ToolSpec::new(dynamic_name, description, parameters, move |args: Value| {
                let servers = Arc::clone(&servers);
                let server_id = server_id.clone();
                let remote_name = remote_name.clone();
                async move {
                    let Some(server) = servers.get(&server_id) else {
                        anyhow::bail!("mcp server not found: {server_id}");
                    };
                    call_server_tool(server, &remote_name, args).await
                }
            })
            .writes(),
        );
    }
}

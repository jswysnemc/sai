use super::client::{
    list_server_tools, runtime_statuses, stop_all_servers, stop_server, test_server,
};
use crate::config::{AppConfig, McpServerConfig};
use crate::paths::SaiPaths;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{bail, Result};
use serde_json::{json, Value};

/// 注册 MCP 管理工具，供模型查询/测试/启停 server。
pub fn register_mcp_manager(registry: &mut ToolRegistry, paths: SaiPaths) {
    registry.register(
        ToolSpec::new(
            "mcp_manager",
            "Manage MCP servers: list_status, list_tools, test, stop, stop_all. Does not permanently edit config; use Settings for CRUD.",
            json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["list_status", "list_tools", "test", "stop", "stop_all"],
                        "description": "Management action"
                    },
                    "server_id": {
                        "type": "string",
                        "description": "Target server id for list_tools/test/stop"
                    }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
            move |args: Value| {
                let paths = paths.clone();
                async move { run_manager(&paths, args).await }
            },
        )
        .writes(),
    );
}

async fn run_manager(paths: &SaiPaths, args: Value) -> Result<String> {
    let action = args
        .get("action")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let server_id = args
        .get("server_id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let config = AppConfig::load_or_default(paths)?;
    if !config.mcp.enabled {
        bail!("MCP is disabled in config");
    }
    match action.as_str() {
        "list_status" => {
            let statuses = runtime_statuses(&config.mcp.servers).await;
            Ok(serde_json::to_string_pretty(&json!({
                "ok": true,
                "enabled": config.mcp.enabled,
                "servers": config.mcp.servers.iter().map(|server| {
                    let status = statuses.iter().find(|item| item.server_id == server.id);
                    json!({
                        "id": server.id,
                        "enabled": server.enabled,
                        "transport": server.transport,
                        "command": server.command,
                        "url": server.url,
                        "running": status.map(|item| item.running).unwrap_or(false),
                        "initialized": status.map(|item| item.initialized).unwrap_or(false),
                        "last_error": status.and_then(|item| item.last_error.clone()),
                    })
                }).collect::<Vec<_>>()
            }))?)
        }
        "list_tools" => {
            let server = require_server(&config, server_id.as_deref())?;
            let tools = list_server_tools(&server).await?;
            Ok(serde_json::to_string_pretty(&json!({
                "ok": true,
                "server_id": server.id,
                "tools": tools.iter().map(|tool| json!({
                    "name": tool.name,
                    "description": tool.description,
                })).collect::<Vec<_>>()
            }))?)
        }
        "test" => {
            let server = require_server(&config, server_id.as_deref())?;
            let (count, names) = test_server(&server).await?;
            Ok(serde_json::to_string_pretty(&json!({
                "ok": true,
                "server_id": server.id,
                "tools_count": count,
                "tools": names,
            }))?)
        }
        "stop" => {
            let id = server_id.ok_or_else(|| anyhow::anyhow!("server_id is required"))?;
            let stopped = stop_server(&id).await;
            Ok(serde_json::to_string_pretty(&json!({
                "ok": true,
                "server_id": id,
                "stopped": stopped,
            }))?)
        }
        "stop_all" => {
            stop_all_servers().await;
            Ok(serde_json::to_string_pretty(&json!({
                "ok": true,
                "stopped_all": true,
            }))?)
        }
        _ => bail!("unsupported mcp_manager action: {action}"),
    }
}

fn require_server(config: &AppConfig, server_id: Option<&str>) -> Result<McpServerConfig> {
    let id = server_id.ok_or_else(|| anyhow::anyhow!("server_id is required"))?;
    config
        .mcp
        .servers
        .iter()
        .find(|server| server.id == id)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("mcp server not found: {id}"))
}

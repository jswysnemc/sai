use super::client::{call_server_tool, dynamic_tool_name, list_server_tools_on_rt};
use super::tool_cache;
use crate::config::{AppConfig, McpServerConfig};
use crate::paths::SaiPaths;
use crate::tools::{ToolRegistry, ToolSpec};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

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

/// 从持久化缓存注册延迟连接的 MCP 工具。
///
/// 参数:
/// - `registry`: 当前工具注册表
/// - `config`: 当前应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 无
pub fn register_cached_mcp_tools(
    registry: &mut ToolRegistry,
    config: &AppConfig,
    paths: &SaiPaths,
) {
    if !config.mcp.enabled || config.mcp.servers.is_empty() {
        return;
    }
    let tools = tool_cache::load(paths, &config.mcp.servers);
    let server_index: Arc<HashMap<String, McpServerConfig>> = Arc::new(
        config
            .mcp
            .servers
            .iter()
            .filter(|server| server.enabled)
            .cloned()
            .map(|server| (server.id.clone(), server))
            .collect(),
    );
    let refreshed_servers = Arc::new(Mutex::new(HashSet::<String>::new()));
    for tool in tools {
        let dynamic_name = dynamic_tool_name(&tool.server_id, &tool.name);
        if registry.contains(&dynamic_name) {
            continue;
        }
        let server_id = tool.server_id.clone();
        let remote_name = tool.name.clone();
        let servers = Arc::clone(&server_index);
        let refreshed_servers = Arc::clone(&refreshed_servers);
        let cache_paths = paths.clone();
        let description = tool_description(&server_id, &remote_name, &tool.description);
        let parameters = tool_parameters(&tool.input_schema);
        registry.register(
            ToolSpec::new(dynamic_name, description, parameters, move |args: Value| {
                let servers = Arc::clone(&servers);
                let refreshed_servers = Arc::clone(&refreshed_servers);
                let cache_paths = cache_paths.clone();
                let server_id = server_id.clone();
                let remote_name = remote_name.clone();
                async move {
                    let Some(server) = servers.get(&server_id) else {
                        anyhow::bail!("mcp server not found: {server_id}");
                    };
                    // 1. 首次调用该服务工具时连接服务并刷新完整工具定义
                    let mut refreshed = refreshed_servers.lock().await;
                    if !refreshed.contains(&server_id) {
                        let tools = super::client::list_server_tools(server).await?;
                        tool_cache::store_server(&cache_paths, server, &tools)?;
                        refreshed.insert(server_id.clone());
                    }
                    drop(refreshed);
                    // 2. 复用刚建立的连接执行目标工具
                    call_server_tool(server, &remote_name, args).await
                }
            })
            .writes(),
        );
    }
}

/// 构造 MCP 工具展示说明。
///
/// 参数:
/// - `server_id`: MCP 服务标识
/// - `remote_name`: 远端工具名称
/// - `description`: 远端工具说明
///
/// 返回:
/// - 带 MCP 来源标识的工具说明
fn tool_description(server_id: &str, remote_name: &str, description: &str) -> String {
    if description.trim().is_empty() {
        format!("MCP tool {remote_name} from server {server_id}")
    } else {
        format!("[MCP:{server_id}] {description}")
    }
}

/// 规范 MCP 工具参数定义。
///
/// 参数:
/// - `input_schema`: MCP 返回的输入结构
///
/// 返回:
/// - 可注册到模型工具定义中的 JSON Schema
fn tool_parameters(input_schema: &Value) -> Value {
    if input_schema.is_null() {
        json!({"type":"object","properties":{}})
    } else {
        input_schema.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造延迟注册测试使用的 Sai 路径。
    ///
    /// 参数:
    /// - `root`: 临时目录根路径
    ///
    /// 返回:
    /// - 指向临时目录的路径集合
    fn test_paths(root: &std::path::Path) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config.jsonc"),
            secrets_file: root.join("secrets.jsonc"),
            skills_dir: root.join("skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("sai.fish"),
            bash_hook_file: root.join("bash-hook.sh"),
            zsh_hook_file: root.join("zsh-hook.zsh"),
            powershell_hook_file: root.join("powershell-hook.ps1"),
        }
    }

    /// 构造不会成功启动的 MCP 服务配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 已启用的无效 stdio 服务配置
    fn unavailable_server() -> McpServerConfig {
        McpServerConfig {
            id: "lazy-cache-test-server".to_string(),
            enabled: true,
            transport: "stdio".to_string(),
            command: "sai-missing-mcp-test-command".to_string(),
            args: Vec::new(),
            env: Default::default(),
            cwd: None,
            url: None,
            message_url: None,
            headers: Default::default(),
            timeout_ms: Some(100),
        }
    }

    #[tokio::test]
    async fn cached_registration_defers_server_start_until_tool_call() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let server = unavailable_server();
        let tool = super::super::client::McpToolInfo {
            server_id: server.id.clone(),
            name: "cached_tool".to_string(),
            description: "Cached tool".to_string(),
            input_schema: json!({"type":"object","properties":{}}),
        };
        tool_cache::store_server(&paths, &server, std::slice::from_ref(&tool)).unwrap();
        let mut config = AppConfig::default();
        config.mcp.enabled = true;
        config.mcp.servers = vec![server.clone()];
        let mut registry = ToolRegistry::new();

        register_cached_mcp_tools(&mut registry, &config, &paths);

        let dynamic_name = dynamic_tool_name(&server.id, &tool.name);
        assert!(registry.contains(&dynamic_name));
        let statuses = super::super::client::runtime_statuses(std::slice::from_ref(&server)).await;
        assert!(!statuses[0].running);
        assert!(registry.call(&dynamic_name, "{}").await.is_err());
    }
}

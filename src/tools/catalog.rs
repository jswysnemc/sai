use super::{builtin_registry, builtin_registry_without_mcp, groups, ToolRegistry};
use crate::config::AppConfig;
use crate::paths::SaiPaths;

/// 内置工具目录条目。
pub(crate) struct ToolCatalogEntry {
    /// 工具名称
    pub name: String,
    /// 用途分组标识
    pub group: &'static str,
    /// 用途分组展示名
    pub group_label: &'static str,
    /// 工具摘要说明
    pub description: String,
}

/// 枚举本地工具及其分组，不连接外部 MCP 服务。
///
/// Agent 设置页只需要工具元数据。MCP 工具发现属于运行时操作，不能阻塞配置界面。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `paths`: 应用目录路径集合
///
/// 返回:
/// - 按工具名排序的目录条目列表
pub(crate) fn tool_catalog(config: &AppConfig, paths: &SaiPaths) -> Vec<ToolCatalogEntry> {
    // 1. 构建本地注册表，跳过 MCP 网络与子进程发现
    let registry = builtin_registry_without_mcp(config, paths);
    // 2. 为每个工具附加用途分组与摘要
    let mut entries = catalog_entries(registry);
    entries.extend([
        catalog_entry("subagent".to_string(), "启动子任务代理".to_string()),
        catalog_entry("todo".to_string(), "管理待办任务清单".to_string()),
        catalog_entry(
            "ask_question".to_string(),
            "向用户提出结构化问题并等待回答".to_string(),
        ),
    ]);
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    entries.dedup_by(|left, right| left.name == right.name);
    entries
}

/// 枚举 MCP 动态工具，供设置页后台补充选项。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `paths`: 应用目录路径集合
///
/// 返回:
/// - MCP 服务发现到的动态工具目录项
pub(crate) fn mcp_tool_catalog(config: &AppConfig, paths: &SaiPaths) -> Vec<ToolCatalogEntry> {
    catalog_entries(builtin_registry(config, paths))
        .into_iter()
        .filter(|entry| entry.group == "mcp" && entry.name != "mcp_manager")
        .collect()
}

/// 将注册表转换为排序后的目录项。
fn catalog_entries(registry: ToolRegistry) -> Vec<ToolCatalogEntry> {
    let mut entries = registry
        .tool_infos()
        .into_iter()
        .map(|info| catalog_entry(info.name, info.description))
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    entries
}

/// 将工具元数据转换为设置页目录项。
fn catalog_entry(name: String, description: String) -> ToolCatalogEntry {
    let group = groups::group_for_tool(&name);
    ToolCatalogEntry {
        name,
        group,
        group_label: groups::group_description(group),
        description: summarize_tool_description(&description),
    }
}

/// 截取工具描述首句作为配置界面摘要。
fn summarize_tool_description(description: &str) -> String {
    description
        .split(['.', '。'])
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(description.trim())
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn catalog_does_not_discover_mcp_servers() {
        let mut config = AppConfig::default();
        config.mcp.enabled = true;
        let (command, args) = if cfg!(windows) {
            (
                "cmd".to_string(),
                vec!["/C".to_string(), "ping -n 3 127.0.0.1 >NUL".to_string()],
            )
        } else {
            (
                "sh".to_string(),
                vec!["-c".to_string(), "sleep 2".to_string()],
            )
        };
        config.mcp.servers.push(crate::config::McpServerConfig {
            id: "slow-server".to_string(),
            enabled: true,
            transport: "stdio".to_string(),
            command,
            args,
            env: Default::default(),
            cwd: None,
            url: None,
            message_url: None,
            headers: Default::default(),
            timeout_ms: Some(500),
        });
        let paths = SaiPaths::new().unwrap();
        let started = Instant::now();

        let entries = tool_catalog(&config, &paths);

        assert!(started.elapsed() < Duration::from_millis(250));
        assert!(entries.iter().any(|entry| entry.name == "mcp_manager"));
        assert!(!entries
            .iter()
            .any(|entry| entry.name.starts_with("mcp_slow_server_")));
    }
}

use super::descriptions::tool_description;
use super::{
    builtin_registry, builtin_registry_without_mcp, groups, register_interactive_tools,
    ToolRegistry,
};
use crate::config::AppConfig;
use crate::paths::SaiPaths;

/// 目录使用的占位会话标识。
///
/// 目录只要工具元数据，这个标识不会落到任何会话状态里。
const CATALOG_SESSION_ID: &str = "tool-catalog";

/// 内置工具目录条目。
pub(crate) struct ToolCatalogEntry {
    /// 工具名称
    pub name: String,
    /// 用途分组标识
    pub group: &'static str,
    /// 是否属于常驻集合：延迟集合含通配符时这些工具仍然直接可见
    pub resident: bool,
    /// 用途分组中文短标题
    pub group_label: &'static str,
    /// 用途分组英文短标题
    pub group_label_en: &'static str,
    /// 分组中文交互说明；空表示无需额外解释
    pub group_hint: &'static str,
    /// 分组英文交互说明
    pub group_hint_en: &'static str,
    /// 相关设置页路径
    pub group_settings_path: Option<&'static str>,
    /// 设置页排序权重，越小越靠前
    pub group_rank: u8,
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
    let mut entries = catalog_entries(catalog_registry(config, paths));
    // 渠道发送工具只在网关收到入站消息、拿到渠道上下文后才注册，目录侧没有
    // 渠道可绑定，只能按名字补一条；否则网关 Agent 的配置页看不到这个工具，
    // 而它却写在该 Agent 的默认白名单里。
    entries.push(catalog_entry(
        "send_channel_message".to_string(),
        tool_description("send_channel_message", "Send a channel message."),
    ));
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    entries.dedup_by(|left, right| left.name == right.name);
    entries
}

/// 构造目录使用的工具注册表。
///
/// 必须和真实交互式会话共用 `register_interactive_tools`：网格工具、子智能体、
/// 任务清单与结构化提问都只在那条路径上注册。早先目录自己抄了一份名单，
/// 于是网格工具虽然注册成功，配置界面上却不存在，用户也就无从选择。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `paths`: 应用目录路径集合
///
/// 返回:
/// - 覆盖交互式会话全部工具的注册表
fn catalog_registry(config: &AppConfig, paths: &SaiPaths) -> ToolRegistry {
    // 1. 按全开配置构建本地注册表，跳过 MCP 网络与子进程发现。
    //    用实际配置枚举会让关闭插件的工具从配置界面上消失，于是无法为
    //    某个 Agent 预先勾选它——而 Agent 白名单本就该独立于全局开关
    let catalog = catalog_config(config);
    let mut registry = builtin_registry_without_mcp(&catalog, paths);
    // 2. 挂上只在会话中注册的工具，保证目录与会话看到同一份工具集合
    register_interactive_tools(
        &mut registry,
        &catalog,
        paths,
        paths.state_dir.display().to_string(),
        CATALOG_SESSION_ID.to_string(),
    );
    // 3. 定时任务只在网关提交路径注册，目录里同样要能勾选
    crate::cron::register_tool(&mut registry, paths.clone(), String::new());
    registry
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

/// 构造一份插件全开的配置副本，仅用于枚举工具目录。
///
/// 逐字段赋值容易在新增插件时漏掉，开关表由 PluginsConfig 自己维护。
///
/// 参数:
/// - `config`: 当前应用配置
///
/// 返回:
/// - 所有插件开关置真的配置副本
pub(crate) fn catalog_config(config: &AppConfig) -> AppConfig {
    let mut catalog = config.clone();
    catalog.plugins = catalog.plugins.all_enabled();
    catalog.memory.enabled = true;
    catalog
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
    // 常驻与否由基础集合决定，与用途分组无关：网格工具自成一组却仍是常驻的，
    // 前端若拿分组推断就会把它显示成"按需"，和实际行为对不上。
    let resident = groups::is_base_tool(&name);
    let meta = groups::group_meta(group);
    ToolCatalogEntry {
        name,
        group,
        resident,
        group_label: meta.label_zh,
        group_label_en: meta.label_en,
        group_hint: meta.hint_zh,
        group_hint_en: meta.hint_en,
        group_settings_path: meta.settings_path,
        group_rank: groups::group_rank(group),
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
    use std::collections::BTreeSet;
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

    /// 构造一份关闭全部插件的配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 插件全关的配置
    fn all_plugins_off() -> AppConfig {
        let mut config = AppConfig::default();
        config.plugins.archlinux.enabled = false;
        config.plugins.man.enabled = false;
        config.plugins.memes.enabled = false;
        config.plugins.web.enabled = false;
        config.plugins.knowledge_base.enabled = false;
        config.plugins.memory.enabled = false;
        config.memory.enabled = false;
        config
    }

    /// 验证插件关闭时工具仍出现在目录里。
    ///
    /// 目录是给 Agent 勾选工具用的，按当前全局开关过滤会让关掉的插件
    /// 在配置界面上彻底消失，于是没法为某个 Agent 单独启用它。
    #[test]
    fn disabled_plugins_still_appear_in_the_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(dir.path());

        let names: Vec<String> = tool_catalog(&all_plugins_off(), &paths)
            .into_iter()
            .map(|entry| entry.name)
            .collect();

        for expected in ["write_memory", "read_memory", "search_knowledge_base"] {
            assert!(names.iter().any(|name| name == expected), "缺少 {expected}");
        }
    }

    /// 验证委派类工具也在目录中。
    ///
    /// 这几个工具不经注册表注册，漏掉就无法在 Agent 里勾选。
    #[test]
    fn delegation_tools_are_listed() {
        let dir = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(dir.path());

        let names: Vec<String> = tool_catalog(&AppConfig::default(), &paths)
            .into_iter()
            .map(|entry| entry.name)
            .collect();

        for expected in ["subagent", "todo", "ask_question"] {
            assert!(names.iter().any(|name| name == expected), "缺少 {expected}");
        }
    }

    /// SSH 工具组必须出现在 Agent 设置目录里，否则界面上看不到这一组。
    #[test]
    fn ssh_tools_are_listed_as_their_own_group() {
        let dir = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(dir.path());
        let entries = tool_catalog(&AppConfig::default(), &paths);
        let ssh: Vec<&ToolCatalogEntry> = entries
            .iter()
            .filter(|entry| entry.group == "ssh")
            .collect();

        assert_eq!(ssh.len(), 4, "SSH 组应有四个工具");
        for expected in [
            "ssh_list_hosts",
            "ssh_run_command",
            "ssh_upload_file",
            "ssh_download_file",
        ] {
            assert!(
                ssh.iter().any(|entry| entry.name == expected),
                "缺少 {expected}"
            );
        }
        assert_eq!(ssh[0].group_label, "SSH 远程");
        assert_eq!(ssh[0].group_label_en, "SSH");
        assert_eq!(ssh[0].group_rank, 1);
        assert_eq!(ssh[0].group_settings_path, Some("/settings/ssh"));
        assert!(
            ssh[0].group_hint.contains("设置 → SSH"),
            "SSH 组应说明主机由用户在设置页配置"
        );
    }

    #[test]
    fn zz_audit_probe() {
        let dir = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(dir.path());
        let config = AppConfig::default();
        let mut registry =
            crate::tools::builtin_registry_without_mcp(&catalog_config(&config), &paths);
        crate::tools::register_interactive_tools(
            &mut registry,
            &config,
            &paths,
            dir.path().to_string_lossy().to_string(),
            "audit-session".to_string(),
        );
        let registered: std::collections::BTreeSet<String> =
            registry.tool_infos().into_iter().map(|i| i.name).collect();
        let entries = tool_catalog(&config, &paths);
        let catalog: std::collections::BTreeSet<String> =
            entries.iter().map(|e| e.name.clone()).collect();
        let missing: Vec<&String> = registered.difference(&catalog).collect();
        println!(
            "AUDIT registered={} catalog={} missing={}",
            registered.len(),
            catalog.len(),
            missing.len()
        );
        let mut by_group: std::collections::BTreeMap<&str, Vec<String>> =
            std::collections::BTreeMap::new();
        for entry in &entries {
            by_group.entry(entry.group).or_default().push(format!(
                "{}{}",
                entry.name,
                if entry.resident { "*" } else { "" }
            ));
        }
        for (group, names) in &by_group {
            println!(
                "AUDIT_GROUP [{}] n={} {}",
                group,
                names.len(),
                names.join(", ")
            );
        }
        println!("AUDIT_LEGEND name* means resident");
    }

    /// 验证目录覆盖交互式会话注册的全部工具。
    ///
    /// Agent 设置页的工具清单来自目录。目录一旦漏掉某个只在会话中注册的工具，
    /// 用户就既看不到也无法为它配置加载方式——网格工具正是这样"注册成功却
    /// 不存在"的。这里直接比对两条路径，任何新增的会话级工具都会被这条断言
    /// 挡下。
    #[test]
    fn the_catalog_covers_every_interactive_tool() {
        let dir = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(dir.path());
        let config = AppConfig::default();
        let mut registry =
            crate::tools::builtin_registry_without_mcp(&catalog_config(&config), &paths);
        crate::tools::register_interactive_tools(
            &mut registry,
            &config,
            &paths,
            dir.path().to_string_lossy().to_string(),
            "catalog-coverage-session".to_string(),
        );
        let catalog: BTreeSet<String> = tool_catalog(&config, &paths)
            .into_iter()
            .map(|entry| entry.name)
            .collect();

        let missing = registry
            .tool_infos()
            .into_iter()
            .map(|info| info.name)
            .filter(|name| !catalog.contains(name))
            .collect::<Vec<_>>();

        assert!(
            missing.is_empty(),
            "以下工具注册了却不在配置目录里，agent 配置页看不到它们: {}",
            missing.join(", ")
        );
    }

    /// 验证网格工具出现在配置目录里，并归入 mesh 分组。
    ///
    /// 用户要在配置界面上为它们选择"常驻 / 按需 / 关闭"，前提是先看得到。
    #[test]
    fn mesh_tools_are_listed_in_the_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(dir.path());
        let entries = tool_catalog(&AppConfig::default(), &paths);

        for expected in ["session_probe", "agent_probe", "mesh_send"] {
            let entry = entries
                .iter()
                .find(|entry| entry.name == expected)
                .unwrap_or_else(|| panic!("目录缺少 {expected}"));
            assert_eq!(entry.group, "mesh", "{expected} 应归入 mesh 分组");
            assert!(!entry.description.is_empty(), "{expected} 应有摘要说明");
        }
    }

    /// 验证只在网关提交路径注册的定时任务工具也进入目录。
    #[test]
    fn the_gateway_only_cron_tool_is_listed_in_the_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(dir.path());

        let names: Vec<String> = tool_catalog(&AppConfig::default(), &paths)
            .into_iter()
            .map(|entry| entry.name)
            .collect();

        assert!(names.iter().any(|name| name == "cron"), "目录缺少 cron");
        assert!(
            names.iter().any(|name| name == "send_channel_message"),
            "目录缺少 send_channel_message"
        );
    }

    /// 验证目录不含重复项。
    #[test]
    fn the_catalog_has_no_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(dir.path());
        let names: Vec<String> = tool_catalog(&AppConfig::default(), &paths)
            .into_iter()
            .map(|entry| entry.name)
            .collect();

        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), names.len());
    }
}

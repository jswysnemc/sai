use super::{mcp_tool_catalog, tool_catalog};
use crate::config::{AppConfig, McpServerConfig};
use crate::paths::SaiPaths;
use std::path::Path;

/// 【工具目录测试】【发现样本】创建一启动就写入标记并退出的 MCP 样本，无需等待定时器。
/// @param root 当前测试的隔离目录
/// @returns 使用该目录且不依赖用户配置的 stdio 服务
fn marker_server(root: &Path) -> McpServerConfig {
    let (command, arguments) = if cfg!(windows) {
        ("cmd", vec!["/D", "/C", "echo started>mcp-started"])
    } else {
        ("sh", vec!["-c", "printf started > mcp-started"])
    };
    McpServerConfig {
        id: "catalog-isolation".to_string(),
        enabled: true,
        transport: "stdio".to_string(),
        command: command.to_string(),
        args: arguments.into_iter().map(str::to_string).collect(),
        env: Default::default(),
        cwd: Some(root.display().to_string()),
        url: None,
        message_url: None,
        headers: Default::default(),
        timeout_ms: Some(5000),
    }
}

/// 【工具目录测试】【MCP 隔离】目录枚举不能启动外部服务，显式发现必须能够触发同一标记。
/// @returns 无；根据真实启动行为断言，不把机器负载或本地插件加载耗时当作服务发现
#[test]
fn catalog_does_not_discover_mcp_servers() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let marker = root.path().join("mcp-started");
    let mut config = AppConfig::default();
    config.mcp.enabled = true;
    config.mcp.servers = vec![marker_server(root.path())];

    // 【工具目录测试】【隔离断言】1. 通过实际目录入口检查外部服务未启动
    let entries = tool_catalog(&config, &paths);
    assert!(!marker.exists(), "本地工具目录启动了 MCP 子进程");
    assert!(entries.iter().any(|entry| entry.name == "mcp_manager"));
    assert!(!entries
        .iter()
        .any(|entry| entry.name.starts_with("mcp_catalog_isolation_")));

    // 【工具目录测试】【样本验证】2. 显式发现启动同一配置，防止无效命令导致隔离断言假通过
    mcp_tool_catalog(&config, &paths);
    assert_eq!(
        std::fs::read_to_string(marker)
            .expect("显式 MCP 发现没有执行启动标记")
            .trim(),
        "started"
    );
}

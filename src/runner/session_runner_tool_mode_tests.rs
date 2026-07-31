use super::*;
use std::path::PathBuf;

/// 创建工具模式测试使用的路径集合。
///
/// 参数:
/// - `state_dir`: 测试状态目录
///
/// 返回:
/// - 不访问用户目录的路径集合
fn test_paths(state_dir: PathBuf) -> SaiPaths {
    SaiPaths {
        config_dir: PathBuf::new(),
        config_file: PathBuf::new(),
        secrets_file: PathBuf::new(),
        skills_dir: PathBuf::new(),
        data_dir: PathBuf::new(),
        cache_dir: PathBuf::new(),
        state_dir,
        pictures_dir: PathBuf::new(),
        fish_hook_file: PathBuf::new(),
        bash_hook_file: PathBuf::new(),
        zsh_hook_file: PathBuf::new(),
        powershell_hook_file: PathBuf::new(),
    }
}

/// 注册无需外部依赖的测试工具。
///
/// 参数:
/// - `registry`: 待补充的工具注册表
/// - `name`: 工具名称
///
/// 返回:
/// - 无
fn register_test_tool(registry: &mut ToolRegistry, name: &str) {
    registry.register(crate::tools::ToolSpec::new(
        name,
        "test tool",
        serde_json::json!({ "type": "object" }),
        |_| async { Ok("ok".to_string()) },
    ));
}

/// 验证空启用白名单表示保留全部工具，而不是仅保留入口必需工具。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn empty_enabled_tools_keeps_registry_across_cli_surfaces() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    let mut config = AppConfig::default();
    config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
        enabled_tools: Vec::new(),
        deferred_tools: vec![crate::config::DEFERRED_ALL_NON_BASE.to_string()],
        skills_full: Vec::new(),
        skills_named: Vec::new(),
    });
    let mut registry = ToolRegistry::new();
    register_test_tool(&mut registry, "read_file");
    register_test_tool(&mut registry, "ask_question");
    let runner = SessionRunner::new(&paths).with_tool_registry(registry);

    for source in [
        SubmissionSource::Command,
        SubmissionSource::ShellIntercept,
        SubmissionSource::Repl,
        SubmissionSource::Web,
    ] {
        let selected = runner
            .load_tool_registry(
                &config,
                source,
                AgentMode::Yolo,
                "tool-mode-session",
                temp.path(),
            )
            .unwrap();

        assert!(selected.contains("read_file"), "source: {source:?}");
        assert!(selected.contains("ask_question"), "source: {source:?}");
    }
}

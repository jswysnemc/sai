use super::*;
use std::path::PathBuf;

/// 创建网关 Runner 测试路径。
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

#[test]
/// 验证渠道发送工具不会被 Agent 工具白名单移除。
fn gateway_channel_tool_survives_agent_runtime_whitelist() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    let mut config = AppConfig::default();
    config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
        enabled_tools: vec!["read_file".to_string()],
        skills_full: Vec::new(),
        skills_named: Vec::new(),
    });
    let mut registry = ToolRegistry::new();
    registry.register(crate::tools::ToolSpec::new(
        "send_channel_message",
        "test channel tool",
        serde_json::json!({ "type": "object" }),
        |_| async { Ok("ok".to_string()) },
    ));
    let runner = SessionRunner::new(&paths).with_tool_registry(registry);

    let selected = runner
        .load_tool_registry(
            &config,
            SubmissionSource::Gateway,
            AgentMode::Yolo,
            "gateway",
            std::path::Path::new("."),
        )
        .unwrap();

    assert!(selected.contains("cron"));
    assert!(selected.contains("send_channel_message"));
}

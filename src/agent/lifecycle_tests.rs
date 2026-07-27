use super::*;
use crate::tools::ToolSpec;
use serde_json::json;

/// 会话已保存的工具必须在 Agent 初始化时同步到提示词与执行门禁。
#[test]
fn new_agent_restores_persisted_loaded_tools() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let mut config = AppConfig::default();
    config.tools.progressive_loading_enabled = true;
    let state = StateStore::new(&paths).unwrap();
    state.init_files().unwrap();
    state
        .save_loaded_tools(&["get_weather".to_string()])
        .unwrap();
    assert_eq!(
        state.load_loaded_tools().unwrap(),
        vec!["get_weather".to_string()]
    );
    let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
    let mut registry = ToolRegistry::new();
    register_weather(&mut registry);
    assert!(registry.contains("get_weather"));

    let agent = Agent::new(config, &paths, state, client, registry, AgentMode::Yolo).unwrap();
    let prompt = agent
        .tool_visibility
        .loaded_context_prompt(&agent.tools)
        .unwrap_or_default();

    assert!(prompt.contains("Loaded tools: get_weather"));
    assert!(agent.tool_visibility.is_visible("get_weather"));
}

/// 模式切换后必须恢复新注册表中仍然存在的会话已加载工具。
#[test]
fn switch_mode_preserves_persisted_loaded_tools() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let mut config = AppConfig::default();
    config.tools.progressive_loading_enabled = true;
    let state = StateStore::new(&paths).unwrap();
    state.init_files().unwrap();
    state
        .save_loaded_tools(&["get_weather".to_string()])
        .unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
    let mut initial_registry = ToolRegistry::new();
    register_weather(&mut initial_registry);
    let mut agent = Agent::new(
        config,
        &paths,
        state,
        client,
        initial_registry,
        AgentMode::Yolo,
    )
    .unwrap();
    agent.restore_loaded_tools(&["get_weather".to_string()]);
    let mut replacement_registry = ToolRegistry::new();
    register_weather(&mut replacement_registry);

    agent
        .switch_mode(AgentMode::Audited, replacement_registry)
        .unwrap();

    assert!(agent.tool_visibility.is_visible("get_weather"));
}

/// 向测试注册表加入天气工具。
///
/// 参数:
/// - `registry`: 待更新的测试工具注册表
///
/// 返回:
/// - 无
fn register_weather(registry: &mut ToolRegistry) {
    registry.register(ToolSpec::new(
        "get_weather",
        "Test weather tool.",
        json!({"type":"object","properties":{}}),
        |_| async { Ok("sunny".to_string()) },
    ));
}

/// 创建隔离的应用路径集合。
///
/// 参数:
/// - `root`: 临时目录根路径
///
/// 返回:
/// - 测试用应用路径
fn test_paths(root: &std::path::Path) -> SaiPaths {
    SaiPaths {
        config_dir: root.join("config"),
        config_file: root.join("config/config.jsonc"),
        secrets_file: root.join("config/secrets.jsonc"),
        skills_dir: root.join("config/skills"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        state_dir: root.join("state"),
        pictures_dir: root.join("pictures"),
        fish_hook_file: root.join("fish/sai.fish"),
        bash_hook_file: root.join("shell/bash-hook.sh"),
        zsh_hook_file: root.join("shell/zsh-hook.zsh"),
        powershell_hook_file: root.join("shell/powershell-hook.ps1"),
    }
}

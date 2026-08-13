use super::*;
use crate::tools::ToolSpec;
use serde_json::json;

/// 会话已保存的工具必须在 Agent 初始化时同步到执行门禁，但不能生成动态系统提示。
#[test]
fn new_agent_restores_persisted_loaded_tools() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let mut config = AppConfig::default();
    config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
        enabled_tools: Vec::new(),
        deferred_tools: vec![crate::config::DEFERRED_ALL_NON_BASE.to_string()],
        skills_full: Vec::new(),
        skills_named: Vec::new(),
    });
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
    assert!(agent.tool_visibility.is_visible("get_weather"));
    assert!(!agent
        .chat_base_context_projection(None)
        .unwrap()
        .dynamic_sources
        .iter()
        .any(|source| source.key == "loaded_tools"));
}

/// 模式切换后必须恢复新注册表中仍然存在的会话已加载工具。
#[test]
fn switch_mode_preserves_persisted_loaded_tools() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let mut config = AppConfig::default();
    config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
        enabled_tools: Vec::new(),
        deferred_tools: vec![crate::config::DEFERRED_ALL_NON_BASE.to_string()],
        skills_full: Vec::new(),
        skills_named: Vec::new(),
    });
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

/// 模式切换保持稳定 system prompt，并把模式说明放入待发送的 user 上下文。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn switch_mode_updates_context_epoch_without_system_mode_context() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let config = AppConfig::default();
    let state = StateStore::new(&paths).unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
    let mut agent = Agent::new(
        config,
        &paths,
        state,
        client,
        ToolRegistry::new(),
        AgentMode::Yolo,
    )
    .unwrap();

    let yolo = agent.chat_base_context_projection(None).unwrap();
    agent
        .switch_mode(AgentMode::Audited, ToolRegistry::new())
        .unwrap();
    let audited = agent.chat_base_context_projection(None).unwrap();

    let yolo_system = message_text(&yolo.messages[0]);
    let audited_system = message_text(&audited.messages[0]);
    assert!(!yolo_system.contains("name=\"yolo\""));
    assert!(!audited_system.contains("name=\"audited\""));
    assert!(yolo
        .user_contexts
        .iter()
        .any(|context| context.contains("name=\"yolo\"")));
    assert!(audited
        .user_contexts
        .iter()
        .any(|context| context.contains("name=\"audited\"")));
    assert!(!audited
        .dynamic_sources
        .iter()
        .any(|source| source.key == "mode_reminder"));
}

/// 验证即时模式先变化时仍能识别工具注册表尚未切换。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn live_mode_change_does_not_mask_installed_tool_mode() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let config = AppConfig::default();
    let state = StateStore::new(&paths).unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
    let agent = Agent::new(
        config,
        &paths,
        state,
        client,
        ToolRegistry::new(),
        AgentMode::Yolo,
    )
    .unwrap();

    agent.apply_live_mode(AgentMode::Plan);

    assert_eq!(agent.mode(), AgentMode::Plan);
    assert_eq!(agent.installed_mode(), AgentMode::Yolo);
}

/// 提取测试消息文本。
///
/// 参数:
/// - `message`: 待读取消息
///
/// 返回:
/// - 文本内容
fn message_text(message: &ChatMessage) -> String {
    match message.content.as_ref() {
        Some(crate::llm::ChatContent::Text(text)) => text.clone(),
        _ => String::new(),
    }
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

/// skill 正文一律靠 load 读取，重载后必须保留加载器，否则切换模型会让 skill 失效。
#[test]
fn reload_keeps_loader_for_visible_skills() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let skill_dir = paths.skills_dir.join("named-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: named-skill\ndescription: demo\n---\n\nrun it.\n",
    )
    .unwrap();
    let mut config = AppConfig::default();
    // 没有延迟工具，只有仅暴露名称的 skill
    config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
        enabled_tools: Vec::new(),
        deferred_tools: Vec::new(),
        skills_full: Vec::new(),
        skills_named: vec!["named-skill".to_string()],
    });
    let state = StateStore::new(&paths).unwrap();
    state.init_files().unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
    let mut agent = Agent::new(
        config.clone(),
        &paths,
        state,
        client.clone(),
        ToolRegistry::new(),
        AgentMode::Yolo,
    )
    .unwrap();
    assert!(agent.tools.contains(crate::tools::LOAD_NAME));

    agent
        .reload(config, client, ToolRegistry::new(), AgentMode::Yolo)
        .unwrap();

    assert!(
        agent.tools.contains(crate::tools::LOAD_NAME),
        "重载后仍须保留 load，否则 skill 正文无法读取"
    );
}

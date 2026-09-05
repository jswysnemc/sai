use crate::config::AppConfig;

/// 【CLI】【模型选择】验证为内置档案固定模型会物化完整档案。
///
/// 只写 id 与模型的空档案会让探索 Agent 退化为全量工具，
/// 因此物化必须保留工具白名单等内置能力。
#[test]
fn setting_a_builtin_agent_model_preserves_its_capabilities() {
    let mut config = AppConfig::default();
    let provider_id = config.providers[0].id.clone();

    assert!(config.set_agent_model("explore", &provider_id, "model-a"));

    let stored = config
        .agents
        .iter()
        .find(|profile| profile.id == "explore")
        .expect("内置档案应物化到配置");
    assert_eq!(stored.provider_id, provider_id);
    assert_eq!(stored.model, "model-a");
    assert!(!stored.enabled_tools.is_empty(), "探索工具白名单必须保留");
    let resolved = config
        .resolved_agent_profiles()
        .into_iter()
        .find(|profile| profile.id == "explore")
        .unwrap();
    assert_eq!(resolved.model, "model-a");
}

/// 【CLI】【模型选择】验证再次设置覆盖会原位更新而不重复入档。
#[test]
fn setting_an_agent_model_again_updates_in_place() {
    let mut config = AppConfig::default();
    let provider_id = config.providers[0].id.clone();
    config.set_agent_model("explore", &provider_id, "model-a");

    assert!(config.set_agent_model("explore", &provider_id, "model-b"));
    let matches = config
        .agents
        .iter()
        .filter(|profile| profile.id == "explore")
        .count();
    assert_eq!(matches, 1, "同档案不应重复物化");
    let stored = config
        .agents
        .iter()
        .find(|profile| profile.id == "explore")
        .unwrap();
    assert_eq!(stored.model, "model-b");

    assert!(
        !config.set_agent_model("explore", &provider_id, "model-b"),
        "无变化不应报告改动"
    );
}

/// 【CLI】【模型选择】验证继承空选项会清除既有覆盖。
#[test]
fn the_inherit_choice_clears_an_existing_override() {
    let mut config = AppConfig::default();
    let provider_id = config.providers[0].id.clone();
    config.set_agent_model("explore", &provider_id, "model-a");

    assert!(config.set_agent_model("explore", "", ""));
    let stored = config
        .agents
        .iter()
        .find(|profile| profile.id == "explore")
        .unwrap();
    assert!(stored.provider_id.is_empty());
    assert!(stored.model.is_empty());
}

/// 【CLI】【模型选择】验证对未配置档案选择继承是无操作。
#[test]
fn the_inherit_choice_is_a_noop_for_untouched_builtins() {
    let mut config = AppConfig::default();

    assert!(!config.set_agent_model("explore", "", ""));
    assert!(
        !config.agents.iter().any(|profile| profile.id == "explore"),
        "不应为无覆盖可清的内置档案物化配置"
    );
}

/// 【CLI】【模型选择】验证旧版迁移档案的覆盖也能被继承清除。
#[test]
fn the_inherit_choice_clears_a_legacy_override() {
    let mut config = AppConfig::default();
    config.subagent.profiles = vec![crate::config::SubagentProfile {
        id: "explore".to_string(),
        name: "旧探索".to_string(),
        description: String::new(),
        system_prompt: String::new(),
        provider_id: "legacy-provider".to_string(),
        model: "legacy-model".to_string(),
        thinking_level: "auto".to_string(),
        exposed: true,
    }];

    assert!(config.set_agent_model("explore", "", ""));
    let resolved = config
        .resolved_agent_profiles()
        .into_iter()
        .find(|profile| profile.id == "explore")
        .unwrap();
    assert!(resolved.provider_id.is_empty(), "旧覆盖应被统一档案盖掉");
    assert!(resolved.model.is_empty());
}

/// 【CLI】【模型选择】验证未知档案返回无改动。
#[test]
fn unknown_agents_report_no_change() {
    let mut config = AppConfig::default();

    assert!(!config.set_agent_model("missing", "openai", "gpt-5"));
    assert!(config.agents.is_empty());
}

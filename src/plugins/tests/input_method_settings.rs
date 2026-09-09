use super::support::FixtureHost;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::discovery::find;
use crate::plugins::registry::register_descriptor;
use crate::plugins::{configure, set_enabled, GrantUpdate};
use crate::tools::ToolRegistry;
use serde_json::json;
use std::sync::Arc;

const PLUGIN: &str = "input-method-investigation";
const TOOL: &str = "linux_input_method_diagnose";

/// 【输入法配置测试】【旧设置】只传递旧调查实际使用的字段，超时保留五秒下限并受宿主总时长限制。
#[test]
fn legacy_input_method_settings_are_derived_without_provider_configuration() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.deep_diagnose.max_tool_steps = 17;
    config.plugins.deep_diagnose.tool_call_timeout_seconds = 1;
    config.display.tool_calls = " FULL ".into();
    let plugin = find(&config, &paths, PLUGIN).unwrap();
    let settings = plugin.settings();
    assert_eq!(settings["max_tool_steps"], 17);
    assert_eq!(settings["tool_timeout_ms"], 5000);
    assert_eq!(settings["progress_mode"], "full");
    assert_eq!(settings.as_object().unwrap().len(), 4);
    assert_eq!(plugin.setting.settings, json!({}));

    config.plugins.deep_diagnose.tool_call_timeout_seconds = u64::MAX;
    assert_eq!(
        find(&config, &paths, PLUGIN).unwrap().settings()["tool_timeout_ms"],
        900_000
    );
}

/// 【输入法配置测试】【显式覆盖】独立设置优先，保存操作不写回派生配置或凭据。
#[test]
fn explicit_input_method_settings_override_legacy_defaults_without_rewriting_them() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let explicit =
        json!({"max_tool_steps":2,"tool_timeout_ms":125,"progress_mode":"hidden","language":"en"});
    configure(&config, &paths, PLUGIN, explicit.clone()).unwrap();
    let plugin = find(&config, &paths, PLUGIN).unwrap();
    assert_eq!(*plugin.settings(), explicit);
    assert_eq!(plugin.setting.settings, explicit);
    let saved = std::fs::read_to_string(paths.config_dir.join("plugins.jsonc")).unwrap();
    assert!(!saved.contains("active_provider"));
    assert!(!saved.contains("api_key"));
}

/// 【输入法配置测试】【加载校验】非法预算、时长和显示模式在注册时失败，不留下半注册工具。
#[test]
fn invalid_input_method_settings_fail_before_registration() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    for settings in [
        json!({"max_tool_steps":-1}),
        json!({"max_tool_steps":1.5}),
        json!({"max_tool_steps":false}),
        json!({"tool_timeout_ms":0}),
        json!({"tool_timeout_ms":"90"}),
        json!({"tool_timeout_ms":false}),
        json!({"progress_mode":"verbose"}),
        json!({"progress_mode":false}),
    ] {
        let mut plugin = find(&config, &paths, PLUGIN).unwrap();
        plugin.setting.settings = settings;
        plugin.refresh_compatibility(&config).unwrap();
        let mut tools = ToolRegistry::new();
        assert!(
            register_descriptor(&mut tools, plugin, Arc::new(FixtureHost::default()), false)
                .is_err()
        );
        assert!(!tools.contains(TOOL));
    }
}

/// 【输入法配置测试】【独立开关】新包沿用旧默认开关，显式设置分别控制调查和资料查询。
#[test]
fn input_method_plugin_overrides_the_legacy_switch_and_keeps_evidence_tools_independent() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.deep_diagnose.enabled = false;
    let initial = crate::tools::builtin_registry_without_mcp(&config, &paths);
    assert!(!initial.contains(TOOL));
    assert!(initial.contains("check_issue"));
    assert!(initial.contains("fcitx5_input_method_wiki_qurey"));
    for (enabled, legacy) in [(true, false), (false, true)] {
        config.plugins.deep_diagnose.enabled = legacy;
        set_enabled(&config, &paths, PLUGIN, enabled, GrantUpdate::Keep).unwrap();
        for tools in [
            crate::tools::builtin_registry_without_mcp(&config, &paths),
            crate::tools::readonly_registry(&config, &paths),
        ] {
            assert_eq!(tools.contains(TOOL), enabled);
            assert!(tools.contains("check_issue"));
            assert!(tools.contains("fcitx5_input_method_wiki_qurey"));
            assert!(tools.plugin_diagnostics().is_empty());
        }
    }
    assert!(crate::tools::tool_catalog(&config, &paths)
        .iter()
        .any(|tool| tool.name == TOOL));
}

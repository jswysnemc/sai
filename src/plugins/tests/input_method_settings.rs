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
const TOOL: &str = "lua__input-method-investigation__linux_input_method_diagnose";

/// 【输入法配置测试】【独立默认】宿主显示偏好不写入包设置，安装保持禁用且无授权
/// @returns 无；实际默认值由 Lua 模块维护
#[test]
fn input_method_settings_do_not_inherit_host_display_preferences() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.display.tool_calls = "full".into();
    super::example_support::install(PLUGIN, &paths);
    let plugin = find(&config, &paths, PLUGIN).unwrap();
    assert_eq!(plugin.settings(), &json!({}));
    assert!(!plugin.setting.enabled);
    assert_eq!(plugin.grants(), Default::default());
}

/// 【输入法配置测试】【显式设置】独立设置原样保存，不写入供应商选择和凭据
/// @returns 无；发现后使用相同设置快照
#[test]
fn explicit_input_method_settings_are_saved_without_provider_configuration() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install(PLUGIN, &paths);
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

/// 【输入法配置测试】【加载校验】非法预算、时长和显示模式在注册前失败
/// @returns 无；失败不留下半注册工具
#[test]
fn invalid_input_method_settings_fail_before_registration() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install(PLUGIN, &paths);
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
        let mut tools = ToolRegistry::new();
        assert!(
            register_descriptor(&mut tools, plugin, Arc::new(FixtureHost::default()), false)
                .is_err()
        );
        assert!(!tools.contains(TOOL));
    }
}

/// 【输入法配置测试】【独立启停】调查、诊断与文档资料分别安装和控制
/// @returns 无；禁用调查不禁用其可选资料来源
#[test]
fn input_method_activation_keeps_evidence_tools_independent() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install(PLUGIN, &paths);
    for id in ["fcitx-wiki", "diagnostic-evidence"] {
        super::example_support::install_enabled(id, &config, &paths);
    }
    assert!(!crate::tools::builtin_registry_without_mcp(&config, &paths).contains(TOOL));
    for enabled in [true, false] {
        set_enabled(&config, &paths, PLUGIN, enabled, GrantUpdate::Keep).unwrap();
        for tools in [
            crate::tools::builtin_registry_without_mcp(&config, &paths),
            crate::tools::readonly_registry(&config, &paths),
        ] {
            assert_eq!(tools.contains(TOOL), enabled);
            assert!(tools.contains("lua__diagnostic-evidence__check_issue"));
            assert!(tools.contains("lua__fcitx-wiki__fcitx5_input_method_wiki_qurey"));
            assert!(tools.plugin_diagnostics().is_empty());
        }
    }
    assert!(!crate::tools::tool_catalog(&config, &paths)
        .iter()
        .any(|tool| tool.name == TOOL));
}

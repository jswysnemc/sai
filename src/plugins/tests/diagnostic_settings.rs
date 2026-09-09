use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::discovery::find;
use crate::plugins::{configure, set_enabled, GrantUpdate};
use crate::tools::ToolPermission;
use serde_json::json;

const PLUGIN: &str = "diagnostic-evidence";

/// 【诊断配置测试】【兼容优先级】旧值只提供缺省配置，显式配置不改写主配置且不能绕过宿主硬上限。
#[test]
fn explicit_diagnostic_settings_override_bounded_legacy_defaults() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.diagnostics.command_timeout_seconds = 0;
    config.plugins.diagnostics.max_stdout_chars = usize::MAX;
    let found = find(&config, &paths, PLUGIN).unwrap();
    assert_eq!(found.settings()["command_timeout_ms"], 1000);
    assert_eq!(found.settings()["max_stdout_chars"], 200_000);
    assert_eq!(found.setting.settings, json!({}));
    config.plugins.diagnostics.command_timeout_seconds = u64::MAX;
    assert_eq!(
        find(&config, &paths, PLUGIN).unwrap().settings()["command_timeout_ms"],
        120_000
    );
    let explicit = json!({"command_timeout_ms":125,"max_stdout_chars":321,"max_stderr_chars":123});
    configure(&config, &paths, PLUGIN, explicit.clone()).unwrap();
    let found = find(&config, &paths, PLUGIN).unwrap();
    assert_eq!(*found.settings(), explicit);
    assert_eq!(found.setting.settings, explicit);
    assert_eq!(config.plugins.diagnostics.command_timeout_seconds, u64::MAX);
}

/// 【诊断配置测试】【失败原子性】非法配置在保存前失败，不能以默认值代替显式错误。
#[test]
fn invalid_diagnostic_settings_do_not_modify_the_saved_configuration() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    configure(&config, &paths, PLUGIN, json!({"command_timeout_ms":125})).unwrap();
    let before = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
    for settings in [
        json!({"command_timeout_ms":false}),
        json!({"command_timeout_ms":0}),
        json!({"command_timeout_ms":120001}),
        json!({"max_stdout_chars":-1}),
        json!({"max_stdout_chars":1.5}),
        json!({"max_stderr_chars":"20"}),
        json!({"max_stderr_chars":false}),
    ] {
        assert!(configure(&config, &paths, PLUGIN, settings).is_err());
        assert_eq!(
            std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
            before
        );
    }
}

/// 【诊断配置测试】【注册边界】共用目录指向 Lua，只读目录只公开取证，显式开关覆盖旧配置。
#[test]
fn diagnostic_registration_preserves_switch_precedence_and_write_separation() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.diagnostics.enabled = false;
    assert!(!crate::tools::builtin_registry_without_mcp(&config, &paths).contains("check_issue"));
    for (enabled, legacy) in [(true, false), (false, true)] {
        config.plugins.diagnostics.enabled = legacy;
        set_enabled(&config, &paths, PLUGIN, enabled, GrantUpdate::Keep).unwrap();
        let common = crate::tools::builtin_registry_without_mcp(&config, &paths);
        let readonly = crate::tools::readonly_registry(&config, &paths);
        assert_eq!(common.contains("check_issue"), enabled);
        assert_eq!(readonly.contains("check_issue"), enabled);
        assert_eq!(common.contains("diagnostic_app_probe"), enabled);
        assert!(!readonly.contains("diagnostic_app_probe"));
        assert!(common.contains("linux_input_method_diagnose"));
        assert!(common.plugin_diagnostics().is_empty());
        assert!(readonly.plugin_diagnostics().is_empty());
        if enabled {
            assert_eq!(common.plugin_owner("check_issue"), Some(PLUGIN));
            assert!(matches!(
                common.permission("check_issue").unwrap(),
                ToolPermission::ReadOnly
            ));
            assert!(matches!(
                common.permission("diagnostic_app_probe").unwrap(),
                ToolPermission::Writes
            ));
        }
    }
    let catalog = crate::tools::tool_catalog(&config, &paths);
    assert!(catalog.iter().any(|tool| tool.name == "check_issue"));
    assert!(catalog
        .iter()
        .any(|tool| tool.name == "diagnostic_app_probe"));
}

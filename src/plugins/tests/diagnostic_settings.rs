use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::discovery::find;
use crate::plugins::{configure, set_enabled, GrantUpdate};
use crate::tools::ToolPermission;
use serde_json::json;

const PLUGIN: &str = "diagnostic-evidence";
const CHECK: &str = "lua__diagnostic-evidence__check_issue";
const PROBE: &str = "lua__diagnostic-evidence__diagnostic_app_probe";

/// 【诊断配置测试】【独立设置】安装不注入主配置，只保存显式时限和输出限制
/// @returns 无；未授权包没有系统能力
#[test]
fn explicit_diagnostic_settings_are_owned_by_the_installed_package() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install(PLUGIN, &paths);
    let found = find(&config, &paths, PLUGIN).unwrap();
    assert_eq!(found.settings(), &json!({}));
    assert_eq!(found.grants(), Default::default());
    let explicit = json!({"command_timeout_ms":125,"max_stdout_chars":321,"max_stderr_chars":123});
    configure(&config, &paths, PLUGIN, explicit.clone()).unwrap();
    let found = find(&config, &paths, PLUGIN).unwrap();
    assert_eq!(*found.settings(), explicit);
    assert_eq!(found.setting.settings, explicit);
}

/// 【诊断配置测试】【失败原子性】非法配置在保存前失败，不能以默认值代替显式错误
/// @returns 无；原配置逐字保留
#[test]
fn invalid_diagnostic_settings_do_not_modify_the_saved_configuration() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install(PLUGIN, &paths);
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

/// 【诊断配置测试】【注册边界】只读目录公开取证，应用探测仍要求写入模式
/// @returns 无；禁用后目录不再保留该外部包
#[test]
fn diagnostic_registration_preserves_activation_and_write_separation() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    assert!(!crate::tools::builtin_registry_without_mcp(&config, &paths).contains(CHECK));
    super::example_support::install(PLUGIN, &paths);
    for enabled in [true, false] {
        set_enabled(&config, &paths, PLUGIN, enabled, GrantUpdate::Keep).unwrap();
        let common = crate::tools::builtin_registry_without_mcp(&config, &paths);
        let readonly = crate::tools::readonly_registry(&config, &paths);
        assert_eq!(common.contains(CHECK), enabled);
        assert_eq!(readonly.contains(CHECK), enabled);
        assert_eq!(common.contains(PROBE), enabled);
        assert!(!readonly.contains(PROBE));
        assert!(common.plugin_diagnostics().is_empty());
        assert!(readonly.plugin_diagnostics().is_empty());
        if enabled {
            assert_eq!(common.plugin_owner(CHECK), Some(PLUGIN));
            assert_eq!(common.permission(CHECK).unwrap(), ToolPermission::ReadOnly);
            assert_eq!(common.permission(PROBE).unwrap(), ToolPermission::Writes);
        }
    }
    let catalog = crate::tools::tool_catalog(&config, &paths);
    assert!(!catalog
        .iter()
        .any(|tool| tool.name == CHECK || tool.name == PROBE));
}

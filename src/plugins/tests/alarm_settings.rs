use crate::{config::AppConfig, paths::SaiPaths};
use serde_json::json;

/// 【闹钟设置测试】【声明与授权】音频范围由清单声明，用户授权可以独立收窄。
/// @returns 无，禁用包不再注册旧工具名称
#[test]
fn alarm_settings_respect_explicit_grants_and_disable() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    super::example_support::install_custom("alarm", &paths, |manifest| {
        manifest
            .capabilities
            .system
            .read_paths
            .insert("~/Music".into());
    });
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    let file = paths.config_dir.join("plugins.jsonc");
    let content = serde_json::to_vec(&json!({"plugins":{"alarm":{
        "enabled":true,"settings":{},
        "grants":{"system":{"schedule":true,"notify":true,"read_paths":["."]}}
    }}}))
    .unwrap();
    std::fs::write(&file, &content).unwrap();
    let found = crate::plugins::discover(&AppConfig::default(), &paths);
    assert!(found.diagnostics.is_empty());
    let alarm = found
        .plugins
        .iter()
        .find(|plugin| plugin.package.manifest.id == "alarm")
        .unwrap();
    assert!(alarm.capabilities().system.read_paths.contains("~/Music"));
    let effective = alarm.capabilities().intersection(&alarm.grants());
    assert_eq!(effective.system.read_paths.len(), 1);
    assert!(effective.system.read_paths.contains("."));
    assert_eq!(std::fs::read(&file).unwrap(), content);
    std::fs::write(&file, r#"{"plugins":{"alarm":{"enabled":false}}}"#).unwrap();
    let registry = crate::tools::builtin_registry_without_mcp(&AppConfig::default(), &paths);
    for name in [
        "lua__alarm__set_alarm",
        "lua__alarm__list_alarms",
        "lua__alarm__cancel_alarm",
    ] {
        assert!(!registry.contains(name));
    }
}

/// 【闹钟设置测试】【非法范围】无效设置产生明确诊断，不能回退到默认读取权限。
/// @returns 无
#[test]
fn alarm_settings_reject_invalid_audio_scope() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    super::example_support::install("alarm", &paths);
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    for settings in [
        json!({"audio_paths":true}),
        json!({"audio_paths":vec![".";65]}),
        json!({"unknown":1}),
    ] {
        std::fs::write(
            paths.config_dir.join("plugins.jsonc"),
            serde_json::to_vec(&json!({"plugins":{"alarm":{"enabled":true,"settings":settings}}}))
                .unwrap(),
        )
        .unwrap();
        let registry = crate::tools::builtin_registry_without_mcp(&AppConfig::default(), &paths);
        assert!(!registry.contains("lua__alarm__set_alarm"));
        assert!(registry
            .plugin_diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.source == "alarm"));
    }
}

/// 【闹钟迁移测试】【注册归属】三个工具名称必须包含普通插件命名空间。
/// @returns 无，普通入口保留写入工具，只读入口只提供查询
#[test]
fn alarm_plugin_owns_existing_tool_names() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install_enabled("alarm", &config, &paths);
    let normal = crate::tools::builtin_registry_without_mcp(&config, &paths);
    assert!(normal.plugin_diagnostics().is_empty());
    for name in [
        "lua__alarm__set_alarm",
        "lua__alarm__list_alarms",
        "lua__alarm__cancel_alarm",
    ] {
        assert_eq!(normal.plugin_owner(name), Some("alarm"), "{name}");
    }
    let readonly = crate::tools::readonly_registry(&config, &paths);
    assert_eq!(
        readonly.plugin_owner("lua__alarm__list_alarms"),
        Some("alarm")
    );
    assert!(!readonly.contains("lua__alarm__set_alarm"));
    assert!(!readonly.contains("lua__alarm__cancel_alarm"));
}

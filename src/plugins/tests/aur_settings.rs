use crate::plugins::{self, GrantChanges, GrantUpdate};
use crate::{
    config::AppConfig,
    paths::SaiPaths,
    tools::{ToolPermission, ToolRegistry},
};

/// 【AUR 配置测试】【兼容开关】普通安装默认禁用，显式设置控制启用且只读目录排除安装工具。
#[test]
fn aur_plugin_settings_override_legacy_defaults_and_preserve_tool_access() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install("package-advisor", &paths);
    let mut registry = ToolRegistry::new();
    assert!(plugins::register_plugins(&mut registry, &config, &paths, false).is_empty());
    assert!(!registry.contains("lua__package-advisor__review_aur_package"));
    plugins::set_enabled(
        &config,
        &paths,
        "package-advisor",
        true,
        GrantUpdate::Declared,
    )
    .unwrap();
    for readonly in [false, true] {
        let mut registry = ToolRegistry::new();
        assert!(plugins::register_plugins(&mut registry, &config, &paths, readonly).is_empty());
        assert_eq!(
            registry.plugin_owner("lua__package-advisor__review_aur_package"),
            Some("package-advisor")
        );
        assert_eq!(
            registry
                .permission("lua__package-advisor__review_aur_package")
                .unwrap(),
            ToolPermission::ReadOnly
        );
        assert_eq!(
            registry.contains("lua__package-advisor__install_aur_package"),
            !readonly
        );
        if !readonly {
            assert_eq!(
                registry
                    .permission("lua__package-advisor__install_aur_package")
                    .unwrap(),
                ToolPermission::Writes
            );
        }
    }
    plugins::set_enabled(
        &config,
        &paths,
        "package-advisor",
        true,
        GrantUpdate::Changes(GrantChanges {
            workspace: Some(false),
            ..Default::default()
        }),
    )
    .unwrap();
    let found = plugins::discover(&config, &paths);
    let package = found
        .plugins
        .iter()
        .find(|p| p.package.manifest.id == "package-advisor")
        .unwrap();
    assert!(!package.grants().system.workspace);
    assert!(package.grants().system.session_storage);
    assert!(!package.grants().system.processes.is_empty());
}

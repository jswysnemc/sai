use super::support::{descriptor, write_package};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::config::{load_config, save_config};
use crate::plugins::{self, GrantUpdate};
use serde_json::json;

/// 【插件测试】【安装生命周期】新包不会自动启用，更新保留设置，卸载清理安装目录和授权。
#[test]
fn installed_packages_require_activation_and_survive_updates() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let source = root.path().join("source");
    plugins::scaffold(&source, "example").unwrap();
    let inspection = plugins::validate_package(&source).unwrap();
    assert_eq!(inspection.tools[0].name, "greet");
    assert_eq!(inspection.commands[0].name, "stats");
    let installed = plugins::install(&source, &paths, false).unwrap();
    assert!(!load_config(&paths).unwrap().plugins["example"].enabled);
    assert!(plugins::install(&source, &paths, false).is_err());
    plugins::configure(&config, &paths, "example", json!({"greeting":"你好"})).unwrap();
    plugins::set_enabled(&config, &paths, "example", true, GrantUpdate::Keep).unwrap();
    plugins::install(&source, &paths, true).unwrap();
    let setting = load_config(&paths).unwrap().plugins["example"].clone();
    assert!(setting.enabled);
    assert_eq!(setting.settings, json!({"greeting":"你好"}));
    plugins::remove(&config, &paths, "example").unwrap();
    assert!(!installed.exists());
    let setting = load_config(&paths).unwrap().plugins["example"].clone();
    assert!(!setting.enabled);
    assert!(setting.grants.unwrap().http.is_empty());
}

/// 【插件测试】【发现与优先级】显式插件设置覆盖旧开关，坏包不会隐藏其他有效插件。
#[test]
fn discovery_isolates_broken_packages_and_honors_explicit_overrides() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.man.enabled = false;
    let found = plugins::discover(&config, &paths);
    assert!(
        !found
            .plugins
            .iter()
            .find(|plugin| plugin.package.manifest.id == "online-man")
            .unwrap()
            .setting
            .enabled
    );
    plugins::set_enabled(&config, &paths, "online-man", true, GrantUpdate::Keep).unwrap();
    let installed = paths.config_dir.join("plugins/good");
    write_package(&installed, &descriptor("good", ""));
    std::fs::create_dir_all(paths.config_dir.join("plugins/broken")).unwrap();
    let found = plugins::discover(&config, &paths);
    assert_eq!(found.diagnostics.len(), 1);
    assert!(
        found
            .plugins
            .iter()
            .find(|plugin| plugin.package.manifest.id == "online-man")
            .unwrap()
            .setting
            .enabled
    );
    let good = found
        .plugins
        .iter()
        .find(|plugin| plugin.package.manifest.id == "good")
        .unwrap();
    assert!(!good.setting.enabled);
    assert!(good.grants().http.is_empty());
    std::fs::write(paths.config_dir.join("plugins.jsonc"), "broken JSON").unwrap();
    let failed = plugins::discover(&config, &paths);
    assert!(failed.plugins.is_empty());
    assert!(failed.diagnostics[0].error.contains("parse plugins.jsonc"));
}

/// 【插件测试】【显式授权】启用不隐式授予网络访问，清单之外的授权会被拒绝。
#[test]
fn network_grants_are_explicit_and_bounded_by_the_manifest() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    write_package(
        &paths.config_dir.join("plugins/network"),
        &descriptor("network", ""),
    );
    plugins::set_enabled(&config, &paths, "network", true, GrantUpdate::Keep).unwrap();
    assert!(load_config(&paths).unwrap().plugins["network"]
        .grants
        .is_none());
    assert!(plugins::set_enabled(
        &config,
        &paths,
        "network",
        true,
        GrantUpdate::Http(["https://other.test".into()].into())
    )
    .is_err());
    plugins::set_enabled(&config, &paths, "network", true, GrantUpdate::Declared).unwrap();
    assert_eq!(
        load_config(&paths).unwrap().plugins["network"]
            .grants
            .as_ref()
            .unwrap()
            .http,
        ["https://example.test".into()].into()
    );
    plugins::set_enabled(
        &config,
        &paths,
        "network",
        true,
        GrantUpdate::Http(Default::default()),
    )
    .unwrap();
    assert!(load_config(&paths).unwrap().plugins["network"]
        .grants
        .as_ref()
        .unwrap()
        .http
        .is_empty());
}

/// 【插件测试】【管理保护】本地安装不能覆盖内置插件，非法配置也不能覆盖已有文件。
#[test]
fn bundled_ids_and_existing_config_are_preserved_on_errors() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let source = root.path().join("source");
    write_package(&source, &descriptor("online-man", ""));
    assert!(plugins::install(&source, &paths, true).is_err());
    assert!(plugins::remove(&config, &paths, "online-man").is_err());
    plugins::set_enabled(&config, &paths, "online-man", false, GrantUpdate::Keep).unwrap();
    let before = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
    let mut bad = load_config(&paths).unwrap();
    bad.plugins.get_mut("online-man").unwrap().settings = json!([]);
    assert!(save_config(&paths, &bad).is_err());
    assert_eq!(
        std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
        before
    );
}

/// 【插件测试】【并发管理】其他进程持有管理锁时不读改写配置或移动目录。
#[test]
fn concurrent_management_is_rejected_before_configuration_changes() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let lock = crate::plugins::config::mutation_lock(&paths).unwrap();
    assert!(plugins::set_enabled(
        &AppConfig::default(),
        &paths,
        "online-man",
        false,
        GrantUpdate::Keep
    )
    .is_err());
    assert!(!paths.config_dir.join("plugins.jsonc").exists());
    drop(lock);
    plugins::set_enabled(
        &AppConfig::default(),
        &paths,
        "online-man",
        false,
        GrantUpdate::Keep,
    )
    .unwrap();
}

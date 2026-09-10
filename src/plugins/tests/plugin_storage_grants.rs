use super::support::{descriptor, write_package};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::config::load_config;
use crate::plugins::{self, GrantChanges, GrantUpdate};
use clap::Parser;
use serde_json::json;

/// 【插件存储测试】【分项授权】单独授予或撤销插件存储不改变会话及其他有效授权。
#[test]
fn plugin_storage_grants_preserve_other_capabilities_and_reject_undeclared_updates() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let mut package = descriptor("storage-grants", "");
    package.package.manifest.capabilities = serde_json::from_value(json!({
        "http":["https://example.test"],"model":true,
        "system":{"plugin_storage":true,"session_storage":true,"workspace":true,"environment":["LANG"]}
    })).unwrap();
    let path = paths.config_dir.join("plugins/storage-grants");
    write_package(&path, &package);
    plugins::set_enabled(
        &config,
        &paths,
        "storage-grants",
        true,
        GrantUpdate::Declared,
    )
    .unwrap();
    let saved = || {
        load_config(&paths).unwrap().plugins["storage-grants"]
            .grants
            .clone()
            .unwrap()
    };
    let baseline = saved();
    for enabled in [false, true] {
        plugins::set_enabled(
            &config,
            &paths,
            "storage-grants",
            true,
            GrantUpdate::Changes(GrantChanges {
                plugin_storage: Some(enabled),
                ..Default::default()
            }),
        )
        .unwrap();
        let mut expected = baseline.clone();
        expected.system.plugin_storage = enabled;
        assert_eq!(saved(), expected);
    }
    plugins::set_enabled(
        &config,
        &paths,
        "storage-grants",
        true,
        GrantUpdate::Changes(GrantChanges {
            session_storage: Some(false),
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(saved().system.plugin_storage);
    assert!(!saved().system.session_storage);
    package.package.manifest.capabilities.system.plugin_storage = false;
    write_package(&path, &package);
    let before = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
    assert!(plugins::set_enabled(
        &config,
        &paths,
        "storage-grants",
        true,
        GrantUpdate::Changes(GrantChanges {
            plugin_storage: Some(true),
            ..Default::default()
        })
    )
    .is_err());
    assert_eq!(
        std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
        before
    );
    plugins::set_enabled(
        &config,
        &paths,
        "storage-grants",
        true,
        GrantUpdate::Changes(GrantChanges {
            environment: Some(Default::default()),
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(!saved().system.plugin_storage);
    assert!(saved().system.workspace && saved().model);
}

/// 【插件存储测试】【命令行冲突】显式开关可与会话授权组合，但不能与相反开关或全量授权同时使用。
#[test]
fn plugin_storage_cli_flags_support_independent_updates_and_conflict_checks() {
    for flags in [
        vec!["--allow-plugin-storage"],
        vec!["--no-plugin-storage"],
        vec!["--allow-plugin-storage", "--no-session-storage"],
        vec!["--no-plugin-storage", "--allow-session-storage"],
    ] {
        let mut arguments = vec!["sai", "plugins", "enable", "sample"];
        arguments.extend(flags);
        assert!(crate::cli::Cli::try_parse_from(arguments).is_ok());
    }
    for flags in [
        vec!["--allow-plugin-storage", "--no-plugin-storage"],
        vec!["--grant-declared", "--allow-plugin-storage"],
        vec!["--grant-declared", "--no-plugin-storage"],
    ] {
        let mut arguments = vec!["sai", "plugins", "enable", "sample"];
        arguments.extend(flags);
        assert_eq!(
            crate::cli::Cli::try_parse_from(arguments)
                .unwrap_err()
                .kind(),
            clap::error::ErrorKind::ArgumentConflict
        );
    }
}

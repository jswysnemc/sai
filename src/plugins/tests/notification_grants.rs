use super::support::{descriptor, write_package};
use crate::plugins::{self, GrantChanges, GrantUpdate};
use crate::{config::AppConfig, paths::SaiPaths};
use clap::Parser;
use serde_json::json;

/// 【通知授权测试】【分项更新】投递授权和答复策略授权分别修改，未声明的新权限不能写入配置。
#[test]
fn notification_delivery_grants_do_not_change_presentation_permissions() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let mut package = descriptor("notify-grants", "");
    package.package.manifest.capabilities=serde_json::from_value(json!({"notifications":true,"system":{"notify":true,"read_paths":["."],"session_storage":true}})).unwrap();
    let directory = paths.config_dir.join("plugins/notify-grants");
    write_package(&directory, &package);
    plugins::set_enabled(
        &config,
        &paths,
        "notify-grants",
        true,
        GrantUpdate::Declared,
    )
    .unwrap();
    let saved = || {
        plugins::config::load_config(&paths).unwrap().plugins["notify-grants"]
            .grants
            .clone()
            .unwrap()
    };
    let baseline = saved();
    for notify in [false, true] {
        plugins::set_enabled(
            &config,
            &paths,
            "notify-grants",
            true,
            GrantUpdate::Changes(GrantChanges {
                notify: Some(notify),
                ..Default::default()
            }),
        )
        .unwrap();
        let mut expected = baseline.clone();
        expected.system.notify = notify;
        assert_eq!(saved(), expected);
    }
    package.package.manifest.capabilities.system.notify = false;
    write_package(&directory, &package);
    let before = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
    assert!(plugins::set_enabled(
        &config,
        &paths,
        "notify-grants",
        true,
        GrantUpdate::Changes(GrantChanges {
            notify: Some(true),
            ..Default::default()
        })
    )
    .is_err());
    assert_eq!(
        std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
        before
    );
}

/// 【通知授权测试】【CLI 参数】独立开关可与旧策略开关组合，相反开关或全量授权不能混用。
#[test]
fn notification_delivery_cli_flags_are_independent_and_conflict_checked() {
    for flags in [
        vec!["--allow-notify"],
        vec!["--no-notify"],
        vec!["--allow-notify", "--no-notifications"],
        vec!["--no-notify", "--allow-notifications"],
    ] {
        let mut args = vec!["sai", "plugins", "enable", "test"];
        args.extend(flags);
        assert!(crate::cli::Cli::try_parse_from(args).is_ok());
    }
    for flags in [
        vec!["--allow-notify", "--no-notify"],
        vec!["--grant-declared", "--allow-notify"],
        vec!["--grant-declared", "--no-notify"],
    ] {
        let mut args = vec!["sai", "plugins", "enable", "test"];
        args.extend(flags);
        assert_eq!(
            crate::cli::Cli::try_parse_from(args).unwrap_err().kind(),
            clap::error::ErrorKind::ArgumentConflict
        );
    }
}

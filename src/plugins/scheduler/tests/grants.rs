use super::fixture;
use crate::{
    config::AppConfig,
    plugins::{self, GrantChanges, GrantUpdate},
};
use clap::Parser;

/// 【调度授权测试】【分项更新】调度撤权保留存储权限，未声明授权失败时不改写配置。
#[test]
fn scheduling_grants_preserve_other_permissions_and_reject_undeclared_access() {
    let (_root, paths, mut descriptor) = fixture();
    let config = AppConfig::default();
    for schedule in [false, true] {
        plugins::set_enabled(
            &config,
            &paths,
            "schedule-fixture",
            true,
            GrantUpdate::Changes(GrantChanges {
                schedule: Some(schedule),
                ..Default::default()
            }),
        )
        .unwrap();
        let saved = plugins::config::load_config(&paths).unwrap().plugins["schedule-fixture"]
            .grants
            .clone()
            .unwrap();
        assert_eq!(saved.system.schedule, schedule);
        assert!(saved.system.plugin_storage);
    }
    descriptor.package.manifest.capabilities.system.schedule = false;
    std::fs::write(
        paths
            .config_dir
            .join("plugins/schedule-fixture/sai-plugin.json"),
        serde_json::to_vec(&descriptor.package.manifest).unwrap(),
    )
    .unwrap();
    let path = paths.config_dir.join("plugins.jsonc");
    let before = std::fs::read(&path).unwrap();
    assert!(plugins::set_enabled(
        &config,
        &paths,
        "schedule-fixture",
        true,
        GrantUpdate::Changes(GrantChanges {
            schedule: Some(true),
            ..Default::default()
        })
    )
    .is_err());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

/// 【调度授权测试】【命令冲突】独立开关支持组合，互斥开关与全量授权不能混用。
#[test]
fn scheduler_cli_options_are_independent_and_conflict_checked() {
    for flags in [
        vec!["--allow-schedule"],
        vec!["--no-schedule"],
        vec!["--allow-schedule", "--no-notify"],
        vec!["--no-schedule", "--allow-plugin-storage"],
    ] {
        let mut args = vec!["sai", "plugins", "enable", "example"];
        args.extend(flags);
        assert!(crate::cli::Cli::try_parse_from(args).is_ok());
    }
    for flags in [
        vec!["--allow-schedule", "--no-schedule"],
        vec!["--grant-declared", "--allow-schedule"],
        vec!["--grant-declared", "--no-schedule"],
    ] {
        let mut args = vec!["sai", "plugins", "enable", "example"];
        args.extend(flags);
        assert_eq!(
            crate::cli::Cli::try_parse_from(args).unwrap_err().kind(),
            clap::error::ErrorKind::ArgumentConflict
        );
    }
}

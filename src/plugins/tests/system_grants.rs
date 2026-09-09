use super::support::{descriptor, write_package};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::config::load_config;
use crate::plugins::{self, GrantChanges, GrantUpdate};
use serde_json::json;

/// 【系统授权测试】【分项修改】文件、环境、进程和既有权限分别更新，非法模板不能写入配置。
#[test]
fn system_grant_updates_are_independent_and_atomic() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let mut package = descriptor("system-grants", "");
    package.package.manifest.capabilities=serde_json::from_value(json!({
        "http":["https://example.test"],"model":true,"tools":["read_file"],
        "system":{"read_paths":["."],"environment":["LANG"],"processes":{"sample":{"program":"example","read_only":true}}}
    })).unwrap();
    let path = paths.config_dir.join("plugins/system-grants");
    write_package(&path, &package);
    plugins::set_enabled(
        &config,
        &paths,
        "system-grants",
        true,
        GrantUpdate::Declared,
    )
    .unwrap();
    let saved = || {
        load_config(&paths).unwrap().plugins["system-grants"]
            .grants
            .clone()
            .unwrap()
    };
    plugins::set_enabled(
        &config,
        &paths,
        "system-grants",
        true,
        GrantUpdate::Changes(GrantChanges {
            environment: Some(Default::default()),
            ..Default::default()
        }),
    )
    .unwrap();
    let grants = saved();
    assert!(grants.system.environment.is_empty());
    assert_eq!(grants.system.read_paths.len(), 1);
    assert_eq!(grants.system.processes.len(), 1);
    assert_eq!(grants.http.len(), 1);
    assert!(grants.model);
    assert_eq!(grants.tools.len(), 1);
    for change in [
        GrantChanges {
            read_paths: Some(["/".into()].into()),
            ..Default::default()
        },
        GrantChanges {
            processes: Some(["unknown".into()].into()),
            ..Default::default()
        },
    ] {
        let before = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
        assert!(plugins::set_enabled(
            &config,
            &paths,
            "system-grants",
            true,
            GrantUpdate::Changes(change)
        )
        .is_err());
        assert_eq!(
            std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
            before
        );
    }
    plugins::set_enabled(
        &config,
        &paths,
        "system-grants",
        true,
        GrantUpdate::Changes(GrantChanges {
            processes: Some(Default::default()),
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(saved().system.processes.is_empty());
    assert_eq!(saved().system.read_paths.len(), 1);
}

/// 【系统授权测试】【更新撤权】修改模板不能复用旧授权，撤销其他能力后也不能重新激活旧模板。
#[test]
fn changing_a_process_template_revokes_old_grants_without_blocking_other_updates() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let mut package = descriptor("template-change", "");
    package.package.manifest.capabilities=serde_json::from_value(json!({"system":{
        "read_paths":["."],"environment":["LANG"],"processes":{"sample":{"program":"original","read_only":true}}
    }})).unwrap();
    let directory = paths.config_dir.join("plugins/template-change");
    write_package(&directory, &package);
    plugins::set_enabled(
        &config,
        &paths,
        "template-change",
        true,
        GrantUpdate::Declared,
    )
    .unwrap();
    package
        .package
        .manifest
        .capabilities
        .system
        .processes
        .get_mut("sample")
        .unwrap()
        .program = "changed".into();
    write_package(&directory, &package);
    let found = plugins::discovery::find(&config, &paths, "template-change").unwrap();
    assert!(found
        .capabilities()
        .intersection(&found.grants())
        .system
        .processes
        .is_empty());
    plugins::set_enabled(
        &config,
        &paths,
        "template-change",
        true,
        GrantUpdate::Changes(GrantChanges {
            environment: Some(Default::default()),
            ..Default::default()
        }),
    )
    .unwrap();
    let found = plugins::discovery::find(&config, &paths, "template-change").unwrap();
    assert!(found.grants().system.processes.is_empty());
    assert_eq!(found.grants().system.read_paths.len(), 1);
    plugins::set_enabled(
        &config,
        &paths,
        "template-change",
        true,
        GrantUpdate::Changes(GrantChanges {
            processes: Some(["sample".into()].into()),
            ..Default::default()
        }),
    )
    .unwrap();
    let found = plugins::discovery::find(&config, &paths, "template-change").unwrap();
    assert_eq!(found.grants().system.processes["sample"].program, "changed");
}

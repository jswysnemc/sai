use super::support::{descriptor, write_package};
use crate::{
    config::AppConfig,
    paths::SaiPaths,
    plugins::{self, config::load_config, GrantChanges, GrantUpdate},
};
use serde_json::json;

/// 【删除授权测试】【独立更新】两类删除权限分别撤销，其他读取、写入与存储授权保持独立
/// @returns 无；非法授权不能改变配置文件
#[test]
fn file_removal_grants_update_independently_and_reject_undeclared_paths_atomically() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let mut package = descriptor("removal-grants", "");
    package.package.manifest.capabilities = serde_json::from_value(json!({
        "system":{"remove_paths":["output"],"trash_paths":["output"],"read_paths":["output"],"plugin_storage":true},
        "binary":{"write_paths":["output"]}
    })).unwrap();
    write_package(&paths.config_dir.join("plugins/removal-grants"), &package);
    plugins::set_enabled(
        &config,
        &paths,
        "removal-grants",
        true,
        GrantUpdate::Declared,
    )
    .unwrap();
    let saved = || {
        load_config(&paths).unwrap().plugins["removal-grants"]
            .grants
            .clone()
            .unwrap()
    };
    for remove in [true, false] {
        let changes = if remove {
            GrantChanges {
                remove_paths: Some(Default::default()),
                ..Default::default()
            }
        } else {
            GrantChanges {
                trash_paths: Some(Default::default()),
                ..Default::default()
            }
        };
        plugins::set_enabled(
            &config,
            &paths,
            "removal-grants",
            true,
            GrantUpdate::Changes(changes),
        )
        .unwrap();
        let grants = saved();
        assert!(grants.system.remove_paths.is_empty());
        assert_eq!(grants.system.trash_paths.is_empty(), !remove);
        assert_eq!(grants.system.read_paths.len(), 1);
        assert_eq!(grants.binary.write_paths.len(), 1);
        assert!(grants.system.plugin_storage);
    }
    for changes in [
        GrantChanges {
            remove_paths: Some(["outside".into()].into()),
            ..Default::default()
        },
        GrantChanges {
            trash_paths: Some(["outside".into()].into()),
            ..Default::default()
        },
    ] {
        let before = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
        assert!(plugins::set_enabled(
            &config,
            &paths,
            "removal-grants",
            true,
            GrantUpdate::Changes(changes)
        )
        .is_err());
        assert_eq!(
            std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
            before
        );
    }
}

/// 【删除授权测试】【清单变化】改变目录声明会撤销旧目录权限，分项更新不能重新激活旧授权
/// @returns 无；重新授予时只能选择新清单中的目录
#[test]
fn file_removal_manifest_path_changes_revoke_old_grants() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let mut package = descriptor("removal-revision", "");
    package.package.manifest.capabilities = serde_json::from_value(json!({"system":{"remove_paths":["before"],"trash_paths":["before"],"read_paths":["output"]}})).unwrap();
    let directory = paths.config_dir.join("plugins/removal-revision");
    write_package(&directory, &package);
    plugins::set_enabled(
        &config,
        &paths,
        "removal-revision",
        true,
        GrantUpdate::Declared,
    )
    .unwrap();
    package.package.manifest.capabilities.system.remove_paths = ["after".into()].into();
    package.package.manifest.capabilities.system.trash_paths = ["after".into()].into();
    write_package(&directory, &package);
    plugins::set_enabled(
        &config,
        &paths,
        "removal-revision",
        true,
        GrantUpdate::Changes(GrantChanges {
            read_paths: Some(Default::default()),
            ..Default::default()
        }),
    )
    .unwrap();
    let found = plugins::discovery::find(&config, &paths, "removal-revision").unwrap();
    assert!(found.grants().system.remove_paths.is_empty());
    assert!(found.grants().system.trash_paths.is_empty());
    plugins::set_enabled(
        &config,
        &paths,
        "removal-revision",
        true,
        GrantUpdate::Changes(GrantChanges {
            trash_paths: Some(["after".into()].into()),
            ..Default::default()
        }),
    )
    .unwrap();
    let found = plugins::discovery::find(&config, &paths, "removal-revision").unwrap();
    assert_eq!(found.grants().system.trash_paths, ["after".into()].into());
    assert!(found.grants().system.remove_paths.is_empty());
}

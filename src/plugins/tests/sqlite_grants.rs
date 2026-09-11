use super::binary_conditional_support::digest;
use super::sqlite_support::*;
use crate::{
    config::AppConfig,
    paths::SaiPaths,
    plugins::{self, GrantChanges, GrantUpdate},
    tools::ToolRegistry,
};
use serde_json::json;
use std::{path::Path, sync::Arc};

/// 【数据库授权测试】【外部注册】读取当前配置，通过正式宿主注册一个已启用的外部包
/// @param root 隔离目录；readonly 为计划模式
/// @returns 当前配置对应的工具表
fn registered(root: &Path, readonly: bool) -> ToolRegistry {
    let paths = SaiPaths::for_tests(root);
    let descriptor =
        plugins::discovery::find(&AppConfig::default(), &paths, "sqlite-files").unwrap();
    let mut registry = ToolRegistry::new();
    if descriptor.setting.enabled {
        let host = Arc::new(
            plugins::private::PrivatePluginHost::for_descriptor(&paths, &descriptor).unwrap(),
        );
        plugins::registry::register_descriptor(&mut registry, descriptor, host, readonly).unwrap();
    }
    registry
}

/// 【数据库授权测试】【独立能力】计算不需要文件授权，读取与发布必须各自具备相应权限
/// @returns 无；伪造 Lua 参数不能扩大调用写入许可
#[tokio::test]
async fn sqlite_memory_computation_does_not_grant_file_read_or_write_access() {
    let root = tempfile::tempdir().unwrap();
    let before = seed(root.path());
    for (read, write, allow) in [
        (false, true, true),
        (true, false, true),
        (true, true, false),
    ] {
        let plugin = runtime(root.path(), |grants| {
            if !read {
                grants.system.read_paths.clear();
            }
            if !write {
                grants.binary.write_paths.clear();
            }
        });
        assert_eq!(
            call(
                &plugin,
                root.path(),
                "query",
                query("allowed/notes.db"),
                allow
            )
            .await
            .is_ok(),
            read
        );
        let args = json!({"source":"allowed/notes.db","path":"allowed/notes.db","expected":digest(&before),"changes":changes("denied"),"allow_writes":true});
        assert!(call(&plugin, root.path(), "apply", args, allow)
            .await
            .is_err());
        let memory=call(&plugin,root.path(),"apply",json!({"changes":[{"op":"create_table","name":"notes","columns":[{"name":"id","kind":"integer"}]}]}),false).await.unwrap();
        assert_eq!(memory["bytes"], 8192);
        assert_eq!(
            std::fs::read(root.path().join("allowed/notes.db")).unwrap(),
            before
        );
    }
}

/// 【数据库授权测试】【计划模式】只读目录保留计算和查询，隐藏写入入口并禁止可选入口发布
/// @returns 无；只读调用不会写入已授权目录
#[tokio::test]
async fn plan_mode_keeps_snapshot_computation_but_blocks_publication() {
    let root = tempfile::tempdir().unwrap();
    let before = seed(root.path());
    let paths = SaiPaths::for_tests(root.path());
    super::support::write_package(
        &paths.config_dir.join("plugins/sqlite-files"),
        &descriptor(),
    );
    plugins::set_enabled(
        &AppConfig::default(),
        &paths,
        "sqlite-files",
        true,
        GrantUpdate::Declared,
    )
    .unwrap();
    let registry = registered(root.path(), true);
    assert!(registry.contains("lua__sqlite-files__query"));
    assert!(registry.contains("lua__sqlite-files__apply"));
    assert!(!registry.contains("lua__sqlite-files__write"));
    crate::runtime_cwd::scope(root.path().to_path_buf(), async {
        registry
            .call(
                "lua__sqlite-files__query",
                &query("allowed/notes.db").to_string(),
            )
            .await
            .unwrap();
        let mut args = json!({"source":"allowed/notes.db","changes":changes("in memory")});
        registry
            .call("lua__sqlite-files__apply", &args.to_string())
            .await
            .unwrap();
        args["path"] = json!("allowed/notes.db");
        args["expected"] = json!(digest(&before));
        assert!(registry
            .call("lua__sqlite-files__apply", &args.to_string())
            .await
            .is_err());
    })
    .await;
    assert_eq!(
        std::fs::read(root.path().join("allowed/notes.db")).unwrap(),
        before
    );
}

/// 【数据库授权测试】【配置撤权】撤销读取或写入在重新加载后生效，未改变的能力继续可用
/// @returns 无；旧实例不能在目录重建时恢复已撤销授权，禁用移除工具
#[tokio::test]
async fn reloading_revoked_grants_and_disabled_packages_removes_their_access() {
    let root = tempfile::tempdir().unwrap();
    let before = seed(root.path());
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::support::write_package(
        &paths.config_dir.join("plugins/sqlite-files"),
        &descriptor(),
    );
    plugins::set_enabled(&config, &paths, "sqlite-files", true, GrantUpdate::Declared).unwrap();
    let initial = registered(root.path(), false);
    for read in [false, true] {
        plugins::set_enabled(&config, &paths, "sqlite-files", true, GrantUpdate::Declared).unwrap();
        let grant_changes = if read {
            GrantChanges {
                read_paths: Some(Default::default()),
                ..Default::default()
            }
        } else {
            GrantChanges {
                write_paths: Some(Default::default()),
                ..Default::default()
            }
        };
        plugins::set_enabled(
            &config,
            &paths,
            "sqlite-files",
            true,
            GrantUpdate::Changes(grant_changes),
        )
        .unwrap();
        let mut current = registered(root.path(), false);
        current.continue_plugin_session(&initial);
        crate::runtime_cwd::scope(root.path().to_path_buf(),async {
            assert_eq!(current.call("lua__sqlite-files__query",&query("allowed/notes.db").to_string()).await.is_ok(),!read);
            let args=json!({"source":"allowed/notes.db","path":"allowed/notes.db","expected":digest(&before),"changes":changes("denied")});
            assert!(current.call("lua__sqlite-files__apply",&args.to_string()).await.is_err());
        }).await;
        assert_eq!(
            std::fs::read(root.path().join("allowed/notes.db")).unwrap(),
            before
        );
    }
    plugins::set_enabled(&config, &paths, "sqlite-files", false, GrantUpdate::Keep).unwrap();
    let mut disabled = registered(root.path(), false);
    disabled.continue_plugin_session(&initial);
    assert!(!disabled.contains("lua__sqlite-files__query"));
}

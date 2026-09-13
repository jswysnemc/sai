use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost, state::StateStore};
use sai_plugin_runtime::{
    host::{PluginHost, StorageRequest},
    Capabilities,
};
use serde_json::{json, Value};

/// 【会话存储测试】【删除隔离】删除一个工作区会话时清理所有插件记录，保留其他会话和持久数据
/// @returns 无；同名 default 会话使用完整目录区分，CLI 与工作区删除入口均清理公共记录
#[tokio::test]
async fn session_deletion_cleans_plugin_scopes_without_touching_other_workspaces() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut capabilities = Capabilities::default();
    capabilities.system.session_storage = true;
    capabilities.system.plugin_storage = true;
    let workspaces = [root.path().join("first"), root.path().join("second")];
    let mut scopes = Vec::new();
    let hosts = [
        PrivatePluginHost::new(&paths, "first-plugin"),
        PrivatePluginHost::new(&paths, "second-plugin"),
    ];
    for work in &workspaces {
        std::fs::create_dir(work).unwrap();
        let store = StateStore::for_workspace_session(&paths, work, "default").unwrap();
        let scope = store.state_dir().display().to_string();
        for host in &hosts {
            host.storage(
                StorageRequest::Set {
                    key: "record".into(),
                    value: json!({"scope":scope}),
                },
                &scope,
                &capabilities,
            )
            .unwrap();
            host.plugin_storage(
                StorageRequest::Set {
                    key: "persistent".into(),
                    value: json!("retained"),
                },
                &capabilities,
                true,
            )
            .unwrap();
        }
        scopes.push(scope);
    }

    // 1. 【会话存储测试】【工作区删除】两个插件都只清理第一个工作区的目标会话
    assert_eq!(
        crate::state::delete_sessions_for_workspace(&paths, &workspaces[0], &["default".into()])
            .unwrap(),
        vec!["default"]
    );
    for host in &hosts {
        assert_eq!(
            host.storage(
                StorageRequest::Get {
                    key: "record".into()
                },
                &scopes[0],
                &capabilities
            )
            .unwrap(),
            Value::Null
        );
        assert_eq!(
            host.storage(
                StorageRequest::Get {
                    key: "record".into()
                },
                &scopes[1],
                &capabilities
            )
            .unwrap(),
            json!({"scope":scopes[1]})
        );
    }

    // 2. 【会话存储测试】【当前会话删除】CLI 使用同一清理边界，跨会话持久记录继续保留
    crate::runtime_cwd::scope(workspaces[1].clone(), async {
        assert!(crate::state::delete_session(&paths, "default").unwrap());
    })
    .await;
    for host in &hosts {
        assert_eq!(
            host.storage(
                StorageRequest::Get {
                    key: "record".into()
                },
                &scopes[1],
                &capabilities
            )
            .unwrap(),
            Value::Null
        );
        assert_eq!(
            host.plugin_storage(
                StorageRequest::Get {
                    key: "persistent".into()
                },
                &capabilities,
                false
            )
            .unwrap(),
            json!("retained")
        );
    }
}

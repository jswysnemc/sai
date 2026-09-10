use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
use sai_plugin_runtime::{
    host::{PluginHost, StorageRequest},
    Capabilities, InvocationContext, PluginPackage, PluginRuntime,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

/// 【插件存储测试】【授权】构造可独立验证两种存储的有效授权。
/// @returns 会话和插件存储能力
pub(super) fn capabilities() -> Capabilities {
    serde_json::from_value(json!({"system":{"plugin_storage":true,"session_storage":true}}))
        .unwrap()
}

/// 【插件存储测试】【记录路径】定位宿主生成的记录，以构造损坏文件和链接边界。
/// @param paths 临时应用路径；id 为插件标识；key 为记录键
/// @returns 插件作用域内的记录路径
pub(super) fn record_path(paths: &SaiPaths, id: &str, key: &str) -> PathBuf {
    paths
        .state_dir
        .join("plugin-storage")
        .join(blake3::hash(id.as_bytes()).to_hex().to_string())
        .join(blake3::hash(b"").to_hex().to_string())
        .join(format!("{}.json", blake3::hash(key.as_bytes()).to_hex()))
}

/// 【插件存储测试】【归属隔离】插件记录跨宿主重建保留，与其他插件及空会话记录分别隔离。
#[test]
fn plugin_storage_persists_across_hosts_without_colliding_with_session_state() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let caps = capabilities();
    let host = PrivatePluginHost::new(&paths, "first");
    let key = "../arbitrary/key";
    let value = json!({"name":"跨会话", "items":[1,null,false]});
    host.plugin_storage(
        StorageRequest::Set {
            key: key.into(),
            value: value.clone(),
        },
        &caps,
        true,
    )
    .unwrap();
    for session in ["", "a", "b"] {
        assert_eq!(
            host.storage(StorageRequest::Get { key: key.into() }, session, &caps)
                .unwrap(),
            Value::Null
        );
        host.storage(
            StorageRequest::Set {
                key: key.into(),
                value: json!(session),
            },
            session,
            &caps,
        )
        .unwrap();
    }
    drop(host);
    let restored = PrivatePluginHost::new(&paths, "first");
    assert_eq!(
        restored
            .plugin_storage(StorageRequest::Get { key: key.into() }, &caps, false)
            .unwrap(),
        value
    );
    let other = PrivatePluginHost::new(&paths, "second");
    assert_eq!(
        other
            .plugin_storage(StorageRequest::Get { key: key.into() }, &caps, false)
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        restored
            .plugin_storage(
                StorageRequest::CompareExchange {
                    key: key.into(),
                    expected: json!(false),
                    value: json!(99)
                },
                &caps,
                true
            )
            .unwrap(),
        false
    );
    assert_eq!(
        restored
            .plugin_storage(
                StorageRequest::CompareExchange {
                    key: key.into(),
                    expected: value,
                    value: json!([2, 3])
                },
                &caps,
                true
            )
            .unwrap(),
        true
    );
    assert_eq!(
        restored
            .plugin_storage(StorageRequest::Get { key: key.into() }, &caps, false)
            .unwrap(),
        json!([2, 3])
    );
    assert_eq!(
        restored
            .plugin_storage(
                StorageRequest::CompareExchange {
                    key: key.into(),
                    expected: json!([2, 3]),
                    value: Value::Null
                },
                &caps,
                true
            )
            .unwrap(),
        true
    );
    assert_eq!(
        restored
            .plugin_storage(StorageRequest::Get { key: key.into() }, &caps, false)
            .unwrap(),
        Value::Null
    );
    for session in ["", "a", "b"] {
        assert_eq!(
            restored
                .storage(StorageRequest::Get { key: key.into() }, session, &caps)
                .unwrap(),
            session
        );
    }
    assert!(!root.path().join("arbitrary").exists());
}

/// 【插件存储测试】【重置边界】正式会话重置只清理会话记录，不删除插件持久记录。
#[test]
fn conversation_reset_preserves_plugin_storage_and_other_sessions() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let state = crate::state::StateStore::new(&paths).unwrap();
    let host = PrivatePluginHost::new(&paths, "first");
    let caps = capabilities();
    let scope = state.state_dir().to_string_lossy();
    host.plugin_storage(
        StorageRequest::Set {
            key: "key".into(),
            value: json!("persistent"),
        },
        &caps,
        true,
    )
    .unwrap();
    for session in [scope.as_ref(), "other", ""] {
        host.storage(
            StorageRequest::Set {
                key: "key".into(),
                value: json!(session),
            },
            session,
            &caps,
        )
        .unwrap();
    }
    state.reset_conversation().unwrap();
    assert_eq!(
        host.storage(StorageRequest::Get { key: "key".into() }, &scope, &caps)
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        host.storage(StorageRequest::Get { key: "key".into() }, "other", &caps)
            .unwrap(),
        "other"
    );
    crate::plugins::clear_session_storage(&paths.state_dir, "").unwrap();
    assert_eq!(
        host.storage(StorageRequest::Get { key: "key".into() }, "", &caps)
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        host.plugin_storage(StorageRequest::Get { key: "key".into() }, &caps, false)
            .unwrap(),
        "persistent"
    );
}

/// 【插件存储测试】【命令行清理】普通和完整清理都撤销直接调用记录，保留插件记录与其他会话。
#[test]
fn cli_clear_removes_direct_call_scopes_without_erasing_plugin_storage() {
    for all in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let host = PrivatePluginHost::new(&paths, "first");
        let caps = capabilities();
        for session in [
            "cli-tool",
            "plugin-command",
            "direct-command",
            "other-session",
        ] {
            host.storage(
                StorageRequest::Set {
                    key: "key".into(),
                    value: json!(session),
                },
                session,
                &caps,
            )
            .unwrap();
        }
        host.plugin_storage(
            StorageRequest::Set {
                key: "key".into(),
                value: json!("persistent"),
            },
            &caps,
            true,
        )
        .unwrap();
        crate::control_commands::clear_state(&paths, all).unwrap();
        for session in ["cli-tool", "plugin-command", "direct-command"] {
            assert_eq!(
                host.storage(StorageRequest::Get { key: "key".into() }, session, &caps)
                    .unwrap(),
                Value::Null,
                "{session}"
            );
        }
        assert_eq!(
            host.storage(
                StorageRequest::Get { key: "key".into() },
                "other-session",
                &caps
            )
            .unwrap(),
            "other-session"
        );
        assert_eq!(
            host.plugin_storage(StorageRequest::Get { key: "key".into() }, &caps, false)
                .unwrap(),
            "persistent"
        );
    }
}

/// 【插件存储测试】【宿主复核】宿主独立验证能力、可信写入权限及请求边界，非法请求不创建目录。
#[test]
fn host_rejects_ungranted_readonly_and_invalid_requests_before_io() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let host = PrivatePluginHost::new(&paths, "first");
    let caps = capabilities();
    for grants in [
        Capabilities::default(),
        super::private_storage::capabilities(),
    ] {
        for request in [
            StorageRequest::Get { key: "key".into() },
            StorageRequest::Set {
                key: "key".into(),
                value: json!(1),
            },
        ] {
            assert!(host.plugin_storage(request, &grants, true).is_err());
        }
    }
    for request in [
        StorageRequest::Set {
            key: "key".into(),
            value: Value::Null,
        },
        StorageRequest::CompareExchange {
            key: "key".into(),
            expected: Value::Null,
            value: Value::Null,
        },
    ] {
        assert!(host.plugin_storage(request, &caps, false).is_err());
    }
    for key in [String::new(), "x".repeat(257), "control\n".into()] {
        assert!(host
            .plugin_storage(StorageRequest::Get { key }, &caps, true)
            .is_err());
    }
    for request in [
        StorageRequest::Set {
            key: "key".into(),
            value: json!("x".repeat(256 * 1024)),
        },
        StorageRequest::CompareExchange {
            key: "key".into(),
            expected: json!("x".repeat(256 * 1024)),
            value: json!(1),
        },
        StorageRequest::CompareExchange {
            key: "key".into(),
            expected: json!("missing"),
            value: json!("x".repeat(256 * 1024)),
        },
    ] {
        assert!(host.plugin_storage(request, &caps, true).is_err());
    }
    assert!(!paths.state_dir.exists());
}

/// 【插件存储测试】【并发事务】多个宿主同时比较同一个空记录，仅一个调用能够提交新值。
#[test]
fn concurrent_compare_exchange_has_one_winner_and_recovers_from_contention() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let barrier = Arc::new(Barrier::new(8));
    let workers: Vec<_> = (0..8)
        .map(|index| {
            let paths = paths.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let host = PrivatePluginHost::new(&paths, "first");
                let caps = capabilities();
                barrier.wait();
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    let result = host.plugin_storage(
                        StorageRequest::CompareExchange {
                            key: "winner".into(),
                            expected: Value::Null,
                            value: json!(index),
                        },
                        &caps,
                        true,
                    );
                    match result {
                        Ok(value) => return (index, value == true),
                        Err(error)
                            if format!("{error:#}").contains("busy")
                                && Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(1));
                        }
                        Err(error) => panic!("unexpected storage failure: {error:#}"),
                    }
                }
            })
        })
        .collect();
    let winners: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .filter(|(_, won)| *won)
        .collect();
    assert_eq!(winners.len(), 1);
    let host = PrivatePluginHost::new(&paths, "first");
    assert_eq!(
        host.plugin_storage(
            StorageRequest::Get {
                key: "winner".into()
            },
            &capabilities(),
            false
        )
        .unwrap(),
        winners[0].0
    );
}

/// 【插件存储测试】【真实运行时】不同会话与新运行时共享绑定插件的记录，伪造归属字段无效。
#[tokio::test]
async fn runtime_recreation_and_session_changes_keep_only_the_bound_plugins_data() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let source = r#"
        sai.register_tool({name="state",description="state",access="writes",parameters={type="object"},execute=function(args, ctx)
            sai.plugin_id = "forged"
            ctx.session_id = "forged"
            ctx.storage_session_id = "forged"
            if args.write then sai.storage.plugin.set("key", args.write) end
            return sai.storage.plugin.get("key")
        end})
    "#;
    let load = |id| {
        let mut descriptor = super::support::descriptor(id, source);
        descriptor.package.manifest.capabilities = capabilities();
        let package = PluginPackage::new(
            descriptor.package.manifest.clone(),
            descriptor.package.sources().clone(),
        )
        .unwrap();
        PluginRuntime::load(
            package,
            json!({}),
            capabilities(),
            Arc::new(PrivatePluginHost::new(&paths, id)),
        )
        .unwrap()
    };
    let plugin = load("first");
    let context = InvocationContext {
        session_id: "one".into(),
        allow_writes: true,
        ..Default::default()
    };
    assert_eq!(
        plugin
            .call_tool("state", json!({"write":42}), context)
            .await
            .unwrap(),
        "42"
    );
    for session in ["two", ""] {
        assert_eq!(
            plugin
                .call_tool(
                    "state",
                    json!({}),
                    InvocationContext {
                        session_id: session.into(),
                        ..Default::default()
                    }
                )
                .await
                .unwrap(),
            "42"
        );
    }
    drop(plugin);
    assert_eq!(
        load("first")
            .call_tool("state", json!({}), Default::default())
            .await
            .unwrap(),
        "42"
    );
    assert_eq!(
        load("forged")
            .call_tool("state", json!({}), Default::default())
            .await
            .unwrap(),
        ""
    );
}

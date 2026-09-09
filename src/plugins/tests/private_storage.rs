use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
use sai_plugin_runtime::{
    host::{PluginHost, StorageRequest},
    Capabilities,
};
use serde_json::{json, Value};

/// 【私有状态测试】【授权】构造只包含私有数据能力的授权。
/// @returns 状态和目录能力
pub(super) fn capabilities() -> Capabilities {
    serde_json::from_value(json!({"system":{"session_storage":true,"workspace":true}})).unwrap()
}

/// 【私有状态测试】【隔离与事务】同键不能跨插件或会话读取，比较交换失败不更改记录。
#[test]
fn private_storage_isolates_plugins_sessions_and_atomic_updates() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let first = PrivatePluginHost::new(&paths, "first");
    let second = PrivatePluginHost::new(&paths, "second");
    let caps = capabilities();
    let key = "../outside/secret";
    first
        .storage(
            StorageRequest::Set {
                key: key.into(),
                value: json!({"review":1}),
            },
            "a",
            &caps,
        )
        .unwrap();
    assert_eq!(
        second
            .storage(StorageRequest::Get { key: key.into() }, "a", &caps)
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        first
            .storage(StorageRequest::Get { key: key.into() }, "b", &caps)
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        first
            .storage(
                StorageRequest::CompareExchange {
                    key: key.into(),
                    expected: json!({"review":2}),
                    value: json!(3)
                },
                "a",
                &caps
            )
            .unwrap(),
        false
    );
    assert_eq!(
        first
            .storage(
                StorageRequest::CompareExchange {
                    key: key.into(),
                    expected: json!({"review":1}),
                    value: json!(3)
                },
                "a",
                &caps
            )
            .unwrap(),
        true
    );
    assert_eq!(
        first
            .storage(StorageRequest::Get { key: key.into() }, "a", &caps)
            .unwrap(),
        3
    );
    assert!(!root.path().join("outside").exists());
    crate::plugins::clear_session_storage(&paths.state_dir, "a").unwrap();
    assert_eq!(
        first
            .storage(StorageRequest::Get { key: key.into() }, "a", &caps)
            .unwrap(),
        Value::Null
    );
}

/// 【私有状态测试】【共同清理入口】任何入口重置 StateStore 都撤销本会话记录，不影响其他会话。
#[test]
fn private_storage_follows_the_shared_conversation_reset() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let state = crate::state::StateStore::new(&paths).unwrap();
    let host = PrivatePluginHost::new(&paths, "test");
    let caps = capabilities();
    let scope = state.state_dir().to_string_lossy();
    for session in [scope.as_ref(), "other-session"] {
        host.storage(
            StorageRequest::Set {
                key: "review".into(),
                value: json!(true),
            },
            session,
            &caps,
        )
        .unwrap();
    }
    state.reset_conversation().unwrap();
    assert_eq!(
        host.storage(
            StorageRequest::Get {
                key: "review".into()
            },
            &scope,
            &caps
        )
        .unwrap(),
        Value::Null
    );
    assert_eq!(
        host.storage(
            StorageRequest::Get {
                key: "review".into()
            },
            "other-session",
            &caps
        )
        .unwrap(),
        true
    );
}

/// 【私有状态测试】【预算】缺少授权、非法键、过大值和过多键都不会扩大存储范围。
#[test]
fn private_storage_rejects_invalid_grants_keys_and_limits() {
    let root = tempfile::tempdir().unwrap();
    let host = PrivatePluginHost::new(&SaiPaths::for_tests(root.path()), "test");
    let caps = capabilities();
    assert!(host
        .storage(
            StorageRequest::Get { key: "ok".into() },
            "a",
            &Capabilities::default()
        )
        .is_err());
    for key in [String::new(), "x".repeat(257), "a\nb".into()] {
        assert!(host
            .storage(StorageRequest::Get { key }, "a", &caps)
            .is_err());
    }
    assert!(host
        .storage(
            StorageRequest::Set {
                key: "huge".into(),
                value: json!("x".repeat(256 * 1024))
            },
            "a",
            &caps
        )
        .is_err());
    for index in 0..128 {
        host.storage(
            StorageRequest::Set {
                key: index.to_string(),
                value: json!(index),
            },
            "a",
            &caps,
        )
        .unwrap();
    }
    assert!(host
        .storage(
            StorageRequest::Set {
                key: "overflow".into(),
                value: json!(0)
            },
            "a",
            &caps
        )
        .is_err());
    host.storage(
        StorageRequest::Set {
            key: "0".into(),
            value: Value::Null,
        },
        "a",
        &caps,
    )
    .unwrap();
    host.storage(
        StorageRequest::Set {
            key: "overflow".into(),
            value: json!(0),
        },
        "a",
        &caps,
    )
    .unwrap();
}

/// 【私有状态测试】【链接拒绝】即使记录被替换为符号链接，也不能读取外部文件。
#[cfg(unix)]
#[test]
fn private_storage_refuses_symlink_records() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let host = PrivatePluginHost::new(&paths, "test");
    let caps = capabilities();
    host.storage(
        StorageRequest::Set {
            key: "key".into(),
            value: json!(1),
        },
        "a",
        &caps,
    )
    .unwrap();
    let record = paths
        .state_dir
        .join("plugin-state")
        .join(blake3::hash(b"test").to_hex().to_string())
        .join(blake3::hash(b"a").to_hex().to_string())
        .join(format!("{}.json", blake3::hash(b"key").to_hex()));
    let outside = root.path().join("outside.json");
    std::fs::write(&outside, "2").unwrap();
    std::fs::remove_file(&record).unwrap();
    std::os::unix::fs::symlink(&outside, record).unwrap();
    assert!(host
        .storage(StorageRequest::Get { key: "key".into() }, "a", &caps)
        .is_err());
    assert_eq!(std::fs::read_to_string(outside).unwrap(), "2");
}

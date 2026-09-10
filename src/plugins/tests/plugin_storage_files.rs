use super::plugin_storage::{capabilities, record_path};
use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
use sai_plugin_runtime::host::{PluginHost, StorageRequest};
use serde_json::{json, Value};
use std::time::Duration;

/// 【插件存储测试】【配额】每个插件最多持有 128 个键，覆盖与删除不占额外配额。
#[test]
fn plugin_storage_enforces_per_plugin_key_and_value_limits() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let host = PrivatePluginHost::new(&paths, "first");
    let caps = capabilities();
    for index in 0..128 {
        host.plugin_storage(
            StorageRequest::Set {
                key: index.to_string(),
                value: json!(index),
            },
            &caps,
            true,
        )
        .unwrap();
    }
    let overflow = StorageRequest::Set {
        key: "extra".into(),
        value: json!(1),
    };
    assert!(format!(
        "{:#}",
        host.plugin_storage(overflow.clone(), &caps, true)
            .unwrap_err()
    )
    .contains("128 keys"));
    let boundary = json!("x".repeat(256 * 1024 - 2));
    assert_eq!(
        host.plugin_storage(
            StorageRequest::Set {
                key: "0".into(),
                value: boundary.clone()
            },
            &caps,
            true
        )
        .unwrap(),
        boundary
    );
    assert_eq!(
        host.plugin_storage(
            StorageRequest::CompareExchange {
                key: "0".into(),
                expected: boundary,
                value: Value::Null
            },
            &caps,
            true
        )
        .unwrap(),
        true
    );
    host.plugin_storage(overflow.clone(), &caps, true).unwrap();
    PrivatePluginHost::new(&paths, "second")
        .plugin_storage(overflow.clone(), &caps, true)
        .unwrap();
    host.storage(overflow, "", &caps).unwrap();
    let directory = record_path(&paths, "first", "extra")
        .parent()
        .unwrap()
        .to_path_buf();
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 128);
    assert!(std::fs::read_dir(directory).unwrap().all(|entry| entry
        .unwrap()
        .path()
        .extension()
        .unwrap()
        == "json"));
}

/// 【插件存储测试】【损坏恢复】过大或无效记录不能进入 Lua，也不能被静默覆盖。
#[test]
fn corrupt_and_oversized_records_fail_without_overwrite_then_recover() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let host = PrivatePluginHost::new(&paths, "first");
    let caps = capabilities();
    let record = record_path(&paths, "first", "key");
    host.plugin_storage(StorageRequest::Get { key: "key".into() }, &caps, false)
        .unwrap();
    for data in [b"invalid json".to_vec(), vec![b' '; 256 * 1024 + 1]] {
        std::fs::write(&record, &data).unwrap();
        assert!(host
            .plugin_storage(StorageRequest::Get { key: "key".into() }, &caps, false)
            .is_err());
        assert!(host
            .plugin_storage(
                StorageRequest::Set {
                    key: "key".into(),
                    value: json!(1)
                },
                &caps,
                true
            )
            .is_err());
        assert_eq!(std::fs::read(&record).unwrap(), data);
    }
    std::fs::write(record, "42").unwrap();
    assert_eq!(
        host.plugin_storage(StorageRequest::Get { key: "key".into() }, &caps, false)
            .unwrap(),
        42
    );
}

/// 【插件存储测试】【锁争用】已占用锁立即返回可重试错误，锁释放后可继续访问。
#[test]
fn storage_lock_contention_fails_promptly_without_blocking_session_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let host = PrivatePluginHost::new(&paths, "first");
    let caps = capabilities();
    host.plugin_storage(
        StorageRequest::Set {
            key: "key".into(),
            value: json!(1),
        },
        &caps,
        true,
    )
    .unwrap();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(paths.state_dir.join(".plugin-storage.lock"))
        .unwrap();
    file.try_lock().unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let cloned = paths.clone();
    let worker = std::thread::spawn(move || {
        let result = PrivatePluginHost::new(&cloned, "first").plugin_storage(
            StorageRequest::Get { key: "key".into() },
            &capabilities(),
            false,
        );
        send.send(result).unwrap();
    });
    let outcome = receive.recv_timeout(Duration::from_secs(2));
    crate::plugins::clear_session_storage(&paths.state_dir, "").unwrap();
    file.unlock().unwrap();
    worker.join().unwrap();
    assert!(format!(
        "{:#}",
        outcome
            .expect("storage must not wait for the lock")
            .unwrap_err()
    )
    .contains("busy"));
    assert_eq!(
        host.plugin_storage(StorageRequest::Get { key: "key".into() }, &caps, false)
            .unwrap(),
        1
    );
}

/// 【插件存储测试】【目录链接】类别、插件和作用域目录均拒绝符号链接，不读取或写入外部目录。
#[cfg(unix)]
#[test]
fn plugin_storage_rejects_symlinks_at_every_namespace_level() {
    for depth in 0..3 {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let host = PrivatePluginHost::new(&paths, "first");
        let caps = capabilities();
        host.plugin_storage(StorageRequest::Get { key: "key".into() }, &caps, false)
            .unwrap();
        let record = record_path(&paths, "first", "key");
        let directory = record.ancestors().nth(depth + 1).unwrap();
        std::fs::remove_dir_all(directory).unwrap();
        let outside = root.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("sentinel"), "unchanged").unwrap();
        std::os::unix::fs::symlink(&outside, directory).unwrap();
        assert!(host
            .plugin_storage(StorageRequest::Get { key: "key".into() }, &caps, false)
            .is_err());
        assert!(host
            .plugin_storage(
                StorageRequest::Set {
                    key: "key".into(),
                    value: json!(1)
                },
                &caps,
                true
            )
            .is_err());
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 1);
        assert_eq!(
            std::fs::read_to_string(outside.join("sentinel")).unwrap(),
            "unchanged"
        );
    }
}

/// 【插件存储测试】【记录链接】有效及悬空链接都不能作为 JSON 记录或锁文件使用。
#[cfg(unix)]
#[test]
fn plugin_storage_rejects_record_and_lock_symlinks() {
    for lock in [false, true] {
        for dangling in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let paths = SaiPaths::for_tests(root.path());
            let host = PrivatePluginHost::new(&paths, "first");
            let caps = capabilities();
            host.plugin_storage(
                StorageRequest::Set {
                    key: "key".into(),
                    value: json!(1),
                },
                &caps,
                true,
            )
            .unwrap();
            let target = if lock {
                paths.state_dir.join(".plugin-storage.lock")
            } else {
                record_path(&paths, "first", "key")
            };
            std::fs::remove_file(&target).unwrap();
            let outside = root.path().join("outside.json");
            if !dangling {
                std::fs::write(&outside, "2").unwrap();
            }
            std::os::unix::fs::symlink(&outside, target).unwrap();
            for request in [
                StorageRequest::Get { key: "key".into() },
                StorageRequest::Set {
                    key: "key".into(),
                    value: json!(3),
                },
                StorageRequest::CompareExchange {
                    key: "key".into(),
                    expected: json!(2),
                    value: json!(3),
                },
            ] {
                assert!(host.plugin_storage(request, &caps, true).is_err());
            }
            if dangling {
                assert!(!outside.exists());
            } else {
                assert_eq!(std::fs::read_to_string(outside).unwrap(), "2");
            }
        }
    }
}

/// 【插件存储测试】【特殊文件】FIFO 和目录不能用作记录或互斥锁，错误后可正常恢复。
#[cfg(unix)]
#[test]
fn plugin_storage_rejects_special_records_and_lock_files() {
    use std::os::unix::ffi::OsStrExt;
    for lock in [false, true] {
        for fifo in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let paths = SaiPaths::for_tests(root.path());
            let host = PrivatePluginHost::new(&paths, "first");
            let caps = capabilities();
            host.plugin_storage(
                StorageRequest::Set {
                    key: "key".into(),
                    value: json!(1),
                },
                &caps,
                true,
            )
            .unwrap();
            let target = if lock {
                paths.state_dir.join(".plugin-storage.lock")
            } else {
                record_path(&paths, "first", "key")
            };
            std::fs::remove_file(&target).unwrap();
            if fifo {
                let path = std::ffi::CString::new(target.as_os_str().as_bytes()).unwrap();
                assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
            } else {
                std::fs::create_dir(&target).unwrap();
            }
            assert!(host
                .plugin_storage(StorageRequest::Get { key: "key".into() }, &caps, false)
                .is_err());
            assert!(host
                .plugin_storage(
                    StorageRequest::Set {
                        key: "key".into(),
                        value: json!(2)
                    },
                    &caps,
                    true
                )
                .is_err());
            if fifo {
                std::fs::remove_file(target).unwrap();
            } else {
                std::fs::remove_dir(target).unwrap();
            }
            assert_eq!(
                host.plugin_storage(
                    StorageRequest::Set {
                        key: "key".into(),
                        value: json!(3)
                    },
                    &caps,
                    true
                )
                .unwrap(),
                3
            );
        }
    }
}

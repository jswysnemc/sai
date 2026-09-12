use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
use sai_plugin_runtime::{host::PluginHost, Capabilities};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

/// 【私有锁测试】【独立授权】无参数；返回只授予插件私有存储的能力
fn grants() -> Capabilities {
    serde_json::from_value(json!({"system":{"plugin_storage":true}})).unwrap()
}

/// 【私有锁测试】【物理位置】输入目录、插件和键；返回正式宿主使用的稳定锁文件路径
fn lock_path(root: &Path, plugin: &str, key: &str) -> PathBuf {
    SaiPaths::for_tests(root)
        .state_dir
        .join("plugin-locks")
        .join(blake3::hash(plugin.as_bytes()).to_hex().to_string())
        .join(blake3::hash(b"").to_hex().to_string())
        .join(format!("{}.lock", blake3::hash(key.as_bytes()).to_hex()))
}

/// 【私有锁测试】【竞争与隔离】同一插件跨实例互斥，其他插件及不同键互不阻塞
/// @returns 无；超时后仍可释放并再次取得同一锁
#[tokio::test]
async fn private_host_lock_serializes_instances_and_isolates_plugins() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let first = PrivatePluginHost::new(&paths, "one");
    let second = PrivatePluginHost::new(&paths, "one");
    let other = PrivatePluginHost::new(&paths, "two");
    let held = first.plugin_lock("same", 1000, &grants()).await.unwrap();
    let error = second
        .plugin_lock("same", 10, &grants())
        .await
        .err()
        .unwrap();
    assert!(format!("{error:#}").contains("plugin lock acquisition timed out"));
    drop(other.plugin_lock("same", 1000, &grants()).await.unwrap());
    drop(
        second
            .plugin_lock("different", 1000, &grants())
            .await
            .unwrap(),
    );
    drop(held);
    drop(second.plugin_lock("same", 1000, &grants()).await.unwrap());
}

/// 【私有锁测试】【取消等待】取消阻塞等待后，迟到工作线程不能占有或泄漏租约
/// @returns 无；撤销后原锁和后续调用都正常
#[tokio::test]
async fn private_host_lock_cancellation_releases_late_waiters() {
    let root = tempfile::tempdir().unwrap();
    let host = Arc::new(PrivatePluginHost::new(
        &SaiPaths::for_tests(root.path()),
        "cancel",
    ));
    let held = host.plugin_lock("key", 1000, &grants()).await.unwrap();
    let waiting = host.clone();
    let task = tokio::spawn(async move { waiting.plugin_lock("key", 600000, &grants()).await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!task.is_finished());
    task.abort();
    assert!(task.await.err().unwrap().is_cancelled());
    drop(held);
    let fresh = tokio::time::timeout(
        Duration::from_secs(2),
        host.plugin_lock("key", 1000, &grants()),
    )
    .await
    .unwrap()
    .unwrap();
    drop(fresh);
}

/// 【私有锁测试】【键数和稳定文件】已知键可以重复使用，达到 128 个键后拒绝新键
/// @returns 无；重复锁不替换文件，拒绝不删除已有锁
#[tokio::test]
async fn private_host_lock_limits_keys_and_preserves_existing_files() {
    let root = tempfile::tempdir().unwrap();
    let host = PrivatePluginHost::new(&SaiPaths::for_tests(root.path()), "limits");
    for index in 0..128 {
        drop(
            host.plugin_lock(&index.to_string(), 1000, &grants())
                .await
                .unwrap(),
        );
    }
    let path = lock_path(root.path(), "limits", "0");
    std::fs::write(&path, b"stable inode marker").unwrap();
    let before = std::fs::metadata(&path).unwrap();
    drop(host.plugin_lock("0", 1000, &grants()).await.unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), b"stable inode marker");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(before.ino(), std::fs::metadata(&path).unwrap().ino());
    }
    #[cfg(not(unix))]
    let _ = before;
    let error = host
        .plugin_lock("overflow", 1000, &grants())
        .await
        .err()
        .unwrap();
    assert!(format!("{error:#}").contains("128 keys"));
    assert!(!lock_path(root.path(), "limits", "overflow").exists());
}

/// 【私有锁测试】【宿主授权】直接调用宿主也不能绕过权限、键和等待时间校验
/// @returns 无；拒绝输入不创建私有锁目录
#[tokio::test]
async fn private_host_lock_rejects_invalid_requests_before_side_effects() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let host = PrivatePluginHost::new(&paths, "invalid");
    let cases = [
        ("key", 1, Capabilities::default()),
        ("", 1, grants()),
        ("key", 0, grants()),
        ("key", 600001, grants()),
    ];
    for (key, timeout, caps) in cases {
        assert!(host.plugin_lock(key, timeout, &caps).await.is_err());
    }
    assert!(!paths.state_dir.exists());
}

/// 【私有锁测试】【链接边界】锁文件与私有目录中的链接不能指向外部对象
/// @returns 无；外部文件保留原字节
#[cfg(unix)]
#[tokio::test]
async fn private_host_lock_rejects_symlinks_in_files_and_namespaces() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let host = PrivatePluginHost::new(&paths, "links");
    let path = lock_path(root.path(), "links", "key");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let outside = root.path().join("outside");
    std::fs::write(&outside, b"untouched").unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();
    assert!(host.plugin_lock("key", 10, &grants()).await.is_err());
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir_all(paths.state_dir.join("plugin-locks")).unwrap();
    std::os::unix::fs::symlink(root.path(), paths.state_dir.join("plugin-locks")).unwrap();
    assert!(host.plugin_lock("key", 10, &grants()).await.is_err());
    assert_eq!(std::fs::read(&outside).unwrap(), b"untouched");
}

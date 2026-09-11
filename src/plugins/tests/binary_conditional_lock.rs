use super::binary_conditional_support::*;
use crate::paths::SaiPaths;
use serde_json::json;

/// 【条件文件测试】【协调锁保护】普通及条件写入都不能原子替换应用自己的锁文件
/// @returns 无；锁文件不变，随后普通输出仍可获得互斥
#[tokio::test]
async fn plugin_output_cannot_replace_its_own_coordination_lock() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    std::fs::create_dir_all(&paths.state_dir).unwrap();
    let lock = paths.state_dir.join("plugin-binary-write.lock");
    std::fs::write(&lock, b"lock marker").unwrap();
    let plugin = runtime(root.path(), &["state"], &["state"]);
    for ordinary in [true, false] {
        let error=plugin.call_tool("run",json!({"path":"state/plugin-binary-write.lock","ordinary":ordinary,"expected":digest(b"lock marker")}),context(root.path())).await.unwrap_err();
        assert!(
            format!("{error:#}").contains("coordination lock"),
            "{error:#}"
        );
        assert_eq!(std::fs::read(&lock).unwrap(), b"lock marker");
    }
    assert_eq!(
        plugin
            .call_tool("run", json!({"path":"state/data"}), context(root.path()))
            .await
            .unwrap(),
        "true"
    );
}

/// 【条件文件测试】【锁对象类型】固定锁路径不能是链接或特殊文件，错误不能触碰输出
/// @returns 无；链接目标保持原样，管道不等待另一端
#[cfg(unix)]
#[tokio::test]
async fn coordination_lock_rejects_links_fifos_and_directories() {
    for kind in ["symlink", "fifo", "directory"] {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        std::fs::create_dir_all(&paths.state_dir).unwrap();
        let lock = paths.state_dir.join("plugin-binary-write.lock");
        let keep = outside.path().join("keep");
        std::fs::write(&keep, b"keep").unwrap();
        match kind {
            "symlink" => std::os::unix::fs::symlink(&keep, &lock).unwrap(),
            "fifo" => {
                let name = std::ffi::CString::new(lock.to_str().unwrap()).unwrap();
                assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
            }
            _ => std::fs::create_dir(&lock).unwrap(),
        }
        let plugin = runtime(root.path(), &["output"], &["output"]);
        for ordinary in [true, false] {
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                plugin.call_tool(
                    "run",
                    json!({"path":"output/a","ordinary":ordinary}),
                    context(root.path()),
                ),
            )
            .await
            .unwrap();
            assert!(result.is_err(), "{kind}");
        }
        assert!(!root.path().join("output").exists());
        assert_eq!(std::fs::read(keep).unwrap(), b"keep");
    }
}

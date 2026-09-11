use super::file_removal_support::*;
use serde_json::json;

/// 【文件删除测试】【单文件行为】独立删除能力可以删除二进制和空文件，缺失目标返回 false
/// @returns 无；不要求正文读取权限，不创建缺失目标父目录
#[tokio::test]
async fn permanent_removal_is_idempotent_and_does_not_need_read_or_write_grants() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("output")).unwrap();
    let plugin = runtime(root.path(), json!({"remove_paths":["output"]}));
    for bytes in [b"".as_slice(), b"\0\xffraw"] {
        std::fs::write(root.path().join("output/a"), bytes).unwrap();
        assert_eq!(
            plugin
                .call_tool("run", json!({"path":"output/a"}), context(root.path()))
                .await
                .unwrap(),
            "true"
        );
        assert!(!root.path().join("output/a").exists());
        assert_eq!(
            plugin
                .call_tool("run", json!({"path":"output/a"}), context(root.path()))
                .await
                .unwrap(),
            "false"
        );
    }
    assert_eq!(
        plugin
            .call_tool(
                "run",
                json!({"path":"output/missing/child"}),
                context(root.path())
            )
            .await
            .unwrap(),
        "false"
    );
    assert!(!root.path().join("output/missing").exists());
}

/// 【文件删除测试】【范围与权限】拒绝授权根、相邻文件和只读调用，缺失路径也必须先授权
/// @returns 无；所有被拒绝目标保持原样
#[tokio::test]
async fn removal_checks_directory_scope_before_existence_or_mutation() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("output")).unwrap();
    std::fs::create_dir_all(root.path().join("outside")).unwrap();
    std::fs::write(root.path().join("outside/keep"), b"keep").unwrap();
    std::fs::write(root.path().join("output/keep"), b"keep").unwrap();
    let plugin = runtime(root.path(), json!({"remove_paths":["output"]}));
    for path in [
        "output",
        "outside/keep",
        "outside/missing/file",
        "output/../outside/keep",
    ] {
        assert!(
            plugin
                .call_tool("run", json!({"path":path}), context(root.path()))
                .await
                .is_err(),
            "{path}"
        );
    }
    let mut readonly = context(root.path());
    readonly.allow_writes = false;
    assert!(plugin
        .call_tool("run", json!({"path":"output/keep"}), readonly)
        .await
        .is_err());
    let file_grant = runtime(root.path(), json!({"remove_paths":["output/keep"]}));
    assert!(file_grant
        .call_tool("run", json!({"path":"output/keep"}), context(root.path()))
        .await
        .is_err());
    assert_eq!(
        std::fs::read(root.path().join("outside/keep")).unwrap(),
        b"keep"
    );
    assert_eq!(
        std::fs::read(root.path().join("output/keep")).unwrap(),
        b"keep"
    );
    assert!(!root.path().join("outside/missing").exists());
}

/// 【文件删除测试】【特殊对象】链接、目录与管道均拒绝，链接目标保持不变
/// @returns 无；检测不会打开管道或递归进入目录
#[cfg(unix)]
#[tokio::test]
async fn removal_rejects_links_directories_and_special_files_without_following_them() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("output/directory")).unwrap();
    std::fs::create_dir(root.path().join("outside")).unwrap();
    std::fs::write(root.path().join("output/keep"), b"keep").unwrap();
    std::fs::write(root.path().join("outside/keep"), b"outside").unwrap();
    for (name, target) in [
        ("inside-link", "output/keep"),
        ("escape", "outside/keep"),
        ("dangling", "outside/missing"),
        ("parent-link", "outside"),
    ] {
        std::os::unix::fs::symlink(
            root.path().join(target),
            root.path().join("output").join(name),
        )
        .unwrap();
    }
    let fifo = std::ffi::CString::new(root.path().join("output/fifo").to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    let plugin = runtime(
        root.path(),
        json!({"remove_paths":["output"],"trash_paths":["output"]}),
    );
    for operation in ["remove_file", "trash_file"] {
        for name in [
            "inside-link",
            "escape",
            "dangling",
            "parent-link/keep",
            "directory",
            "fifo",
        ] {
            assert!(
                tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    plugin.call_tool(
                        "run",
                        json!({"path":format!("output/{name}"),"operation":operation}),
                        context(root.path())
                    )
                )
                .await
                .unwrap()
                .is_err(),
                "{operation}: {name}"
            );
        }
    }
    assert_eq!(
        std::fs::read(root.path().join("output/keep")).unwrap(),
        b"keep"
    );
    assert_eq!(
        std::fs::read(root.path().join("outside/keep")).unwrap(),
        b"outside"
    );
    assert!(
        std::fs::symlink_metadata(root.path().join("output/dangling"))
            .unwrap()
            .is_symlink()
    );
}

/// 【文件删除测试】【锁对象保护】原锁名称与硬链接别名都不能删除或回收
/// @returns 无；锁文件和别名保留原内容，后续删除正常文件仍然成功
#[cfg(unix)]
#[tokio::test]
async fn file_removal_cannot_remove_the_coordination_lock_or_its_aliases() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("state")).unwrap();
    let lock = root.path().join("state/plugin-binary-write.lock");
    let alias = root.path().join("state/alias");
    std::fs::write(&lock, b"lock marker").unwrap();
    std::fs::hard_link(&lock, &alias).unwrap();
    let plugin = runtime(
        root.path(),
        json!({"remove_paths":["state"],"trash_paths":["state"]}),
    );
    for operation in ["remove_file", "trash_file"] {
        for path in ["state/plugin-binary-write.lock", "state/alias"] {
            let error = plugin
                .call_tool(
                    "run",
                    json!({"path":path,"operation":operation}),
                    context(root.path()),
                )
                .await
                .unwrap_err();
            assert!(
                format!("{error:#}").contains("coordination lock"),
                "{error:#}"
            );
        }
    }
    assert_eq!(std::fs::read(&lock).unwrap(), b"lock marker");
    assert_eq!(std::fs::read(&alias).unwrap(), b"lock marker");
    std::fs::write(root.path().join("state/data"), b"remove").unwrap();
    assert_eq!(
        plugin
            .call_tool("run", json!({"path":"state/data"}), context(root.path()))
            .await
            .unwrap(),
        "true"
    );
}

/// 【文件删除测试】【宿主复核】直接调用宿主也必须验证目录、操作类型与可信写入权限
/// @returns 无；拒绝不会创建状态锁或改变文件
#[tokio::test]
async fn file_removal_host_revalidates_grants_without_the_lua_binding() {
    use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
    use sai_plugin_runtime::{host::*, Capabilities};
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("output")).unwrap();
    std::fs::write(root.path().join("output/a"), b"keep").unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let host = PrivatePluginHost::new(&paths, "removal-host");
    for (kind, path, allowed, grants) in [
        (FileRemovalKind::Permanent, "output/a", true, json!({})),
        (
            FileRemovalKind::Trash,
            "output/a",
            true,
            json!({"remove_paths":["output"]}),
        ),
        (
            FileRemovalKind::Permanent,
            "output/a",
            false,
            json!({"remove_paths":["output"]}),
        ),
        (
            FileRemovalKind::Permanent,
            "output/a",
            true,
            json!({"remove_paths":["other"]}),
        ),
        (
            FileRemovalKind::Permanent,
            "../output/a",
            true,
            json!({"remove_paths":["output"]}),
        ),
    ] {
        let capabilities: Capabilities = serde_json::from_value(json!({"system":grants})).unwrap();
        assert!(host
            .remove_file(
                FileRemovalRequest {
                    path: path.into(),
                    kind
                },
                SystemContext {
                    workdir: root.path().to_string_lossy().into_owned(),
                    allow_writes: allowed
                },
                capabilities
            )
            .await
            .is_err());
    }
    assert_eq!(
        std::fs::read(root.path().join("output/a")).unwrap(),
        b"keep"
    );
    assert!(!paths.state_dir.exists());
}

/// 【文件删除测试】【无需正文权限】文件本身不可读取时，目录授权仍然允许解除单个名称
/// @returns 无；不打开或读取源文件正文
#[cfg(unix)]
#[tokio::test]
async fn file_removal_does_not_require_read_access_to_file_contents() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("output")).unwrap();
    let path = root.path().join("output/unreadable");
    std::fs::write(&path, b"private").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let plugin = runtime(root.path(), json!({"remove_paths":["output"]}));
    assert_eq!(
        plugin
            .call_tool(
                "run",
                json!({"path":"output/unreadable"}),
                context(root.path())
            )
            .await
            .unwrap(),
        "true"
    );
    assert!(!path.exists());
}

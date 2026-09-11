use super::binary_conditional_support::*;
use serde_json::json;

/// 【条件文件测试】【双路径范围】只有同时位于有效读取及写入范围内的目标才能比较
/// @returns 无；越界错误不创建目标目录，也不伪装为条件冲突
#[tokio::test]
async fn read_and_write_roots_are_both_checked_before_creating_any_output_directory() {
    let root = tempfile::tempdir().unwrap();
    for (reads, writes, path, expected_error) in [
        (
            vec!["readable"],
            vec!["output"],
            "output/nested/a",
            "read paths",
        ),
        (
            vec!["output"],
            vec!["writable"],
            "output/nested/a",
            "write paths",
        ),
        (
            vec!["output/a"],
            vec!["output"],
            "output/ab/file",
            "read paths",
        ),
        (
            vec!["output"],
            vec!["output/a"],
            "output/ab/file",
            "write paths",
        ),
    ] {
        let plugin = runtime(root.path(), &reads, &writes);
        for expected in [None, Some(digest(b""))] {
            let mut args = json!({"path":path});
            if let Some(expected) = expected {
                args["expected"] = json!(expected);
            }
            let error = plugin
                .call_tool("run", args, context(root.path()))
                .await
                .unwrap_err();
            assert!(format!("{error:#}").contains(expected_error), "{error:#}");
        }
    }
    assert!(!root.path().join("output").exists());
}

/// 【条件文件测试】【缺失单文件授权】精确文件读取声明允许创建该文件但不授予相邻文件权限
/// @returns 无；目录不存在时也按路径范围校验
#[tokio::test]
async fn missing_exact_read_grants_allow_only_the_requested_file() {
    let root = tempfile::tempdir().unwrap();
    let plugin = runtime(root.path(), &["output/index.json"], &["output"]);
    assert_eq!(
        plugin
            .call_tool(
                "run",
                json!({"path":"output/index.json"}),
                context(root.path())
            )
            .await
            .unwrap(),
        "true"
    );
    for path in ["output/other.json", "output/index.json/child"] {
        assert!(plugin
            .call_tool("run", json!({"path":path}), context(root.path()))
            .await
            .is_err());
    }
    assert_eq!(
        std::fs::read_dir(root.path().join("output"))
            .unwrap()
            .count(),
        1
    );
}

/// 【条件文件测试】【路径名称】父目录跳转和跨平台设备名称不能产生文件或目录
/// @returns 无；全部请求在输出目录创建前失败
#[tokio::test]
async fn nonportable_or_traversing_conditional_paths_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let plugin = runtime(root.path(), &["output"], &["output"]);
    for path in [
        "output/../escape",
        "output/a:stream",
        "output/con.json",
        "output/nul",
        "output/trailing./a",
        "output/a /b",
    ] {
        assert!(
            plugin
                .call_tool("run", json!({"path":path}), context(root.path()))
                .await
                .is_err(),
            "{path}"
        );
    }
    assert!(!root.path().join("output").exists());
}

/// 【条件文件测试】【对象类型】目录不能用不存在条件或摘要条件伪装成普通冲突
/// @returns 无；目标目录保持原状
#[tokio::test]
async fn directory_targets_are_errors_for_every_revision_condition() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("output/dir")).unwrap();
    let plugin = runtime(root.path(), &["output"], &["output"]);
    for expected in [None, Some(digest(b""))] {
        let mut args = json!({"path":"output/dir"});
        if let Some(expected) = expected {
            args["expected"] = json!(expected);
        }
        let error = plugin
            .call_tool("run", args, context(root.path()))
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("regular file"), "{error:#}");
    }
    assert!(root.path().join("output/dir").is_dir());
}

/// 【条件文件测试】【特殊对象与链接】管道、套接字和越界链接都不能参与条件写入
/// @returns 无；错误不修改链接目标且不会阻塞等待管道写入者
#[cfg(unix)]
#[tokio::test]
async fn special_files_and_escaping_links_are_rejected_without_touching_their_targets() {
    use std::os::unix::{fs::symlink, net::UnixListener};
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("output")).unwrap();
    let keep = outside.path().join("keep");
    std::fs::write(&keep, b"keep").unwrap();
    symlink(&keep, root.path().join("output/link")).unwrap();
    symlink(outside.path(), root.path().join("output/directory-link")).unwrap();
    symlink(
        outside.path().join("missing"),
        root.path().join("output/dangling"),
    )
    .unwrap();
    symlink("/dev/null", root.path().join("output/device")).unwrap();
    let fifo = root.path().join("output/fifo");
    let fifo_name = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo_name.as_ptr(), 0o600) }, 0);
    let _socket = UnixListener::bind(root.path().join("output/socket")).unwrap();
    let plugin = runtime(root.path(), &["output"], &["output"]);
    for name in [
        "fifo",
        "socket",
        "link",
        "directory-link/nested/a",
        "dangling",
        "device",
    ] {
        for expected in [None, Some(digest(b"keep"))] {
            let mut args = json!({"path":format!("output/{name}")});
            if let Some(expected) = expected {
                args["expected"] = json!(expected);
            }
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                plugin.call_tool("run", args, context(root.path())),
            )
            .await
            .unwrap();
            assert!(result.is_err(), "{name}");
        }
    }
    assert_eq!(std::fs::read(keep).unwrap(), b"keep");
    assert!(!outside.path().join("nested").exists());
    assert!(!outside.path().join("missing").exists());
}

/// 【条件文件测试】【授权内初始链接】初始链接解析到读写交集时更新真实目标，保留链接本身
/// @returns 无；授权外链接仍由独立越界场景拒绝
#[cfg(unix)]
#[tokio::test]
async fn initial_links_inside_both_grants_resolve_to_the_same_file_revision() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("output")).unwrap();
    std::fs::write(root.path().join("output/file"), b"old").unwrap();
    std::os::unix::fs::symlink("file", root.path().join("output/alias")).unwrap();
    let plugin = runtime(root.path(), &["output/alias"], &["output"]);
    assert_eq!(
        plugin
            .call_tool(
                "run",
                json!({"path":"output/alias","expected":digest(b"old")}),
                context(root.path())
            )
            .await
            .unwrap(),
        "true"
    );
    assert_eq!(
        std::fs::read(root.path().join("output/file")).unwrap(),
        b"new"
    );
    assert!(std::fs::symlink_metadata(root.path().join("output/alias"))
        .unwrap()
        .is_symlink());
}

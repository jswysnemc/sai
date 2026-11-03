//! 临时目录路径别名与补丁目标链接保护的跨平台回归测试。

use super::test_support;

/// 【文件补丁测试】【路径规范化】夹具使用真实目录路径，消除系统目录链接及短路径别名。
/// 参数: 无；返回: 无。
#[test]
fn temporary_workspace_uses_canonical_system_directory() {
    let root = test_support::tempdir();
    assert_eq!(root.path(), root.path().canonicalize().unwrap());
}

/// 【文件补丁测试】【Windows 父级跳转】规范化工作目录不能在路径拼接时吞掉协议中的父级跳转。
/// 参数: 无；返回: 无。
#[cfg(windows)]
#[tokio::test]
async fn verbatim_workspace_rejects_parent_traversal_before_path_joining() {
    use super::{ToolRegistry, json, register};

    let root = test_support::tempdir();
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let mut registry = ToolRegistry::new();
    register(&mut registry);
    crate::runtime_cwd::scope(workspace.clone(), async {
        for path in [
            "../escape".to_string(),
            "..\\escape".to_string(),
            "nested/../escape".to_string(),
            "nested\\..\\escape".to_string(),
            format!("{}\\..\\escape", workspace.display()),
            format!("{}\\nested\\..\\escape", workspace.display()),
        ] {
            let patch = format!("*** Begin Patch\n*** Add File: {path}\n+new\n*** End Patch");
            let error = registry
                .call("apply_patch", &json!({"patch": patch}).to_string())
                .await
                .unwrap_err();
            assert!(
                format!("{error:#}").contains("parent-directory traversal"),
                "{error:#}"
            );
        }
        assert!(!root.path().join("escape").exists());
        assert!(!workspace.join("escape").exists());
        assert!(!workspace.join("nested").exists());
    })
    .await;
}

/// 【文件补丁测试】【链接临时目录】通过含链接的系统目录创建工作区，同时拒绝补丁目标链接。
/// 参数: 无；返回: 无。
#[cfg(unix)]
#[tokio::test]
async fn linked_temporary_base_allows_patches_but_rejects_target_symlinks() {
    use super::{ToolRegistry, json, register};

    let base = std::env::temp_dir().canonicalize().unwrap();
    let outer = tempfile::tempdir_in(base).unwrap();
    let actual = outer.path().join("actual");
    let alias = outer.path().join("alias");
    std::fs::create_dir(&actual).unwrap();
    std::os::unix::fs::symlink(&actual, &alias).unwrap();
    let root = test_support::tempdir_in(&alias);
    let mut registry = ToolRegistry::new();
    register(&mut registry);

    crate::runtime_cwd::scope(root.path().into(), async {
        // 1. 【文件补丁测试】【真实调用】补齐父目录的新增操作不能被系统临时目录链接阻断
        let patch = "*** Begin Patch\n*** Add File: nested/new\n+content\n*** End Patch";
        registry
            .call("apply_patch", &json!({"patch": patch}).to_string())
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(root.path().join("nested/new")).unwrap(),
            "content\n"
        );
        assert_eq!(root.path(), root.path().canonicalize().unwrap());

        // 2. 【文件补丁测试】【目标保护】工作区内的父目录链接和文件链接仍然拒绝访问
        std::fs::write(actual.join("outside"), "old\n").unwrap();
        std::os::unix::fs::symlink(&actual, root.path().join("link")).unwrap();
        std::os::unix::fs::symlink(actual.join("outside"), root.path().join("file-link")).unwrap();
        for (path, expected) in [
            ("link/outside", "patch parent must not be a symlink"),
            (
                "file-link",
                "patch target is not a regular non-symlink file",
            ),
        ] {
            let patch =
                format!("*** Begin Patch\n*** Update File: {path}\n@@\n-old\n+new\n*** End Patch");
            let error = registry
                .call("apply_patch", &json!({"patch": patch}).to_string())
                .await
                .unwrap_err();
            assert!(format!("{error:#}").contains(expected), "{error:#}");
        }
        assert_eq!(
            std::fs::read_to_string(actual.join("outside")).unwrap(),
            "old\n"
        );
    })
    .await;
}

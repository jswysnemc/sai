//! 从真实文件和工具注册入口验证补丁的提交、恢复和并发行为。

use super::*;
use std::sync::atomic::AtomicBool;

/// 【文件补丁测试】【调用夹具】在临时工作目录中通过注册表执行真实工具。
/// 参数: root 为目录，patch 为协议文本；返回: 工具结果。
async fn call(root: &std::path::Path, patch: &str) -> Result<String> {
    let mut registry = ToolRegistry::new();
    register(&mut registry);
    crate::runtime_cwd::scope(
        root.into(),
        registry.call("apply_patch", &json!({"patch":patch}).to_string()),
    )
    .await
}

/// 【文件补丁测试】【批量操作】一次调用完成新增、修改、移动和删除，结果可供界面复用。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn applies_all_operations_and_emits_actual_diff() {
    let root = test_support::tempdir();
    std::fs::write(root.path().join("a"), "one\ntwo\n").unwrap();
    std::fs::write(root.path().join("remove"), "old\n").unwrap();
    let output = call(root.path(), "*** Begin Patch\n*** Add File: nested/new\n+new\n*** Update File: a\n*** Move to: moved\n@@\n-two\n+TWO\n*** Delete File: remove\n*** End Patch").await.unwrap();
    assert_eq!(
        std::fs::read_to_string(root.path().join("moved")).unwrap(),
        "one\nTWO\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("nested/new")).unwrap(),
        "new\n"
    );
    assert!(!root.path().join("a").exists() && !root.path().join("remove").exists());
    let output: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(output["changed_files"].as_array().unwrap().len(), 3);
    assert_eq!(output["changed_files"][1]["action"], "Renamed");
    assert!(output["diff"].as_str().unwrap().contains("+TWO"));
}

/// 【文件补丁测试】【预检隔离】后续片段校验失败时，前面的新增和修改均不得落盘。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn validation_failure_does_not_write_any_files_or_directories() {
    let root = test_support::tempdir();
    std::fs::write(root.path().join("a"), "one\n").unwrap();
    for patch in [
        "*** Begin Patch\n*** Add File: nested/new\n+new\n*** Update File: a\n@@\n-missing\n+new\n*** End Patch",
        "*** Begin Patch\n*** Update File: a\n*** Move to: a\n*** End Patch",
        "*** Begin Patch\n*** Add File: a\n+overwrite\n*** End Patch",
        "*** Begin Patch\n*** Add File: ../escape\n+x\n*** End Patch",
    ] {
        assert!(call(root.path(), patch).await.is_err());
        assert_eq!(
            std::fs::read_to_string(root.path().join("a")).unwrap(),
            "one\n"
        );
        assert!(!root.path().join("nested").exists());
    }
}

/// 【文件补丁测试】【故障恢复】后续提交失败时恢复已完成写入，提交前外部修改则直接拒绝。
/// 参数: 无；返回: 无。
#[test]
fn rolls_back_commit_failure_and_rejects_stale_snapshots() {
    let root = test_support::tempdir();
    std::fs::write(root.path().join("a"), "a\n").unwrap();
    std::fs::write(root.path().join("b"), "b\n").unwrap();
    crate::runtime_cwd::with_directory(root.path().into(), || {
        let operations = parser::parse("*** Begin Patch\n*** Update File: a\n@@\n-a\n+A\n*** Update File: b\n@@\n-b\n+B\n*** End Patch").unwrap();
        let mut plan = prepare::prepare(&operations).unwrap();
        let error = transaction::commit_with(&mut plan, &AtomicBool::new(false), |index| {
            anyhow::ensure!(index != 1, "injected failure");
            Ok(())
        })
        .unwrap_err();
        assert!(error.to_string().contains("rolled back"));
        assert_eq!(
            std::fs::read_to_string(root.path().join("a")).unwrap(),
            "a\n"
        );
    });
    // 1. 【文件补丁测试】【线程目录】文件系统断言使用绝对路径，运行期目录不修改进程目录
    crate::runtime_cwd::with_directory(root.path().into(), || {
        let operations =
            parser::parse("*** Begin Patch\n*** Update File: a\n@@\n-a\n+A\n*** End Patch")
                .unwrap();
        let mut plan = prepare::prepare(&operations).unwrap();
        std::fs::write(root.path().join("a"), "external\n").unwrap();
        assert!(transaction::commit(&mut plan, &AtomicBool::new(false)).is_err());
        assert_eq!(
            std::fs::read_to_string(root.path().join("a")).unwrap(),
            "external\n"
        );
    });
}

/// 【文件补丁测试】【并发更新】两个不同片段的补丁按锁顺序读取最新内容，不丢失彼此修改。
/// 参数: 无；返回: 无。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_patches_preserve_non_overlapping_updates() {
    let root = test_support::tempdir();
    std::fs::write(root.path().join("a"), "first\nsecond\n").unwrap();
    let (first, second) = tokio::join!(
        call(
            root.path(),
            "*** Begin Patch\n*** Update File: a\n@@\n-first\n+FIRST\n*** End Patch"
        ),
        call(
            root.path(),
            "*** Begin Patch\n*** Update File: a\n@@\n-second\n+SECOND\n*** End Patch"
        )
    );
    first.unwrap();
    second.unwrap();
    assert_eq!(
        std::fs::read_to_string(root.path().join("a")).unwrap(),
        "FIRST\nSECOND\n"
    );
}

/// 【文件补丁测试】【链接边界】符号链接和父目录链接不得把补丁导向其他目标。
/// 参数: 无；返回: 无。
#[cfg(unix)]
#[tokio::test]
async fn rejects_symlink_and_hardlink_aliases() {
    let root = test_support::tempdir();
    let outside = test_support::tempdir();
    std::fs::write(outside.path().join("a"), "old\n").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
    assert!(
        call(
            root.path(),
            "*** Begin Patch\n*** Update File: link/a\n@@\n-old\n+new\n*** End Patch"
        )
        .await
        .is_err()
    );
    std::fs::hard_link(outside.path().join("a"), root.path().join("hard")).unwrap();
    assert!(
        call(
            root.path(),
            "*** Begin Patch\n*** Update File: hard\n@@\n-old\n+new\n*** End Patch"
        )
        .await
        .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(outside.path().join("a")).unwrap(),
        "old\n"
    );
}

/// 【文件补丁测试】【恢复冲突】恢复过程不能覆盖外部编辑，错误必须指出需要人工检查的文件。
/// 参数: 无；返回: 无。
#[test]
fn rollback_preserves_external_edits_and_reports_incomplete_recovery() {
    let root = test_support::tempdir();
    for name in ["a", "b"] {
        std::fs::write(root.path().join(name), "old\n").unwrap();
    }
    crate::runtime_cwd::with_directory(root.path().into(), || {
        let patch = "*** Begin Patch\n*** Update File: a\n@@\n-old\n+new\n*** Update File: b\n@@\n-old\n+new\n*** End Patch";
        let mut plan = prepare::prepare(&parser::parse(patch).unwrap()).unwrap();
        let recovery_path = plan.files[0].target.path.display().to_string();
        let result = transaction::commit_with(&mut plan, &AtomicBool::new(false), |index| {
            if index == 1 {
                std::fs::write(root.path().join("a"), "external\n")?;
                anyhow::bail!("injected failure");
            }
            Ok(())
        });
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("ROLLBACK INCOMPLETE")
                && error.contains(&format!("{recovery_path}: file edit conflict:")),
            "{error}"
        );
        assert_eq!(
            std::fs::read_to_string(root.path().join("a")).unwrap(),
            "external\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.path().join("b")).unwrap(),
            "old\n"
        );
    });
}

/// 【文件补丁测试】【取消与权限】提交前取消不创建目标目录，更新和重命名保留可执行权限。
/// 参数: 无；返回: 无。
#[test]
fn cancellation_prevents_writes_and_updates_preserve_permissions() {
    let root = test_support::tempdir();
    crate::runtime_cwd::with_directory(root.path().into(), || {
        let operations =
            parser::parse("*** Begin Patch\n*** Add File: nested/new\n+new\n*** End Patch")
                .unwrap();
        let mut plan = prepare::prepare(&operations).unwrap();
        assert!(transaction::commit(&mut plan, &AtomicBool::new(true)).is_err());
        assert!(!root.path().join("nested").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(root.path().join("script"), "old\n").unwrap();
            std::fs::set_permissions(
                root.path().join("script"),
                std::fs::Permissions::from_mode(0o751),
            )
            .unwrap();
            let operations = parser::parse("*** Begin Patch\n*** Update File: script\n*** Move to: moved\n@@\n-old\n+new\n*** End Patch").unwrap();
            let mut plan = prepare::prepare(&operations).unwrap();
            transaction::commit(&mut plan, &AtomicBool::new(false)).unwrap();
            assert_eq!(
                std::fs::metadata(root.path().join("moved"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o751
            );
        }
    });
}

/// 【文件补丁测试】【锁顺序】同一文件重复加锁会有界失败，目标输入顺序不同也不会死锁。
/// 参数: 无；返回: 无。
#[test]
fn lock_wait_is_bounded_and_target_order_is_normalized() {
    let root = test_support::tempdir();
    let a = root.path().join("a");
    let b = root.path().join("b");
    let held = FileEditLocks::acquire(&[a.clone(), b.clone()]).unwrap();
    assert!(
        FileEditLocks::acquire_for(
            &[b.clone(), a.clone()],
            std::time::Duration::from_millis(20)
        )
        .is_err()
    );
    drop(held);
    assert!(FileEditLocks::acquire_for(&[b, a], std::time::Duration::from_millis(20)).is_ok());
}

/// 【文件补丁测试】【清理边界】祖先目录在提交前被替换为链接时，恢复清理不访问链接目标。
/// 参数: 无；返回: 无。
#[cfg(unix)]
#[test]
fn directory_cleanup_does_not_follow_a_replaced_ancestor() {
    let root = test_support::tempdir();
    let outside = test_support::tempdir();
    std::fs::create_dir(outside.path().join("deeper")).unwrap();
    crate::runtime_cwd::with_directory(root.path().into(), || {
        let operations = parser::parse(
            "*** Begin Patch\n*** Add File: nested/deeper/new\n+content\n*** End Patch",
        )
        .unwrap();
        let mut plan = prepare::prepare(&operations).unwrap();
        let error = transaction::commit_with(&mut plan, &AtomicBool::new(false), |_| {
            std::fs::rename(root.path().join("nested"), root.path().join("detached"))?;
            std::os::unix::fs::symlink(outside.path(), root.path().join("nested"))?;
            anyhow::bail!("injected ancestor replacement")
        })
        .unwrap_err();
        assert!(error.to_string().contains("rolled back"));
        assert!(outside.path().join("deeper").is_dir());
        assert!(!root.path().join("detached/deeper").exists());
        assert!(root.path().join("nested").is_symlink());
    });
}

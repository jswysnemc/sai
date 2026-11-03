//! 验证基础工具可见性、审批、现有编辑工具锁和终端差异视图。

use super::*;
use crate::tools::ToolPermission;

/// 【文件补丁测试】【基础工具】直接注册并默认暴露，规划模式拒绝写入，审计模式等待批准。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn base_tool_is_registered_and_uses_existing_permission_policy() {
    let root = test_support::tempdir();
    let mut registry = ToolRegistry::new();
    crate::tools::default_tools::register(&mut registry);
    assert!(registry.contains("apply_patch"));
    assert!(crate::tools::groups::is_base_tool("apply_patch"));
    assert!(!crate::tools::progressive::is_deferred_tool(
        "apply_patch",
        &[crate::config::DEFERRED_ALL_NON_BASE.into()]
    ));
    let args = json!({"patch": format!("*** Begin Patch\n*** Add File: {}\n+new\n*** End Patch", root.path().join("a").display())}).to_string();
    assert_eq!(
        registry.permission_for_call("apply_patch", &args).unwrap(),
        ToolPermission::Writes
    );
    registry.set_permission_profile(crate::permission::PermissionProfile::new(
        crate::permission::PermissionProfileMode::Plan,
        root.path().into(),
        None,
    ));
    assert!(registry.call("apply_patch", &args).await.is_err());
    registry.set_permission_profile(crate::permission::PermissionProfile::new(
        crate::permission::PermissionProfileMode::Audited,
        root.path().into(),
        None,
    ));
    assert!(registry.requires_permission("apply_patch", &args).unwrap());
    assert!(!root.path().join("a").exists());
    registry
        .record_permission_approved("apply_patch", &args, None)
        .unwrap();
    registry.call("apply_patch", &args).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(root.path().join("a")).unwrap(),
        "new\n"
    );
}

/// 【文件补丁测试】【共享互斥】已有替换工具必须等待补丁使用的同一把锁，不提前读取旧内容。
/// 参数: 无；返回: 无。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_editor_waits_for_the_shared_patch_lock() {
    let root = test_support::tempdir();
    let path = root.path().join("a");
    std::fs::write(&path, "first\n").unwrap();
    let guard = FileEditLocks::acquire(std::slice::from_ref(&path)).unwrap();
    let mut registry = ToolRegistry::new();
    crate::tools::default_tools::register(&mut registry);
    let args = json!({"path":path,"old_string":"second","new_string":"third"}).to_string();
    let pending = tokio::spawn(async move { registry.call("str_replace", &args).await });
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert!(!pending.is_finished());
    std::fs::write(&path, "second\n").unwrap();
    drop(guard);
    pending.await.unwrap().unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "third\n");
}

/// 【文件补丁测试】【等待取消】旧写入工具等待补丁锁时被取消，释放锁后不能发生延迟写入。
/// 参数: 无；返回: 无。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_legacy_writer_does_not_write_after_the_lock_is_released() {
    let root = test_support::tempdir();
    let path = root.path().join("a");
    std::fs::write(&path, "kept\n").unwrap();
    let guard = FileEditLocks::acquire(std::slice::from_ref(&path)).unwrap();
    let mut registry = ToolRegistry::new();
    crate::tools::default_tools::register(&mut registry);
    let args = json!({"path":path, "content":"cancelled\n"}).to_string();
    let pending = tokio::spawn(async move { registry.call("write_file", &args).await });
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    drop(guard);
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert_eq!(std::fs::read_to_string(path).unwrap(), "kept\n");
}

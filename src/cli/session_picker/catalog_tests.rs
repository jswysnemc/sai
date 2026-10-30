use super::Catalog;
use crate::{paths::SaiPaths, state};
use std::sync::mpsc;

/// 【会话选择】【加载测试】创建规范化的双工作区和同名会话。
/// 参数: 无；返回: 临时目录守卫、应用路径与两个工作区目录
fn fixture() -> (
    tempfile::TempDir,
    SaiPaths,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let guard = tempfile::tempdir().unwrap();
    let root = crate::platform::windows_path::canonicalize(guard.path()).unwrap();
    let paths = SaiPaths::for_tests(&root);
    let a = root.join("workspace-a");
    let b = root.join("workspace-b");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    state::ensure_workspace_session(&paths, &a, "shared", "Local session").unwrap();
    state::ensure_workspace_session(&paths, &b, "shared", "Foreign session").unwrap();
    (guard, paths, a, b)
}

/// 【会话选择】【首屏回归】初始列表只读取当前工作区索引，且不写入任何会话数据。
/// 参数: 无；返回: 无
#[test]
fn initial_catalog_reads_only_one_index() {
    let (_guard, paths, a, _) = fixture();
    let before = state::session_index_io_counts();
    let (catalog, picker) = Catalog::new(&paths, &a, false).unwrap();
    let after = state::session_index_io_counts();
    assert_eq!(after.0 - before.0, 1);
    assert_eq!(after.1, before.1);
    assert_eq!(picker.targets.len(), 1);
    assert_eq!(picker.target().unwrap().workspace_path.as_ref(), Some(&a));
    assert!(!picker.loading);
    assert!(catalog.pending.is_none());
}

/// 【会话选择】【精确选择回归】后台结果重排时以工作区和会话 ID 保留选择，而非旧下标。
/// 参数: 无；返回: 无
#[test]
fn completed_catalog_preserves_exact_selection_and_scope() {
    let (_guard, paths, a, b) = fixture();
    let (mut catalog, mut picker) = Catalog::new(&paths, &b, false).unwrap();
    let targets = state::resume_catalog(&paths, &b, true).unwrap();
    let (sender, receiver) = mpsc::channel();
    catalog.pending = Some(receiver);
    picker.loading = true;
    picker.toggle();
    picker.query = "session".into();
    picker.filter();
    sender.send(Ok(targets)).unwrap();
    assert!(catalog.poll(&mut picker));
    assert_eq!(picker.query, "session");
    assert!(picker.all);
    assert!(!picker.loading);
    assert_eq!(picker.visible.len(), 2);
    assert_eq!(picker.target().unwrap().workspace_path.as_ref(), Some(&b));
    assert_eq!(picker.selected, 1);
    picker.toggle();
    assert_eq!(picker.visible.len(), 1);
    assert_eq!(picker.target().unwrap().workspace_path.as_ref(), Some(&b));
    assert_ne!(picker.target().unwrap().workspace_path.as_ref(), Some(&a));
}

/// 【会话选择】【搜索回归】结果到达前输入的搜索仍用于筛选新增工作区。
/// 参数: 无；返回: 无
#[test]
fn completed_catalog_keeps_query_entered_while_loading() {
    let (_guard, paths, a, b) = fixture();
    let (mut catalog, mut picker) = Catalog::new(&paths, &a, true).unwrap();
    let (sender, receiver) = mpsc::channel();
    catalog.pending = Some(receiver);
    picker.loading = true;
    picker.query = "Foreign".into();
    picker.filter();
    assert!(picker.target().is_none());
    sender
        .send(state::resume_catalog(&paths, &a, true))
        .unwrap();
    assert!(catalog.poll(&mut picker));
    assert_eq!(picker.visible.len(), 1);
    assert_eq!(picker.target().unwrap().workspace_path.as_ref(), Some(&b));
}

/// 【会话选择】【范围回归】加载期间切回当前工作区，结果到达后不强制打开全部范围。
/// 参数: 无；返回: 无
#[test]
fn completed_catalog_keeps_scope_changed_while_loading() {
    let (_guard, paths, a, _) = fixture();
    let (mut catalog, mut picker) = Catalog::new(&paths, &a, true).unwrap();
    let (sender, receiver) = mpsc::channel();
    catalog.pending = Some(receiver);
    picker.loading = true;
    picker.toggle();
    sender
        .send(state::resume_catalog(&paths, &a, true))
        .unwrap();
    assert!(catalog.poll(&mut picker));
    assert!(!picker.all);
    assert_eq!(picker.visible.len(), 1);
    assert_eq!(picker.target().unwrap().workspace_path.as_ref(), Some(&a));
}

/// 【会话选择】【后台失败回归】加载错误保留当前列表，后台断开也结束加载状态。
/// 参数: 无；返回: 无
#[test]
fn load_failure_or_disconnect_keeps_local_catalog_usable() {
    let (_guard, paths, a, _) = fixture();
    let (mut catalog, mut picker) = Catalog::new(&paths, &a, true).unwrap();
    let (sender, receiver) = mpsc::channel();
    catalog.pending = Some(receiver);
    picker.loading = true;
    sender.send(Err(anyhow::anyhow!("broken index"))).unwrap();
    assert!(catalog.poll(&mut picker));
    assert!(picker.load_error.as_ref().unwrap().contains("broken index"));
    assert!(!picker.loading);
    assert_eq!(picker.targets.len(), 1);
    let (sender, receiver) = mpsc::channel();
    catalog.pending = Some(receiver);
    picker.loading = true;
    assert!(!catalog.poll(&mut picker));
    drop(sender);
    assert!(catalog.poll(&mut picker));
    assert!(!picker.loading);
    picker.toggle();
    assert_eq!(picker.target().unwrap().workspace_path.as_ref(), Some(&a));
}

/// 【会话选择】【结果缓存回归】读取完成后反复切换范围不会重复读取索引。
/// 参数: 无；返回: 无
#[test]
fn ready_catalog_does_not_spawn_another_load() {
    let (_guard, paths, a, _) = fixture();
    let (mut catalog, mut picker) = Catalog::new(&paths, &a, true).unwrap();
    let (sender, receiver) = mpsc::channel();
    catalog.pending = Some(receiver);
    sender
        .send(state::resume_catalog(&paths, &a, true))
        .unwrap();
    assert!(catalog.poll(&mut picker));
    let before = state::session_index_io_counts();
    picker.toggle();
    picker.toggle();
    catalog.load_all(&mut picker).unwrap();
    assert!(catalog.pending.is_none());
    assert!(!picker.loading);
    assert_eq!(state::session_index_io_counts(), before);
}

/// 【会话选择】【重复切换回归】已有后台读取时切换范围不启动第二次读取。
/// 参数: 无；返回: 无
#[test]
fn pending_catalog_reuses_the_same_receiver() {
    let (_guard, paths, a, _) = fixture();
    let (mut catalog, mut picker) = Catalog::new(&paths, &a, false).unwrap();
    let (sender, receiver) = mpsc::channel();
    catalog.pending = Some(receiver);
    catalog.load_all(&mut picker).unwrap();
    sender.send(Ok(Vec::new())).unwrap();
    assert!(catalog.poll(&mut picker));
    assert!(picker.targets.is_empty());
}

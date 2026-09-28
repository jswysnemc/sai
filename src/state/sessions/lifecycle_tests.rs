use super::*;
use crate::paths::SaiPaths;

/// 【会话管理】【空状态】读取空工作区不应生成任何默认或草稿会话。
/// @returns 无；使用隔离目录，无外部参数
#[test]
fn empty_workspace_reads_do_not_create_sessions() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    assert!(list_sessions_for_workspace(&paths, &workspace)
        .unwrap()
        .is_empty());
    assert!(list_located_sessions_for_workspace(&paths, &workspace)
        .unwrap()
        .is_empty());
    assert!(active_session_id_for_workspace(&paths, &workspace)
        .unwrap()
        .is_empty());
    assert!(list_all_sessions(&paths).unwrap().is_empty());
}

/// 【会话管理】【完全清空】删除最后一条后，反复读取仍为空且显式创建可以恢复使用。
/// @returns 无；使用隔离工作区，无外部参数
#[test]
fn deleting_last_session_stays_empty_until_explicit_creation() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let session = create_session_for_workspace(&paths, &workspace, Some("First")).unwrap();
    let ids = list_sessions_for_workspace(&paths, &workspace)
        .unwrap()
        .into_iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    assert!(!ids.is_empty());
    assert_eq!(
        delete_sessions_for_workspace(&paths, &workspace, &ids)
            .unwrap()
            .len(),
        ids.len()
    );
    for _ in 0..3 {
        assert!(list_sessions_for_workspace(&paths, &workspace)
            .unwrap()
            .is_empty());
        assert!(list_located_sessions_for_workspace(&paths, &workspace)
            .unwrap()
            .is_empty());
        assert!(active_session_id_for_workspace(&paths, &workspace)
            .unwrap()
            .is_empty());
    }
    assert!(state_dir_for_workspace_session(&paths, &workspace, &session.id).is_err());
    let next = create_session_for_workspace(&paths, &workspace, Some("Next")).unwrap();
    assert_ne!(next.id, "default");
    assert_eq!(
        list_sessions_for_workspace(&paths, &workspace)
            .unwrap()
            .len(),
        1
    );
}

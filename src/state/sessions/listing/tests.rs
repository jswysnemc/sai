use super::*;
use crate::paths::SaiPaths;

/// 【会话载入】【外部更新】每次列表反映最新索引，普通读取不产生会话目录或索引写入。
/// @returns 无；无参数
#[test]
fn lists_fresh_metadata_without_rewriting_or_creating_session_directories() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    let scope = workspace_scope_for_path(&paths, &workspace);
    let first = list_located_sessions_for_workspace(&paths, &workspace).unwrap();
    assert_eq!(first.len(), 1);
    assert!(first[0].is_current);
    assert!(!first[0].state_dir.exists());
    let mut info = first[0].info.clone();
    info.title = "Updated elsewhere".into();
    let bytes = serde_json::to_vec(&[info]).unwrap();
    std::fs::write(scope.state_dir.join("index.json"), &bytes).unwrap();
    let before = super::super::index::session_index_io_counts();
    let refreshed = list_located_sessions_for_workspace(&paths, &workspace).unwrap();
    let after = super::super::index::session_index_io_counts();
    assert_eq!(refreshed[0].info.title, "Updated elsewhere");
    assert_eq!((after.0 - before.0, after.1 - before.1), (1, 0));
    assert_eq!(
        std::fs::read(scope.state_dir.join("index.json")).unwrap(),
        bytes
    );
}

/// 【会话载入】【工作区隔离】不同工作区的同名会话必须绑定各自目录。
/// @returns 无；无参数
#[test]
fn same_named_sessions_keep_their_workspace_identity() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let first = root.path().join("first");
    let second = root.path().join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    let left = list_located_sessions_for_workspace(&paths, &first).unwrap();
    let right = list_located_sessions_for_workspace(&paths, &second).unwrap();
    assert_eq!(left[0].info.id, right[0].info.id);
    assert_ne!(left[0].state_dir, right[0].state_dir);
    assert_ne!(left[0].workspace_id, right[0].workspace_id);
}

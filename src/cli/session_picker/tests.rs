use super::{model::Picker, rows};
use crate::{paths::SaiPaths, state};

/// 【会话恢复】【测试目录】创建规范化后的临时根目录。
///
/// 会话存储按规范路径登记工作区：macOS 临时目录经 /var -> /private/var 软链接，
/// Windows 可能给出 8.3 短名，不规范化时断言会拿别名与存储路径比较。
///
/// 返回:
/// - 临时目录守卫与规范根路径
fn temp_root() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = crate::platform::windows_path::canonicalize(dir.path()).unwrap();
    (dir, root)
}

/// 【会话恢复】【测试数据】创建不同工作区的同名会话。
/// 参数: paths 为隔离存储，root 为测试根目录；返回: 两个工作区目录
fn workspaces(
    paths: &SaiPaths,
    root: &std::path::Path,
) -> (std::path::PathBuf, std::path::PathBuf) {
    let a = root.join("workspace-a");
    let b = root.join("workspace-b");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    state::ensure_workspace_session(paths, &a, "shared", "Local session").unwrap();
    state::ensure_workspace_session(paths, &b, "shared", "Foreign session").unwrap();
    (a, b)
}

/// 【会话恢复】【范围回归】默认只显示当前目录，切换保留搜索且不能混淆同名会话。
/// 参数: 无；返回: 无
#[test]
fn scope_search_and_grouping_keep_workspace_identity() {
    let (_guard, root) = temp_root();
    let paths = SaiPaths::for_tests(&root);
    let (a, b) = workspaces(&paths, &root);
    let targets = state::resume_catalog(&paths, &a, true).unwrap();
    let current =
        state::workspace_id_for_path(&crate::platform::windows_path::canonicalize(&a).unwrap());
    let mut picker = Picker::new(targets, current, false);
    assert_eq!(picker.visible.len(), 1);
    assert_eq!(picker.target().unwrap().workspace_path.as_ref(), Some(&a));
    picker.toggle();
    assert_eq!(picker.visible.len(), 2);
    let (lines, _) = rows::body(&picker, 80);
    assert_eq!(lines.len(), 4);
    assert!(lines[0].contains("workspace-a"));
    assert!(lines[2].contains("workspace-b"));
    picker.query = "Foreign".into();
    picker.filter();
    assert_eq!(picker.target().unwrap().workspace_path.as_ref(), Some(&b));
    picker.toggle();
    assert!(picker.target().is_none());
    picker.toggle();
    assert_eq!(picker.target().unwrap().session.info.id, "shared");
}

/// 【会话恢复】【精确解析】显式目录可选跨区同名会话，不改变任何活动指针。
/// 参数: 无；返回: 无
#[tokio::test]
async fn explicit_workspace_and_missing_directory_are_validated() {
    let (_guard, root) = temp_root();
    let paths = SaiPaths::for_tests(&root);
    let (a, b) = workspaces(&paths, &root);
    crate::runtime_cwd::scope(a.clone(), async {
        let local = state::resolve_resume_target(&paths, "shared", None).unwrap();
        assert_eq!(local.workspace_path, Some(a));
        let foreign = state::resolve_resume_target(&paths, "shared", Some(&b)).unwrap();
        assert_eq!(foreign.directory(&paths).unwrap(), b);
        std::fs::remove_dir(&b).unwrap();
        assert!(foreign.directory(&paths).is_err());
    })
    .await;
}

/// 【会话恢复】【旧索引兼容】路径丢失时保留会话，通过 Web 注册表可补全路径。
/// 参数: 无；返回: 无
#[test]
fn legacy_web_workspace_paths_are_resolved_without_writes() {
    let (_guard, root) = temp_root();
    let paths = SaiPaths::for_tests(&root);
    let (a, b) = workspaces(&paths, &root);
    let foreign = state::resume_catalog(&paths, &a, true)
        .unwrap()
        .into_iter()
        .find(|item| item.workspace_path.as_ref() == Some(&b))
        .unwrap();
    let base = foreign
        .session
        .state_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    std::fs::remove_file(base.join("workspace.json")).unwrap();
    let unknown = state::resume_catalog(&paths, &a, true).unwrap();
    assert!(unknown.iter().any(|item| item.workspace_path.is_none()));
    std::fs::create_dir_all(paths.state_dir.join("web")).unwrap();
    std::fs::write(
        paths.state_dir.join("web/workspaces.json"),
        serde_json::to_vec(&serde_json::json!({"workspaces": [{"path": b}]})).unwrap(),
    )
    .unwrap();
    let recovered = state::resume_catalog(&paths, &a, true).unwrap();
    assert!(recovered.iter().all(|item| item.workspace_path.is_some()));
    assert!(!base.join("workspace.json").exists());
}

/// 【会话恢复】【歧义处理】当前工作区无匹配时不能随机恢复其他工作区同名会话。
/// 参数: 无；返回: 无
#[tokio::test]
async fn ambiguous_foreign_ids_require_a_workspace() {
    let (_guard, root) = temp_root();
    let paths = SaiPaths::for_tests(&root);
    workspaces(&paths, &root);
    crate::runtime_cwd::scope(root.clone(), async {
        assert!(state::resolve_resume_target(&paths, "shared", None)
            .unwrap_err()
            .to_string()
            .contains("ambiguous"));
        assert!(state::resume_catalog(&paths, &root, false)
            .unwrap()
            .is_empty());
    })
    .await;
}

/// 【会话恢复】【路径别名】符号链接目录必须使用与会话存储一致的规范标识。
/// 参数: 无；返回: 无
#[cfg(unix)]
#[test]
fn symbolic_link_uses_the_same_workspace_scope() {
    let (_guard, root) = temp_root();
    let paths = SaiPaths::for_tests(&root);
    let (a, _) = workspaces(&paths, &root);
    let alias = root.join("alias");
    std::os::unix::fs::symlink(&a, &alias).unwrap();
    let targets = state::resume_catalog(&paths, &alias, false).unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].directory(&paths).unwrap(), a);
}

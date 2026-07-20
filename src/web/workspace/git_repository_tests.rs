use super::*;
use std::path::Path;

/// 规范化路径字符串，便于跨平台比较。
fn path_key(path: impl AsRef<std::path::Path>) -> String {
    let path = path.as_ref();
    let canonical = crate::platform::windows_path::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf());
    let simplified = crate::platform::windows_path::simplified(&canonical);
    let value = simplified.display().to_string();
    #[cfg(windows)]
    {
        return value.replace('/', "\\").to_ascii_lowercase();
    }
    #[cfg(not(windows))]
    {
        value
    }
}

/// 创建用于仓库发现测试的最小 Git 仓库。
///
/// 参数:
/// - `root`: 待初始化目录
///
/// 返回:
/// - 无
async fn init_repository(root: &Path) {
    tokio::fs::create_dir_all(root).await.unwrap();
    git_success(root, &["init", "-b", "main"]).await.unwrap();
    git_success(root, &["config", "user.name", "Sai Test"])
        .await
        .unwrap();
    git_success(root, &["config", "user.email", "sai@example.com"])
        .await
        .unwrap();
    git_success(root, &["config", "core.autocrlf", "false"])
        .await
        .unwrap();
    tokio::fs::write(root.join("README.md"), "repository\n")
        .await
        .unwrap();
    git_success(root, &["add", "README.md"]).await.unwrap();
    git_success(root, &["commit", "-m", "initial"])
        .await
        .unwrap();
}

/// 验证工作区可发现多个仓库，并允许访问仓库关联的外部 worktree。
#[tokio::test]
async fn discovers_repositories_and_manages_worktrees() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let first = workspace.join("packages/first");
    let second = workspace.join("services/second");
    let worktree = temp.path().join("first-feature");
    let nested_worktree = workspace.join("first-nested-worktree");
    init_repository(&first).await;
    init_repository(&second).await;

    let created = git_op(
        &first,
        GitOperationRequest {
            action: "worktree_add",
            worktree_path: Some(worktree.to_str().unwrap()),
            workspace_root: Some(workspace.to_str().unwrap()),
            new_branch: Some("feature/worktree"),
            ..GitOperationRequest::new("worktree_add")
        },
    )
    .await
    .unwrap();
    assert!(created.ok, "{}", created.stderr);
    let nested_created = git_op(
        &first,
        GitOperationRequest {
            action: "worktree_add",
            worktree_path: Some(nested_worktree.to_str().unwrap()),
            workspace_root: Some(workspace.to_str().unwrap()),
            new_branch: Some("feature/nested-worktree"),
            ..GitOperationRequest::new("worktree_add")
        },
    )
    .await
    .unwrap();
    assert!(nested_created.ok, "{}", nested_created.stderr);

    let repositories =
        git_repositories_with_options(&workspace, GitRepositoryDiscoveryOptions::default())
            .await
            .unwrap();
    assert_eq!(repositories.repositories.len(), 2);
    let first_summary = repositories
        .repositories
        .iter()
        .find(|repository| path_key(&repository.root) == path_key(&first))
        .unwrap();
    assert_eq!(first_summary.worktrees.len(), 3);
    assert!(first_summary
        .worktrees
        .iter()
        .any(|item| path_key(&item.path) == path_key(&worktree)));

    let statuses = git_repository_statuses(
        &workspace,
        &[
            second.display().to_string(),
            first.display().to_string(),
            first.display().to_string(),
        ],
    )
    .await
    .unwrap();
    assert_eq!(statuses.repositories.len(), 2);
    assert_eq!(
        path_key(&statuses.repositories[0].repo_root),
        path_key(&second)
    );
    assert_eq!(
        path_key(&statuses.repositories[1].repo_root),
        path_key(&first)
    );

    let validated = validate_git_repository_root(&workspace, worktree.to_str().unwrap())
        .await
        .unwrap();
    // Windows 8.3 短路径与 macOS /private 前缀在 canonicalize 后可能不同
    assert_eq!(path_key(&validated), path_key(&worktree));

    let removed = git_op(
        &first,
        GitOperationRequest {
            action: "worktree_remove",
            worktree_path: Some(worktree.to_str().unwrap()),
            ..GitOperationRequest::new("worktree_remove")
        },
    )
    .await
    .unwrap();
    assert!(removed.ok, "{}", removed.stderr);
    assert!(!worktree.exists());

    let nested_removed = git_op(
        &first,
        GitOperationRequest {
            action: "worktree_remove",
            worktree_path: Some(nested_worktree.to_str().unwrap()),
            ..GitOperationRequest::new("worktree_remove")
        },
    )
    .await
    .unwrap();
    assert!(nested_removed.ok, "{}", nested_removed.stderr);
    assert!(!nested_worktree.exists());
}

/// 验证仓库自动探测关闭后不扫描工作区中的嵌套仓库。
#[tokio::test]
async fn disables_nested_repository_detection() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let nested = workspace.join("packages/nested");
    init_repository(&nested).await;

    let repositories = git_repositories_with_options(
        &workspace,
        GitRepositoryDiscoveryOptions {
            auto_repository_detection: false,
            ..GitRepositoryDiscoveryOptions::default()
        },
    )
    .await
    .unwrap();

    assert!(repositories.repositories.is_empty());
}

/// 验证 worktree 探测开关和数量限制会应用到仓库摘要。
#[tokio::test]
async fn applies_worktree_detection_options() {
    let temp = tempfile::tempdir().unwrap();
    let repository = temp.path().join("repository");
    let first_worktree = temp.path().join("first-worktree");
    let second_worktree = temp.path().join("second-worktree");
    init_repository(&repository).await;
    git_success(
        &repository,
        &[
            "worktree",
            "add",
            "-b",
            "feature/first",
            first_worktree.to_str().unwrap(),
        ],
    )
    .await
    .unwrap();
    git_success(
        &repository,
        &[
            "worktree",
            "add",
            "-b",
            "feature/second",
            second_worktree.to_str().unwrap(),
        ],
    )
    .await
    .unwrap();

    let limited = git_repositories_with_options(
        &repository,
        GitRepositoryDiscoveryOptions {
            detect_worktrees_limit: 2,
            ..GitRepositoryDiscoveryOptions::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(limited.repositories[0].worktrees.len(), 2);

    let disabled = git_repositories_with_options(
        &repository,
        GitRepositoryDiscoveryOptions {
            detect_worktrees: false,
            ..GitRepositoryDiscoveryOptions::default()
        },
    )
    .await
    .unwrap();
    assert!(disabled.repositories[0].worktrees.is_empty());
}

/// 验证新 worktree 不能逃逸活动工作区允许范围。
#[tokio::test]
async fn rejects_worktree_creation_outside_workspace_scope() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let repository = workspace.join("repository");
    let target = outside.path().join("outside-worktree");
    init_repository(&repository).await;

    let response = git_op(
        &repository,
        GitOperationRequest {
            action: "worktree_add",
            worktree_path: Some(target.to_str().unwrap()),
            workspace_root: Some(workspace.to_str().unwrap()),
            new_branch: Some("feature/outside"),
            ..GitOperationRequest::new("worktree_add")
        },
    )
    .await
    .unwrap();

    assert!(!response.ok);
    assert!(response
        .stderr
        .contains("outside the active workspace scope"));
    assert!(!target.exists());
}

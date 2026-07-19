use super::*;
use std::path::Path;

/// 创建包含初始提交的测试仓库。
///
/// 参数:
/// - `root`: 待初始化目录
///
/// 返回:
/// - 无
async fn init_repository(root: &Path) {
    git_success(root, &["init", "-b", "main"]).await.unwrap();
    git_success(root, &["config", "user.name", "Sai Test"])
        .await
        .unwrap();
    git_success(root, &["config", "user.email", "sai@example.com"])
        .await
        .unwrap();
    tokio::fs::write(root.join("tracked.txt"), "initial\n")
        .await
        .unwrap();
    git_success(root, &["add", "tracked.txt"]).await.unwrap();
    git_success(root, &["commit", "-m", "initial"])
        .await
        .unwrap();
}

#[tokio::test]
async fn manages_local_branch_lifecycle() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_repository(repo).await;

    create_branch(repo, Some("feature/editor"), None)
        .await
        .unwrap();
    rename_branch(repo, Some("feature/editor"), Some("feature/workspace"))
        .await
        .unwrap();
    switch_branch(repo, Some("main"), Some("local"))
        .await
        .unwrap();
    let state = git_status(repo).await.unwrap();
    delete_branch(repo, &state, Some("feature/workspace"), false)
        .await
        .unwrap();

    assert!(!branch_exists_local(repo, "feature/workspace").await);
}

#[tokio::test]
async fn switches_remote_branch_with_explicit_kind() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let remote = temp.path().join("remote.git");
    tokio::fs::create_dir(&repo).await.unwrap();
    tokio::fs::create_dir(&remote).await.unwrap();
    init_repository(&repo).await;
    git_success(&remote, &["init", "--bare"]).await.unwrap();
    git_success(
        &repo,
        &["remote", "add", "upstream", remote.to_str().unwrap()],
    )
    .await
    .unwrap();
    create_branch(&repo, Some("feature/nested"), None)
        .await
        .unwrap();
    git_success(&repo, &["push", "-u", "upstream", "feature/nested"])
        .await
        .unwrap();
    switch_branch(&repo, Some("main"), Some("local"))
        .await
        .unwrap();
    git_success(&repo, &["branch", "-D", "feature/nested"])
        .await
        .unwrap();

    switch_branch(&repo, Some("upstream/feature/nested"), Some("remote"))
        .await
        .unwrap();

    assert_eq!(git_status(&repo).await.unwrap().head, "feature/nested");
}

#[tokio::test]
async fn discards_both_paths_of_a_staged_rename() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_repository(repo).await;
    git_success(repo, &["mv", "tracked.txt", "renamed.txt"])
        .await
        .unwrap();

    let response = git_op(
        repo,
        GitOperationRequest {
            action: "discard",
            path: Some("renamed.txt"),
            old_path: Some("tracked.txt"),
            message: None,
            remote_url: None,
            branch: None,
            branch_kind: None,
            new_branch: None,
            start_point: None,
            post_action: None,
            all: false,
            amend: false,
            signoff: false,
            force: false,
        },
    )
    .await
    .unwrap();

    assert!(response.ok, "{}", response.stderr);
    assert!(repo.join("tracked.txt").exists());
    assert!(!repo.join("renamed.txt").exists());
    assert!(response.state.entries.is_empty());
}

#[tokio::test]
async fn commits_all_changes_with_signoff() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_repository(repo).await;
    tokio::fs::write(repo.join("tracked.txt"), "updated\n")
        .await
        .unwrap();
    tokio::fs::write(repo.join("new.txt"), "new\n")
        .await
        .unwrap();

    let response = git_op(
        repo,
        GitOperationRequest {
            action: "commit",
            path: None,
            old_path: None,
            message: Some("feat: update files"),
            remote_url: None,
            branch: None,
            branch_kind: None,
            new_branch: None,
            start_point: None,
            post_action: None,
            all: true,
            amend: false,
            signoff: true,
            force: false,
        },
    )
    .await
    .unwrap();

    assert!(response.ok, "{}", response.stderr);
    assert!(response.state.entries.is_empty());
    let body = git_success(repo, &["show", "-s", "--format=%B", "HEAD"])
        .await
        .unwrap();
    assert!(body
        .stdout
        .contains("Signed-off-by: Sai Test <sai@example.com>"));
}

#[tokio::test]
async fn detects_and_aborts_merge_conflict() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_repository(repo).await;
    create_branch(repo, Some("feature/conflict"), None)
        .await
        .unwrap();
    tokio::fs::write(repo.join("tracked.txt"), "feature\n")
        .await
        .unwrap();
    git_success(repo, &["commit", "-am", "feature change"])
        .await
        .unwrap();
    switch_branch(repo, Some("main"), Some("local"))
        .await
        .unwrap();
    tokio::fs::write(repo.join("tracked.txt"), "main\n")
        .await
        .unwrap();
    git_success(repo, &["commit", "-am", "main change"])
        .await
        .unwrap();
    let merge = git_raw(repo, &["merge", "feature/conflict"]).await.unwrap();
    assert!(!merge.status.success());

    let state = git_status(repo).await.unwrap();
    assert_eq!(
        state.operation.as_ref().map(|value| value.kind.as_str()),
        Some("merge")
    );
    assert_eq!(state.dirty_counts.conflicted, 1);

    let response = git_op(
        repo,
        GitOperationRequest {
            action: "abort_operation",
            path: None,
            old_path: None,
            message: None,
            remote_url: None,
            branch: None,
            branch_kind: None,
            new_branch: None,
            start_point: None,
            post_action: None,
            all: false,
            amend: false,
            signoff: false,
            force: false,
        },
    )
    .await
    .unwrap();

    assert!(response.ok, "{}", response.stderr);
    assert!(response.state.operation.is_none());
    assert_eq!(response.state.dirty_counts.conflicted, 0);
}

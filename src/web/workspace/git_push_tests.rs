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
async fn pushes_current_branch_to_selected_remote() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let remote = temp.path().join("backup.git");
    tokio::fs::create_dir(&repo).await.unwrap();
    tokio::fs::create_dir(&remote).await.unwrap();
    init_repository(&repo).await;
    git_success(&remote, &["init", "--bare"]).await.unwrap();
    git_success(
        &repo,
        &["remote", "add", "backup", remote.to_str().unwrap()],
    )
    .await
    .unwrap();

    let response = git_op(
        &repo,
        GitOperationRequest {
            remote_name: Some("backup"),
            ..GitOperationRequest::new("push_to")
        },
    )
    .await
    .unwrap();

    assert!(response.ok, "{}", response.stderr);
    assert_eq!(response.state.upstream, "backup/main");
    let remote_head = git_success(&remote, &["rev-parse", "refs/heads/main"])
        .await
        .unwrap();
    let local_head = git_success(&repo, &["rev-parse", "HEAD"]).await.unwrap();
    assert_eq!(remote_head.stdout, local_head.stdout);
}

#[tokio::test]
async fn rejects_push_to_unknown_remote() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_repository(repo).await;

    let response = git_op(
        repo,
        GitOperationRequest {
            remote_name: Some("missing"),
            ..GitOperationRequest::new("push_to")
        },
    )
    .await
    .unwrap();

    assert!(!response.ok);
    assert!(response.stderr.contains("remote does not exist: missing"));
}

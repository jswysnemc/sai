use super::*;

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
/// 验证可以通过增强操作将指定分支合并到当前分支。
#[tokio::test]
async fn merges_branch_into_current_branch() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_repository(repo).await;
    create_branch(repo, Some("feature/merge"), None)
        .await
        .unwrap();
    tokio::fs::write(repo.join("feature.txt"), "merged content\n")
        .await
        .unwrap();
    git_success(repo, &["add", "feature.txt"]).await.unwrap();
    git_success(repo, &["commit", "-m", "feature commit"])
        .await
        .unwrap();
    switch_branch(repo, Some("main"), Some("local"))
        .await
        .unwrap();

    let response = git_op(
        repo,
        GitOperationRequest {
            branch: Some("feature/merge"),
            ..GitOperationRequest::new("merge_branch")
        },
    )
    .await
    .unwrap();

    assert!(response.ok, "{}", response.stderr);
    assert_eq!(response.state.head, "main");
    assert!(repo.join("feature.txt").exists());
}

/// 验证可以将当前分支变基到指定分支。
#[tokio::test]
async fn rebases_current_branch_onto_target_branch() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_repository(repo).await;
    create_branch(repo, Some("base/update"), None)
        .await
        .unwrap();
    tokio::fs::write(repo.join("base.txt"), "base update\n")
        .await
        .unwrap();
    git_success(repo, &["add", "base.txt"]).await.unwrap();
    git_success(repo, &["commit", "-m", "base update"])
        .await
        .unwrap();
    switch_branch(repo, Some("main"), Some("local"))
        .await
        .unwrap();
    create_branch(repo, Some("feature/rebase"), None)
        .await
        .unwrap();
    tokio::fs::write(repo.join("topic.txt"), "topic change\n")
        .await
        .unwrap();
    git_success(repo, &["add", "topic.txt"]).await.unwrap();
    git_success(repo, &["commit", "-m", "topic change"])
        .await
        .unwrap();

    let response = git_op(
        repo,
        GitOperationRequest {
            branch: Some("base/update"),
            ..GitOperationRequest::new("rebase_branch")
        },
    )
    .await
    .unwrap();

    assert!(response.ok, "{}", response.stderr);
    assert_eq!(response.state.head, "feature/rebase");
    git_success(
        repo,
        &[
            "merge-base",
            "--is-ancestor",
            "base/update",
            "feature/rebase",
        ],
    )
    .await
    .unwrap();
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

use super::*;

#[tokio::test]
async fn publishes_current_branch_to_new_origin() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let remote = temp.path().join("remote.git");
    tokio::fs::create_dir(&repo).await.unwrap();
    tokio::fs::create_dir(&remote).await.unwrap();
    git_success(&repo, &["init", "-b", "main"]).await.unwrap();
    git_success(&repo, &["config", "user.name", "Sai Test"])
        .await
        .unwrap();
    git_success(&repo, &["config", "user.email", "sai@example.com"])
        .await
        .unwrap();
    tokio::fs::write(repo.join("tracked.txt"), "initial\n")
        .await
        .unwrap();
    git_success(&repo, &["add", "tracked.txt"]).await.unwrap();
    git_success(&repo, &["commit", "-m", "initial"])
        .await
        .unwrap();
    git_success(&remote, &["init", "--bare"]).await.unwrap();

    let response = git_op(
        &repo,
        GitOperationRequest {
            action: "publish",
            remote_url: Some(remote.to_str().unwrap()),
            ..GitOperationRequest::new("publish")
        },
    )
    .await
    .unwrap();

    assert!(response.ok, "{}", response.stderr);
    assert_eq!(response.state.upstream, "origin/main");
    assert!(ref_exists(&remote, "refs/heads/main").await);
}

#[tokio::test]
async fn rejects_publish_without_commit() {
    let temp = tempfile::tempdir().unwrap();
    git_success(temp.path(), &["init", "-b", "main"])
        .await
        .unwrap();
    let state = git_status(temp.path()).await.unwrap();
    assert!(!state.has_commits);

    let response = git_op(
        temp.path(),
        GitOperationRequest {
            action: "publish",
            remote_url: Some("https://example.com/repo.git"),
            ..GitOperationRequest::new("publish")
        },
    )
    .await
    .unwrap();

    assert!(!response.ok);
    assert!(response.stderr.contains("create a commit"));
}

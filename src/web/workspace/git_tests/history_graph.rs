use super::*;

#[tokio::test]
async fn checks_out_and_cherry_picks_history_commits() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_repository(repo).await;
    create_branch(repo, Some("feature/history"), None)
        .await
        .unwrap();
    tokio::fs::write(repo.join("feature.txt"), "feature\n")
        .await
        .unwrap();
    git_success(repo, &["add", "feature.txt"]).await.unwrap();
    git_success(repo, &["commit", "-m", "feature history"])
        .await
        .unwrap();
    let feature_sha = git_success(repo, &["rev-parse", "HEAD"])
        .await
        .unwrap()
        .stdout;
    switch_branch(repo, Some("main"), Some("local"))
        .await
        .unwrap();

    let cherry_pick = git_op(
        repo,
        GitOperationRequest {
            action: "cherry_pick",
            commit: Some(&feature_sha),
            ..GitOperationRequest::new("cherry_pick")
        },
    )
    .await
    .unwrap();
    assert!(cherry_pick.ok, "{}", cherry_pick.stderr);
    assert!(repo.join("feature.txt").exists());

    let checkout = git_op(
        repo,
        GitOperationRequest {
            action: "checkout_commit",
            commit: Some(&feature_sha),
            ..GitOperationRequest::new("checkout_commit")
        },
    )
    .await
    .unwrap();
    assert!(checkout.ok, "{}", checkout.stderr);
    assert_eq!(checkout.state.head, "(detached)");
}
#[tokio::test]
async fn marks_outgoing_and_incoming_graph_commits() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let remote = temp.path().join("remote.git");
    let peer = temp.path().join("peer");
    tokio::fs::create_dir(&repo).await.unwrap();
    tokio::fs::create_dir(&remote).await.unwrap();
    init_repository(&repo).await;
    git_success(&remote, &["init", "--bare"]).await.unwrap();
    git_success(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    )
    .await
    .unwrap();
    git_success(&repo, &["push", "-u", "origin", "main"])
        .await
        .unwrap();

    tokio::fs::write(repo.join("local.txt"), "local\n")
        .await
        .unwrap();
    git_success(&repo, &["add", "local.txt"]).await.unwrap();
    git_success(&repo, &["commit", "-m", "local only"])
        .await
        .unwrap();

    git_success(
        temp.path(),
        &[
            "clone",
            "-b",
            "main",
            remote.to_str().unwrap(),
            peer.to_str().unwrap(),
        ],
    )
    .await
    .unwrap();
    git_success(&peer, &["config", "user.name", "Sai Peer"])
        .await
        .unwrap();
    git_success(&peer, &["config", "user.email", "peer@example.com"])
        .await
        .unwrap();
    tokio::fs::write(peer.join("remote.txt"), "remote\n")
        .await
        .unwrap();
    git_success(&peer, &["add", "remote.txt"]).await.unwrap();
    git_success(&peer, &["commit", "-m", "remote only"])
        .await
        .unwrap();
    git_success(&peer, &["push"]).await.unwrap();
    git_success(&repo, &["fetch", "origin"]).await.unwrap();

    let graph = git_log(&repo, Some(20), Some(0)).await.unwrap();
    assert!(graph.commits.iter().any(|commit| {
        commit.subject == "local only"
            && commit.local_only
            && commit.files.iter().any(|file| file.path == "local.txt")
    }));
    assert!(graph
        .commits
        .iter()
        .any(|commit| commit.subject == "remote only" && commit.remote_only));
}

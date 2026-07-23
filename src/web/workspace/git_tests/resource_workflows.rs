use super::*;

#[tokio::test]
async fn manages_stashes_tags_and_remotes() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let remote = temp.path().join("backup.git");
    tokio::fs::create_dir(&repo).await.unwrap();
    tokio::fs::create_dir(&remote).await.unwrap();
    init_repository(&repo).await;
    git_success(&remote, &["init", "--bare"]).await.unwrap();
    tokio::fs::write(repo.join("tracked.txt"), "stashed\n")
        .await
        .unwrap();
    tokio::fs::write(repo.join("new.txt"), "untracked\n")
        .await
        .unwrap();

    let stashed = git_op(
        &repo,
        GitOperationRequest {
            action: "stash_push",
            message: Some("workspace changes"),
            include_untracked: true,
            ..GitOperationRequest::new("stash_push")
        },
    )
    .await
    .unwrap();
    assert!(stashed.ok, "{}", stashed.stderr);
    assert!(stashed.state.entries.is_empty());

    let tagged = git_op(
        &repo,
        GitOperationRequest {
            action: "tag_create",
            tag: Some("v1.0.0"),
            ..GitOperationRequest::new("tag_create")
        },
    )
    .await
    .unwrap();
    assert!(tagged.ok, "{}", tagged.stderr);
    let remote_added = git_op(
        &repo,
        GitOperationRequest {
            action: "remote_add",
            remote_name: Some("backup"),
            remote_url: Some(remote.to_str().unwrap()),
            ..GitOperationRequest::new("remote_add")
        },
    )
    .await
    .unwrap();
    assert!(remote_added.ok, "{}", remote_added.stderr);

    let resources = git_resources(&repo).await.unwrap();
    assert_eq!(resources.stashes.len(), 1);
    assert_eq!(resources.tags[0].name, "v1.0.0");
    assert_eq!(resources.remotes[0].name, "backup");

    let applied = git_op(
        &repo,
        GitOperationRequest {
            action: "stash_apply",
            stash_ref: Some("stash@{0}"),
            ..GitOperationRequest::new("stash_apply")
        },
    )
    .await
    .unwrap();
    assert!(applied.ok, "{}", applied.stderr);
    assert!(repo.join("new.txt").exists());

    for request in [
        GitOperationRequest {
            action: "stash_drop",
            stash_ref: Some("stash@{0}"),
            ..GitOperationRequest::new("stash_drop")
        },
        GitOperationRequest {
            action: "tag_delete",
            tag: Some("v1.0.0"),
            ..GitOperationRequest::new("tag_delete")
        },
        GitOperationRequest {
            action: "remote_remove",
            remote_name: Some("backup"),
            ..GitOperationRequest::new("remote_remove")
        },
    ] {
        let response = git_op(&repo, request).await.unwrap();
        assert!(response.ok, "{}", response.stderr);
    }
    let resources = git_resources(&repo).await.unwrap();
    assert!(resources.stashes.is_empty());
    assert!(resources.tags.is_empty());
    assert!(resources.remotes.is_empty());
}

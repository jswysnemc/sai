use super::*;

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
            paths: &[],
            old_path: None,
            message: None,
            remote_url: None,
            branch: None,
            branch_kind: None,
            new_branch: None,
            start_point: None,
            post_action: None,
            patch: None,
            commit: None,
            reset_mode: None,
            stash_ref: None,
            tag: None,
            remote_name: None,
            worktree_path: None,
            workspace_root: None,
            include_untracked: false,
            exclude_untracked: false,
            resolution: None,
            content: None,
            all: false,
            amend: false,
            signoff: false,
            allow_empty: false,
            force: false,
        },
    )
    .await
    .unwrap();

    assert!(response.ok, "{}", response.stderr);
    assert!(response.state.operation.is_none());
    assert_eq!(response.state.dirty_counts.conflicted, 0);
}
#[tokio::test]
async fn reads_and_resolves_merge_editor_content() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_repository(repo).await;
    create_branch(repo, Some("feature/merge-editor"), None)
        .await
        .unwrap();
    tokio::fs::write(repo.join("tracked.txt"), "theirs\n")
        .await
        .unwrap();
    git_success(repo, &["commit", "-am", "theirs"])
        .await
        .unwrap();
    switch_branch(repo, Some("main"), Some("local"))
        .await
        .unwrap();
    tokio::fs::write(repo.join("tracked.txt"), "ours\n")
        .await
        .unwrap();
    git_success(repo, &["commit", "-am", "ours"]).await.unwrap();
    assert!(!git_raw(repo, &["merge", "feature/merge-editor"])
        .await
        .unwrap()
        .status
        .success());

    let conflict = git_conflict(repo, "tracked.txt").await.unwrap();
    assert_eq!(conflict.base.as_deref(), Some("initial\n"));
    assert_eq!(conflict.ours.as_deref(), Some("ours\n"));
    assert_eq!(conflict.theirs.as_deref(), Some("theirs\n"));
    assert!(conflict.current.contains("<<<<<<<"));

    let resolved = git_op(
        repo,
        GitOperationRequest {
            action: "resolve_conflict",
            path: Some("tracked.txt"),
            resolution: Some("content"),
            content: Some("resolved\n"),
            ..GitOperationRequest::new("resolve_conflict")
        },
    )
    .await
    .unwrap();
    assert!(resolved.ok, "{}", resolved.stderr);
    assert_eq!(resolved.state.dirty_counts.conflicted, 0);
    assert_eq!(resolved.state.dirty_counts.staged, 1);
    assert_eq!(
        tokio::fs::read_to_string(repo.join("tracked.txt"))
            .await
            .unwrap()
            .replace("\r\n", "\n"),
        "resolved\n"
    );

    let completed = git_op(repo, GitOperationRequest::new("continue_operation"))
        .await
        .unwrap();
    assert!(completed.ok, "{}", completed.stderr);
    assert!(completed.state.operation.is_none());
    assert!(completed.state.entries.is_empty());
}

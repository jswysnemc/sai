use super::*;

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
            paths: &[],
            old_path: Some("tracked.txt"),
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
    assert!(repo.join("tracked.txt").exists());
    assert!(!repo.join("renamed.txt").exists());
    assert!(response.state.entries.is_empty());
}
/// 验证批量路径可以一次完成暂存、取消暂存和丢弃。
#[tokio::test]
async fn batches_file_stage_unstage_and_discard() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_repository(repo).await;
    tokio::fs::write(repo.join("tracked.txt"), "batch update\n")
        .await
        .unwrap();
    tokio::fs::write(repo.join("new.txt"), "new file\n")
        .await
        .unwrap();
    let paths = vec!["tracked.txt".to_string(), "new.txt".to_string()];

    let staged = git_op(
        repo,
        GitOperationRequest {
            paths: &paths,
            ..GitOperationRequest::new("stage")
        },
    )
    .await
    .unwrap();
    assert!(staged.ok, "{}", staged.stderr);
    assert_eq!(staged.state.dirty_counts.staged, 2);

    let unstaged = git_op(
        repo,
        GitOperationRequest {
            paths: &paths,
            ..GitOperationRequest::new("unstage")
        },
    )
    .await
    .unwrap();
    assert!(unstaged.ok, "{}", unstaged.stderr);
    assert_eq!(unstaged.state.dirty_counts.staged, 0);

    let discarded = git_op(
        repo,
        GitOperationRequest {
            paths: &paths,
            ..GitOperationRequest::new("discard")
        },
    )
    .await
    .unwrap();
    assert!(discarded.ok, "{}", discarded.stderr);
    assert!(discarded.state.entries.is_empty());
    assert!(!repo.join("new.txt").exists());
    let content = tokio::fs::read_to_string(repo.join("tracked.txt"))
        .await
        .unwrap()
        .replace("\r\n", "\n");
    assert_eq!(content.replace("\r\n", "\n"), "initial\n");
}

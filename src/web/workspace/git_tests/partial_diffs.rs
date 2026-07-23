use super::*;

#[tokio::test]
async fn separates_staged_and_unstaged_diff_content() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_repository(repo).await;
    tokio::fs::write(repo.join("tracked.txt"), "staged\n")
        .await
        .unwrap();
    git_success(repo, &["add", "tracked.txt"]).await.unwrap();
    tokio::fs::write(repo.join("tracked.txt"), "unstaged\n")
        .await
        .unwrap();
    tokio::fs::write(repo.join("new.txt"), "untracked\n")
        .await
        .unwrap();

    let staged = git_diff(repo, "staged", Some("tracked.txt")).await.unwrap();
    let unstaged = git_diff(repo, "unstaged", None).await.unwrap();

    assert_eq!(staged.base_ref, "HEAD");
    assert_eq!(staged.head_ref, "INDEX");
    assert!(staged.patch.contains("+staged"));
    assert!(!staged.patch.contains("+unstaged"));
    assert_eq!(unstaged.base_ref, "INDEX");
    assert_eq!(unstaged.head_ref, "WORKTREE");
    assert!(unstaged.patch.contains("+unstaged"));
    assert!(unstaged.patch.contains("+untracked"));
}
#[tokio::test]
async fn applies_partial_patch_to_index_and_worktree() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_repository(repo).await;
    tokio::fs::write(repo.join("tracked.txt"), "updated\n")
        .await
        .unwrap();
    let patch = concat!(
        "diff --git a/tracked.txt b/tracked.txt\n",
        "--- a/tracked.txt\n",
        "+++ b/tracked.txt\n",
        "@@ -1 +1 @@\n",
        "-initial\n",
        "+updated\n"
    );

    let staged = git_op(
        repo,
        GitOperationRequest {
            action: "stage_patch",
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
            patch: Some(patch),
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
    assert!(staged.ok, "{}", staged.stderr);
    assert_eq!(staged.state.dirty_counts.staged, 1);
    assert_eq!(staged.state.dirty_counts.unstaged, 0);

    let unstaged = git_op(
        repo,
        GitOperationRequest {
            action: "unstage_patch",
            patch: Some(patch),
            ..GitOperationRequest::new("unstage_patch")
        },
    )
    .await
    .unwrap();
    assert!(unstaged.ok, "{}", unstaged.stderr);
    assert_eq!(unstaged.state.dirty_counts.staged, 0);
    assert_eq!(unstaged.state.dirty_counts.unstaged, 1);

    let discarded = git_op(
        repo,
        GitOperationRequest {
            action: "discard_patch",
            patch: Some(patch),
            ..GitOperationRequest::new("discard_patch")
        },
    )
    .await
    .unwrap();
    assert!(discarded.ok, "{}", discarded.stderr);
    assert!(discarded.state.entries.is_empty());
    assert_eq!(
        tokio::fs::read_to_string(repo.join("tracked.txt"))
            .await
            .unwrap()
            .replace("\r\n", "\n"),
        "initial\n"
    );
}

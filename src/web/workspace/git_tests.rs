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
            patch: None,
            commit: None,
            reset_mode: None,
            stash_ref: None,
            tag: None,
            remote_name: None,
            include_untracked: false,
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
            patch: None,
            commit: None,
            reset_mode: None,
            stash_ref: None,
            tag: None,
            remote_name: None,
            include_untracked: false,
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
            patch: None,
            commit: None,
            reset_mode: None,
            stash_ref: None,
            tag: None,
            remote_name: None,
            include_untracked: false,
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
            include_untracked: false,
            all: false,
            amend: false,
            signoff: false,
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
            .unwrap(),
        "initial\n"
    );
}

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

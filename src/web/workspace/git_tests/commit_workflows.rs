use super::*;

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
            paths: &[],
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
            worktree_path: None,
            workspace_root: None,
            include_untracked: false,
            exclude_untracked: false,
            resolution: None,
            content: None,
            all: true,
            amend: false,
            signoff: true,
            allow_empty: false,
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
/// 验证显式允许时可以创建没有文件变化的提交。
#[tokio::test]
async fn creates_explicit_empty_commit() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_repository(repo).await;

    let response = git_op(
        repo,
        GitOperationRequest {
            message: Some("chore: record checkpoint"),
            allow_empty: true,
            ..GitOperationRequest::new("commit")
        },
    )
    .await
    .unwrap();

    assert!(response.ok, "{}", response.stderr);
    let subject = git_success(repo, &["show", "-s", "--format=%s", "HEAD"])
        .await
        .unwrap();
    assert_eq!(subject.stdout.trim(), "chore: record checkpoint");
}

/// 验证 Commit All 可以只提交已跟踪文件并保留未跟踪文件。
#[tokio::test]
async fn commits_all_without_hidden_untracked_files() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_repository(repo).await;
    tokio::fs::write(repo.join("tracked.txt"), "tracked update\n")
        .await
        .unwrap();
    tokio::fs::write(repo.join("hidden.txt"), "remain untracked\n")
        .await
        .unwrap();

    let response = git_op(
        repo,
        GitOperationRequest {
            message: Some("fix: update tracked file"),
            all: true,
            exclude_untracked: true,
            ..GitOperationRequest::new("commit")
        },
    )
    .await
    .unwrap();

    assert!(response.ok, "{}", response.stderr);
    assert!(response
        .state
        .entries
        .iter()
        .any(|entry| entry.path == "hidden.txt" && entry.untracked));
    let tracked = git_success(repo, &["show", "HEAD:tracked.txt"])
        .await
        .unwrap();
    assert_eq!(tracked.stdout, "tracked update");
    let indexed = git_success(repo, &["ls-files", "--", "hidden.txt"])
        .await
        .unwrap();
    assert!(indexed.stdout.trim().is_empty());
}

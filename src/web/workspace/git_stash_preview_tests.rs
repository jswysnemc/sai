use super::*;

/// 验证 stash 预览包含已跟踪和未跟踪文件的真实补丁。
#[tokio::test]
async fn reads_stash_patch_with_untracked_files() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    git_success(repo, &["init", "-b", "main"]).await.unwrap();
    git_success(repo, &["config", "user.name", "Sai Test"])
        .await
        .unwrap();
    git_success(repo, &["config", "user.email", "sai@example.com"])
        .await
        .unwrap();
    tokio::fs::write(repo.join("tracked.txt"), "initial\n")
        .await
        .unwrap();
    git_success(repo, &["add", "tracked.txt"]).await.unwrap();
    git_success(repo, &["commit", "-m", "initial"])
        .await
        .unwrap();
    tokio::fs::write(repo.join("tracked.txt"), "changed\n")
        .await
        .unwrap();
    tokio::fs::write(repo.join("new.txt"), "untracked\n")
        .await
        .unwrap();
    stash_push(repo, Some("preview"), true).await.unwrap();

    let diff = git_stash_diff(repo, "stash@{0}").await.unwrap();

    assert_eq!(diff.mode, "stash");
    assert!(diff.patch.contains("-initial"));
    assert!(diff.patch.contains("+changed"));
    assert!(diff.patch.contains("new.txt"));
}

/// 验证 stash 预览拒绝非标准引用。
#[tokio::test]
async fn rejects_invalid_stash_preview_reference() {
    let temp = tempfile::tempdir().unwrap();
    let error = git_stash_diff(temp.path(), "HEAD").await.unwrap_err();
    assert!(error.to_string().contains("invalid stash reference"));
}

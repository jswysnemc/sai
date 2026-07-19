use super::*;

/// 验证文件比较返回真实 `--no-index` Diff。
#[tokio::test]
async fn compares_two_worktree_files() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    git_success(repo, &["init", "-b", "main"]).await.unwrap();
    tokio::fs::write(repo.join("base.txt"), "first\nshared\n")
        .await
        .unwrap();
    tokio::fs::write(repo.join("target.txt"), "second\nshared\n")
        .await
        .unwrap();

    let result = git_file_compare(repo, "base.txt", "target.txt")
        .await
        .unwrap();

    assert_eq!(result.mode, "files");
    assert_eq!(result.base_ref, "base.txt");
    assert_eq!(result.head_ref, "target.txt");
    assert!(result.patch.contains("-first"));
    assert!(result.patch.contains("+second"));
}

/// 验证文件比较拒绝相同路径和越界路径。
#[tokio::test]
async fn rejects_invalid_file_comparison_paths() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    git_success(repo, &["init", "-b", "main"]).await.unwrap();
    tokio::fs::write(repo.join("file.txt"), "content\n")
        .await
        .unwrap();

    let same = git_file_compare(repo, "file.txt", "file.txt")
        .await
        .unwrap_err();
    assert!(same.to_string().contains("two different paths"));

    let escaped = git_file_compare(repo, "../outside.txt", "file.txt")
        .await
        .unwrap_err();
    assert!(escaped.to_string().contains("escapes repository"));
}

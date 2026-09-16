use super::*;

/// 【工作概览】【工作树测试】暂存和未暂存改动按最终内容统计；无参数，无返回值。
#[tokio::test]
async fn counts_final_worktree_and_untracked_text_without_patch_payloads() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    init_repository(root).await;
    tokio::fs::write(root.join("tracked.txt"), "staged\n")
        .await
        .unwrap();
    git_success(root, &["add", "tracked.txt"]).await.unwrap();
    tokio::fs::write(root.join("tracked.txt"), "final\nextra\n")
        .await
        .unwrap();
    tokio::fs::write(root.join("new.txt"), "first\nsecond\n")
        .await
        .unwrap();
    tokio::fs::write(root.join("empty.txt"), "").await.unwrap();
    tokio::fs::write(root.join("binary.bin"), b"binary\0data")
        .await
        .unwrap();

    let stats = git_diff_stats(root).await.unwrap();
    assert_eq!(
        stats,
        GitDiffStats {
            added: 4,
            removed: 1
        }
    );
    let response = serde_json::to_value(stats).unwrap();
    assert_eq!(response.as_object().unwrap().len(), 2);
    assert!(response.get("patch").is_none());
}

/// 【工作概览】【初始仓库测试】没有 HEAD 时仍按工作树最终内容统计；无参数，无返回值。
#[tokio::test]
async fn supports_repositories_without_a_first_commit() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    git_success(root, &["init", "-b", "main"]).await.unwrap();
    tokio::fs::write(root.join("tracked.txt"), "staged\n")
        .await
        .unwrap();
    git_success(root, &["add", "tracked.txt"]).await.unwrap();
    tokio::fs::write(root.join("tracked.txt"), "first\nsecond\n")
        .await
        .unwrap();
    tokio::fs::write(root.join("untracked.txt"), "third\n")
        .await
        .unwrap();

    assert_eq!(
        git_diff_stats(root).await.unwrap(),
        GitDiffStats {
            added: 3,
            removed: 0
        }
    );
}

/// 【工作概览】【重命名测试】统计重命名后的正文变化；无参数，无返回值。
#[tokio::test]
async fn counts_changes_in_renamed_files() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    init_repository(root).await;
    git_success(root, &["mv", "tracked.txt", "renamed file.txt"])
        .await
        .unwrap();
    tokio::fs::write(root.join("renamed file.txt"), "initial\nextra\n")
        .await
        .unwrap();

    assert_eq!(
        git_diff_stats(root).await.unwrap(),
        GitDiffStats {
            added: 1,
            removed: 0
        }
    );
}

/// 【工作概览】【大补丁测试】计数不受补丁传输截断影响且响应保持紧凑；无参数，无返回值。
#[tokio::test]
async fn large_changes_still_return_only_complete_counts() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    init_repository(root).await;
    tokio::fs::write(
        root.join("tracked.txt"),
        "replacement line\n".repeat(40_000),
    )
    .await
    .unwrap();

    let stats = git_diff_stats(root).await.unwrap();
    assert_eq!(
        stats,
        GitDiffStats {
            added: 40_000,
            removed: 1
        }
    );
    assert!(serde_json::to_vec(&stats).unwrap().len() < 80);
}

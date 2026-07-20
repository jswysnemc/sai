use super::*;
use std::path::Path;

/// 创建包含初始提交的冲突测试仓库。
///
/// 参数:
/// - `root`: 待初始化目录
///
/// 返回:
/// - 无
async fn init_conflict_repository(root: &Path) {
    git_success(root, &["init", "-b", "main"]).await.unwrap();
    git_success(root, &["config", "user.name", "Sai Test"])
        .await
        .unwrap();
    git_success(root, &["config", "user.email", "sai@example.com"])
        .await
        .unwrap();
    git_success(root, &["config", "core.autocrlf", "false"])
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

/// 验证 add/add 冲突可以读取双方阶段并采用 ours 解决。
#[tokio::test]
async fn resolves_add_add_conflict() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_conflict_repository(repo).await;
    create_branch(repo, Some("feature/add-add"), None)
        .await
        .unwrap();
    tokio::fs::write(repo.join("shared.txt"), "theirs\n")
        .await
        .unwrap();
    git_success(repo, &["add", "shared.txt"]).await.unwrap();
    git_success(repo, &["commit", "-m", "feature file"])
        .await
        .unwrap();
    switch_branch(repo, Some("main"), Some("local"))
        .await
        .unwrap();
    tokio::fs::write(repo.join("shared.txt"), "ours\n")
        .await
        .unwrap();
    git_success(repo, &["add", "shared.txt"]).await.unwrap();
    git_success(repo, &["commit", "-m", "main file"])
        .await
        .unwrap();
    let merge = git_raw(repo, &["merge", "feature/add-add"]).await.unwrap();
    assert!(!merge.status.success());

    let conflict = git_conflict(repo, "shared.txt").await.unwrap();
    assert!(conflict.base.is_none());
    assert_eq!(conflict.ours.as_deref(), Some("ours\n"));
    assert_eq!(conflict.theirs.as_deref(), Some("theirs\n"));

    let resolved = git_op(
        repo,
        GitOperationRequest {
            action: "resolve_conflict",
            path: Some("shared.txt"),
            resolution: Some("ours"),
            ..GitOperationRequest::new("resolve_conflict")
        },
    )
    .await
    .unwrap();

    assert!(resolved.ok, "{}", resolved.stderr);
    assert_eq!(resolved.state.dirty_counts.conflicted, 0);
    let shared = tokio::fs::read_to_string(repo.join("shared.txt"))
        .await
        .unwrap()
        .replace("\r\n", "\n");
    assert_eq!(shared, "ours\n");
}

/// 验证 modify/delete 冲突选择删除侧时会暂存文件删除。
#[tokio::test]
async fn resolves_modify_delete_conflict() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path();
    init_conflict_repository(repo).await;
    create_branch(repo, Some("feature/delete"), None)
        .await
        .unwrap();
    git_success(repo, &["rm", "tracked.txt"]).await.unwrap();
    git_success(repo, &["commit", "-m", "delete file"])
        .await
        .unwrap();
    switch_branch(repo, Some("main"), Some("local"))
        .await
        .unwrap();
    tokio::fs::write(repo.join("tracked.txt"), "ours modified\n")
        .await
        .unwrap();
    git_success(repo, &["commit", "-am", "modify file"])
        .await
        .unwrap();
    let merge = git_raw(repo, &["merge", "feature/delete"]).await.unwrap();
    assert!(!merge.status.success());

    let conflict = git_conflict(repo, "tracked.txt").await.unwrap();
    assert_eq!(conflict.ours.as_deref(), Some("ours modified\n"));
    assert!(conflict.theirs.is_none());

    let resolved = git_op(
        repo,
        GitOperationRequest {
            action: "resolve_conflict",
            path: Some("tracked.txt"),
            resolution: Some("theirs"),
            ..GitOperationRequest::new("resolve_conflict")
        },
    )
    .await
    .unwrap();

    assert!(resolved.ok, "{}", resolved.stderr);
    assert_eq!(resolved.state.dirty_counts.conflicted, 0);
    assert!(!repo.join("tracked.txt").exists());
}

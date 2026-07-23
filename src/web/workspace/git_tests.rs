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

#[path = "git_tests/branch_workflows.rs"]
mod branch_workflows;

#[path = "git_tests/commit_workflows.rs"]
mod commit_workflows;

#[path = "git_tests/conflict_workflows.rs"]
mod conflict_workflows;

#[path = "git_tests/file_changes.rs"]
mod file_changes;

#[path = "git_tests/history_graph.rs"]
mod history_graph;

#[path = "git_tests/partial_diffs.rs"]
mod partial_diffs;

#[path = "git_tests/resource_workflows.rs"]
mod resource_workflows;

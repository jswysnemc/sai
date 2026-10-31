use super::*;

/// 【Git侧栏测试】【命令辅助】执行隔离测试仓库的 Git 配置和提交。
/// 参数: directory 为测试目录，args 为参数；返回: 无。
fn git(directory: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// 【Git侧栏测试】【状态分类】复用接口正确显示暂存、未暂存和未跟踪文件，不修改仓库。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn snapshot_reports_staged_unstaged_and_untracked_files() {
    let root = tempfile::tempdir().unwrap();
    git(root.path(), &["init"]);
    git(root.path(), &["config", "user.name", "Fixture"]);
    git(
        root.path(),
        &["config", "user.email", "fixture@example.invalid"],
    );
    std::fs::write(root.path().join("tracked.txt"), "before\n").unwrap();
    git(root.path(), &["add", "tracked.txt"]);
    git(root.path(), &["commit", "-m", "test: initial fixture"]);
    std::fs::write(root.path().join("tracked.txt"), "after\n").unwrap();
    std::fs::write(root.path().join("staged.txt"), "staged\n").unwrap();
    git(root.path(), &["add", "staged.txt"]);
    std::fs::write(root.path().join("untracked file.txt"), "new\n").unwrap();
    let summary = read_summary(root.path().into()).await;
    assert_eq!(summary.state, "ready");
    assert_eq!(
        (summary.staged, summary.unstaged, summary.untracked),
        (1, 1, 1)
    );
    assert!(summary
        .files
        .contains(&("??".into(), "untracked file.txt".into())));
    assert!(summary.files.contains(&(" M".into(), "tracked.txt".into())));
    assert!(summary.files.contains(&("A ".into(), "staged.txt".into())));
    assert_eq!(
        (
            summary.changes["tracked.txt"].added,
            summary.changes["tracked.txt"].removed
        ),
        (1, 1)
    );
    assert_eq!(
        (
            summary.changes["staged.txt"].added,
            summary.changes["staged.txt"].removed
        ),
        (1, 0)
    );
    assert_eq!(
        (
            summary.changes["untracked file.txt"].added,
            summary.changes["untracked file.txt"].removed
        ),
        (1, 0)
    );
    let no_repo = tempfile::tempdir().unwrap();
    assert_eq!(read_summary(no_repo.path().into()).await.state, "not_repo");
}

/// 【Git侧栏测试】【后台隔离】目录变化后取消旧任务，不把旧工作区结果显示在新会话。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn watcher_discards_stale_results_when_directory_changes() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    git(first.path(), &["init"]);
    let mut watcher = GitWatcher::default();
    assert!(watcher.tick(first.path().into()));
    assert!(watcher.tick(second.path().into()));
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(30)).await;
        watcher.tick(second.path().into());
        if watcher.summary.state == "not_repo" {
            break;
        }
    }
    assert_eq!(watcher.summary.state, "not_repo");
    assert_eq!(watcher.directory.as_deref(), Some(second.path()));
    assert!(!watcher.tick(second.path().into()));
    assert!(watcher.wait() > Duration::ZERO);
}

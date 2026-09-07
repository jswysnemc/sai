use super::background_tasks::cleanup_background_tasks;
use super::goal_completions::{
    acknowledge_background_completions, poll_session_background_completions,
};
use super::store::{BackgroundCommandStore, BackgroundCommandTask};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use std::process::Stdio;

/// 【后台命令】【消费回归】创建只等待标准输入的隔离子进程。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 测试拥有的子进程；测试退出时自动终止
fn waiting_child() -> tokio::process::Child {
    #[cfg(unix)]
    let mut command = {
        let mut command = tokio::process::Command::new("sh");
        command.args(["-c", "read pending"]);
        command
    };
    #[cfg(windows)]
    let mut command = {
        let mut command = tokio::process::Command::new("powershell.exe");
        command.args(["-NoProfile", "-Command", "[Console]::ReadLine()"]);
        command
    };
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap()
}

/// 【后台命令】【消费回归】登记一条完成记录和一条触发异步超时刷新的记录。
///
/// 参数:
/// - `store`: 临时后台任务存储
/// - `pid`: 测试隔离子进程标识
///
/// 返回:
/// - 无；初始化失败时终止测试
fn seed_tasks(store: &BackgroundCommandStore, pid: u32) {
    store.init().unwrap();
    let finished = BackgroundCommandTask {
        id: "finished-task".to_string(),
        runtime_process_id: None,
        runtime_owner_kind: Some("session".to_string()),
        runtime_owner_id: Some("session-1".to_string()),
        runtime_process_kind: Some("background_command".to_string()),
        goal_id: None,
        label: "completed command".to_string(),
        command: "test fixture".to_string(),
        cwd: ".".to_string(),
        pid,
        pgid: None,
        status: "exited".to_string(),
        stdout_log: store.logs_dir().join("finished.out").display().to_string(),
        stderr_log: store.logs_dir().join("finished.err").display().to_string(),
        started_at: 0,
        updated_at: 1,
        timeout_seconds: 1,
        completion_notified: false,
    };
    let mut running = finished.clone();
    running.id = "timeout-task".to_string();
    running.status = "running".to_string();
    running.stdout_log = store.logs_dir().join("timeout.out").display().to_string();
    running.stderr_log = store.logs_dir().join("timeout.err").display().to_string();
    store.save(&[finished, running]).unwrap();
}

/// 【后台命令】【消费回归】验证刷新期间的确认不会被旧任务快照覆盖。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无；同一完成通知再次出现时断言失败
#[tokio::test]
async fn background_poll_preserves_concurrent_acknowledgement() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    let mut child = waiting_child();
    seed_tasks(&store, child.id().unwrap());
    let mut config = AppConfig::default();
    config.tools.background_command_stop_grace_seconds = 1;

    // 1. 【后台命令】【消费回归】把真实轮询停在超时终止的等待点
    let pending = poll_session_background_completions(&paths, &config, "session-1");
    tokio::pin!(pending);
    assert!(futures_util::poll!(&mut pending).is_pending());

    // 2. 【后台命令】【消费回归】另一个消费者在刷新返回之前确认完成通知
    acknowledge_background_completions(&paths, "session-1", &["finished-task".to_string()])
        .unwrap();
    assert!(store.load().unwrap()[0].completion_notified);
    let (notices, _) = pending.await.unwrap();
    child.wait().await.unwrap();

    assert!(
        !notices
            .iter()
            .any(|notice| notice.task_id == "finished-task"),
        "已确认的后台完成通知不能在刷新后再次投递"
    );
    assert!(
        store
            .load()
            .unwrap()
            .iter()
            .find(|task| task.id == "finished-task")
            .unwrap()
            .completion_notified
    );
}

/// 【后台命令】【消费回归】验证清理完成后，迟到的刷新不会恢复旧任务。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无；清理后的任务再次出现时断言失败
#[tokio::test]
async fn background_poll_does_not_restore_cleaned_tasks() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    let mut child = waiting_child();
    seed_tasks(&store, child.id().unwrap());
    let mut config = AppConfig::default();
    config.tools.background_command_stop_grace_seconds = 1;
    let pending = poll_session_background_completions(&paths, &config, "session-1");
    tokio::pin!(pending);
    assert!(futures_util::poll!(&mut pending).is_pending());

    // 1. 【后台命令】【消费回归】模拟模型在完成通知到达前清理全部终态任务
    let mut cleanup_config = config.clone();
    cleanup_config.tools.background_command_stop_grace_seconds = 0;
    cleanup_background_tasks(serde_json::json!({}), &paths, &cleanup_config)
        .await
        .unwrap();
    assert!(store.load().unwrap().is_empty());
    let (notices, _) = pending.await.unwrap();
    child.wait().await.unwrap();

    assert!(notices.is_empty(), "已经清理的后台任务不能再次产生完成通知");
    assert!(store.load().unwrap().is_empty(), "旧刷新不能恢复已删除任务");
}

/// 【后台命令】【并发新增】验证状态刷新期间其他会话新建的任务不会消失。
///
/// 参数: 无
/// 返回: 无；并发新增记录被旧快照覆盖时断言失败
#[tokio::test]
async fn background_poll_preserves_concurrently_started_task() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    let mut child = waiting_child();
    seed_tasks(&store, child.id().unwrap());
    let mut config = AppConfig::default();
    config.tools.background_command_stop_grace_seconds = 1;
    let pending = poll_session_background_completions(&paths, &config, "session-1");
    tokio::pin!(pending);
    assert!(futures_util::poll!(&mut pending).is_pending());
    let mut added = store.load().unwrap()[0].clone();
    added.id = "new-task".to_string();
    added.runtime_owner_id = Some("session-2".to_string());
    added.status = "running".to_string();
    added.pid = std::process::id();
    added.timeout_seconds = 0;
    store.upsert(added).unwrap();
    pending.await.unwrap();
    child.wait().await.unwrap();
    assert!(store.load().unwrap().iter().any(|task| {
        task.id == "new-task" && task.status == "running" && task.owned_by_session("session-2")
    }));
}

/// 【后台命令】【输出保留】验证通知确认后刷新列表不会抢先删除模型待读日志。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无；确认通知后无法读取命令结果时断言失败
#[tokio::test]
async fn acknowledged_output_survives_background_listing() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let state = crate::state::StateStore::new(&paths).unwrap();
    state.init_files().unwrap();
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    store.init().unwrap();
    let stdout = store.logs_dir().join("retained.out");
    let stderr = store.logs_dir().join("retained.err");
    std::fs::write(&stdout, "completed output\n").unwrap();
    std::fs::write(&stderr, "").unwrap();
    let task: BackgroundCommandTask = serde_json::from_value(serde_json::json!({
        "id": "retained-task", "label": "completed", "command": "test", "cwd": ".",
        "pid": 0, "status": "exited", "stdout_log": stdout, "stderr_log": stderr,
        "started_at": 1, "updated_at": 2, "timeout_seconds": 0,
        "runtime_owner_kind": "session", "runtime_owner_id": state.session_id()
    }))
    .unwrap();
    store.upsert(task).unwrap();
    let config = AppConfig::default();
    acknowledge_background_completions(&paths, state.session_id(), &["retained-task".to_string()])
        .unwrap();
    let owner = super::background_tasks::BackgroundRuntimeOwner::session(state.session_id());
    super::background_tasks::list_background_tasks(&paths, &config, true, Some(&owner))
        .await
        .unwrap();
    let output = super::background_tasks::read_background_task_output(
        serde_json::json!({"task_id": "retained-task"}),
        &config,
        &paths,
    )
    .await
    .expect("确认通知后仍应允许模型读取输出");
    let body: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(body["stdout"], "completed output");
}

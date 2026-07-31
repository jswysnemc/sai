use super::*;
use crate::state::StateStore;
use crate::tools::command::test_support::isolated_test_paths;

/// 验证后台任务标识只保留安全字符并统一格式。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn sanitize_id_keeps_safe_subset() {
    assert_eq!(sanitize_id("Dev Server 01!"), "dev-server-01");
}

/// 验证交互会话 owner 定位失败时不会退回当前活动会话。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn session_owner_requires_exact_session_state() {
    let temp = tempfile::tempdir().unwrap();
    let paths = isolated_test_paths(temp.path().to_path_buf());
    let owner = BackgroundRuntimeOwner::session("missing-session");

    assert!(state_for_runtime_owner(&paths, Some(&owner)).is_err());
}

/// 验证网关后台任务可通过 owner 元数据和旧标签识别。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn gateway_owned_tasks_are_detected_by_owner_and_label() {
    let mut task = BackgroundCommandTask {
        id: "1".to_string(),
        runtime_process_id: None,
        runtime_owner_kind: Some("gateway".to_string()),
        runtime_owner_id: Some("qq".to_string()),
        runtime_process_kind: Some("gateway".to_string()),
        goal_id: None,
        label: "gateway:qq".to_string(),
        command: "sai gateway qq-bot".to_string(),
        cwd: ".".to_string(),
        pid: 100,
        pgid: None,
        status: "running".to_string(),
        stdout_log: "stdout.log".to_string(),
        stderr_log: "stderr.log".to_string(),
        started_at: 0,
        updated_at: 0,
        timeout_seconds: 0,
        completion_notified: false,
    };
    assert!(is_gateway_owned_task(&task));

    // 兼容旧记录：无 owner 元数据但 label 带 gateway: 前缀
    task.runtime_owner_kind = None;
    task.runtime_owner_id = None;
    task.runtime_process_kind = None;
    assert!(is_gateway_owned_task(&task));

    // 普通后台任务不应被识别为网关
    task.label = "dev server".to_string();
    assert!(!is_gateway_owned_task(&task));
}

/// 验证无限超时的运行中任务不会被状态刷新误判为结束。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[tokio::test]
async fn refresh_keeps_unlimited_running_task() {
    let mut tasks = vec![BackgroundCommandTask {
        id: "task-1".to_string(),
        runtime_process_id: None,
        runtime_owner_kind: None,
        runtime_owner_id: None,
        runtime_process_kind: None,
        goal_id: None,
        label: "server".to_string(),
        command: "sleep 9999".to_string(),
        cwd: ".".to_string(),
        pid: std::process::id(),
        pgid: None,
        status: "running".to_string(),
        stdout_log: "stdout.log".to_string(),
        stderr_log: "stderr.log".to_string(),
        started_at: 0,
        updated_at: 0,
        timeout_seconds: 0,
        completion_notified: false,
    }];

    refresh_task_statuses(&mut tasks, &AppConfig::default()).await;

    assert_eq!(tasks[0].status, "running");
}

/// 验证读取后台输出会记录运行事件并恢复输出上限状态。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[tokio::test]
async fn output_read_records_runtime_event_and_output_cap_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let paths = isolated_test_paths(temp.path().to_path_buf());
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    store.init().unwrap();
    let stdout_log = store.logs_dir().join("task-1.out.log");
    let stderr_log = store.logs_dir().join("task-1.err.log");
    std::fs::write(&stdout_log, "alpha\nbeta\ngamma\n").unwrap();
    std::fs::write(&stderr_log, "").unwrap();
    store
        .save(&[BackgroundCommandTask {
            id: "task-1".to_string(),
            runtime_process_id: Some("background_command_task-1".to_string()),
            runtime_owner_kind: None,
            runtime_owner_id: None,
            runtime_process_kind: None,
            goal_id: None,
            label: "server".to_string(),
            command: "printf lines".to_string(),
            cwd: ".".to_string(),
            pid: 123,
            pgid: Some(123),
            status: "exited".to_string(),
            stdout_log: stdout_log.display().to_string(),
            stderr_log: stderr_log.display().to_string(),
            started_at: 0,
            updated_at: 0,
            timeout_seconds: 0,
            completion_notified: false,
        }])
        .unwrap();
    let mut config = AppConfig::default();
    config.tools.background_command_log_max_bytes = 8;

    let response = read_background_task_output(
        json!({
            "task_id": "task-1",
            "stream": "stdout",
            "tail_lines": 10
        }),
        &config,
        &paths,
    )
    .await
    .unwrap();
    let body: Value = serde_json::from_str(&response).unwrap();
    let snapshot = StateStore::new(&paths)
        .unwrap()
        .session_snapshot(1_000)
        .unwrap();
    let failure = snapshot.runtime_recovery.latest_failure.unwrap();
    let db_path = crate::state::active_state_dir(&paths)
        .unwrap()
        .join("conversation.db");
    let conn = rusqlite::Connection::open(db_path).unwrap();
    let event_count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM runtime_process_events
             WHERE process_id = ?1
             AND stream = 'stdout'
             AND event_kind = 'output_read'",
            ["background_command_task-1"],
            |row| row.get(0),
        )
        .unwrap();

    assert!(body["stdout"].as_str().unwrap().contains("gamma"));
    assert_eq!(
        failure.kind,
        crate::runtime_recovery::RuntimeRecoveryKind::OutputCapReached
    );
    assert_eq!(
        failure.process_id.as_deref(),
        Some("background_command_task-1")
    );
    assert_eq!(failure.last_safe_seq, Some(1));
    assert_eq!(event_count, 1);
}

use super::background;
use super::background_wait::wait_background_task;
use super::store::{BackgroundCommandStore, BackgroundCommandTask};
use super::test_support::isolated_test_paths;
use crate::config::AppConfig;
use crate::tools::ToolRegistry;
use serde_json::Value;

/// 【后台命令】【等待回归】等待结束时返回仍运行的任务和日志，供主 Agent 判断。
#[tokio::test]
async fn regression_background_wait_returns_running_snapshot_and_logs() {
    let temp = tempfile::tempdir().unwrap();
    let paths = isolated_test_paths(temp.path().to_path_buf());
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    terminal_task(&store, "running");
    let mut tasks = store.load().unwrap();
    tasks[0].pid = std::process::id();
    tasks[0].timeout_seconds = 0;
    tasks[0].started_at = super::store::unix_seconds();
    std::fs::write(&tasks[0].stdout_log, "waiting for input\n").unwrap();
    store.save(&tasks).unwrap();
    let output = wait_background_task(
        serde_json::json!({"task_id": "task-1", "timeout_seconds": 1}),
        &AppConfig::default(),
        &paths,
        None,
    )
    .await
    .unwrap();
    let body: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(body["task"]["status"], "running");
    assert!(body["stdout"]
        .as_str()
        .unwrap()
        .contains("waiting for input"));
    assert_eq!(body["needs_attention"], true);
    assert!(!store.load().unwrap()[0].completion_notified);
}

/// 等待并刷新一个会话时，其他会话的后台任务必须保留且不可被选为等待结果。
#[tokio::test]
async fn background_wait_preserves_other_session_tasks() {
    let temp = tempfile::tempdir().unwrap();
    let paths = isolated_test_paths(temp.path().to_path_buf());
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    terminal_task(&store, "running");
    let mut own = store.load().unwrap().remove(0);
    own.timeout_seconds = 0;
    own.runtime_owner_kind = Some("session".into());
    own.runtime_owner_id = Some("session-one".into());
    let mut other = own.clone();
    other.id = "other-task".into();
    other.runtime_owner_id = Some("session-two".into());
    other.pid = std::process::id();
    store.save(&[own, other.clone()]).unwrap();
    let owner = super::background_tasks::BackgroundRuntimeOwner::session("session-one");
    let output = wait_background_task(
        serde_json::json!({"timeout_seconds": 1}),
        &AppConfig::default(),
        &paths,
        Some(&owner),
    )
    .await
    .unwrap();
    let body: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(body["task"]["id"], "task-1");
    let saved = store.load().unwrap();
    assert_eq!(saved.len(), 2);
    let retained = saved.iter().find(|task| task.id == other.id).unwrap();
    assert_eq!(retained.status, "running");
    assert_eq!(retained.updated_at, other.updated_at);
}

fn terminal_task(store: &BackgroundCommandStore, status: &str) {
    store.init().unwrap();
    let stdout_log = store.logs_dir().join("task.out.log");
    let stderr_log = store.logs_dir().join("task.err.log");
    std::fs::write(&stdout_log, "").unwrap();
    std::fs::write(&stderr_log, "").unwrap();
    store
        .save(&[BackgroundCommandTask {
            id: "task-1".to_string(),
            runtime_process_id: None,
            runtime_owner_kind: None,
            runtime_owner_id: None,
            runtime_process_kind: None,
            goal_id: None,
            label: "task".to_string(),
            command: "true".to_string(),
            cwd: ".".to_string(),
            pid: u32::MAX,
            pgid: None,
            status: status.to_string(),
            stdout_log: stdout_log.display().to_string(),
            stderr_log: stderr_log.display().to_string(),
            started_at: 0,
            updated_at: 1,
            timeout_seconds: 30,
            completion_notified: false,
        }])
        .unwrap();
}

/// 验证后台命令注册表接受 wait 动作。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn background_command_schema_accepts_wait_action() {
    let temp = tempfile::tempdir().unwrap();
    let paths = isolated_test_paths(temp.path().to_path_buf());
    let config = AppConfig::default();
    let arguments = r#"{"action":"wait","task_id":"task-1","timeout_seconds":1}"#;

    let mut writable_registry = ToolRegistry::new();
    background::register(&mut writable_registry, config.clone(), paths.clone(), true);
    assert!(writable_registry
        .validate_arguments("background_command", arguments)
        .is_ok());

    let mut readonly_registry = ToolRegistry::new();
    background::register_readonly(&mut readonly_registry, config, paths);
    assert!(readonly_registry
        .validate_arguments("background_command", arguments)
        .is_ok());
}

/// 验证 wait 对已经结束的任务立即返回状态，不再报 schema 错误。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[tokio::test]
async fn wait_returns_terminal_task() {
    let temp = tempfile::tempdir().unwrap();
    let paths = isolated_test_paths(temp.path().to_path_buf());
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    terminal_task(&store, "exited");

    let output = wait_background_task(
        serde_json::json!({"task_id": "task-1", "timeout_seconds": 1}),
        &AppConfig::default(),
        &paths,
        None,
    )
    .await
    .unwrap();
    let body: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(body["completed"], true);
    assert_eq!(body["task"]["status"], "exited");
}

/// 验证未指定任务 ID 时，wait 返回最先结束的初始运行任务。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[tokio::test]
async fn wait_without_task_id_returns_any_finished_task() {
    let temp = tempfile::tempdir().unwrap();
    let paths = isolated_test_paths(temp.path().to_path_buf());
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    store.init().unwrap();
    let first = store.logs_dir().join("first.log");
    let second = store.logs_dir().join("second.log");
    std::fs::write(&first, "").unwrap();
    std::fs::write(&second, "").unwrap();
    let task = |id: &str, pid: u32, output: &std::path::Path| BackgroundCommandTask {
        id: id.to_string(),
        runtime_process_id: None,
        runtime_owner_kind: None,
        runtime_owner_id: None,
        runtime_process_kind: None,
        goal_id: None,
        label: id.to_string(),
        command: "true".to_string(),
        cwd: ".".to_string(),
        pid,
        pgid: None,
        status: "running".to_string(),
        stdout_log: output.display().to_string(),
        stderr_log: output.display().to_string(),
        started_at: 0,
        updated_at: 1,
        timeout_seconds: 0,
        completion_notified: false,
    };
    store
        .save(&[
            task("long", std::process::id(), &first),
            task("short", i32::MAX as u32, &second),
        ])
        .unwrap();

    let output = wait_background_task(
        serde_json::json!({"timeout_seconds": 1}),
        &AppConfig::default(),
        &paths,
        None,
    )
    .await
    .unwrap();
    let body: Value = serde_json::from_str(&output).unwrap();

    assert_eq!(body["task"]["id"], "short");
}

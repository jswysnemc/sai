use super::*;
use crate::tools::command::{unix_seconds, BackgroundCommandStore, BackgroundCommandTask};

/// 【后台命令】【提醒测试】构造会话所属的安静后台任务。
///
/// 参数: `paths` 为隔离路径，`session` 为会话标识
/// 返回: 已保存任务及其存储
fn quiet_task(paths: &SaiPaths, session: &str) -> (BackgroundCommandTask, BackgroundCommandStore) {
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    store.init().unwrap();
    let log = store.logs_dir().join("quiet.log");
    std::fs::write(&log, "waiting for input\n").unwrap();
    let started_at = unix_seconds().saturating_sub(100);
    std::fs::File::options()
        .write(true)
        .open(&log)
        .unwrap()
        .set_times(
            std::fs::FileTimes::new()
                .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(started_at)),
        )
        .unwrap();
    let task = BackgroundCommandTask {
        id: "quiet-task".into(),
        runtime_process_id: None,
        runtime_owner_kind: Some("session".into()),
        runtime_owner_id: Some(session.into()),
        runtime_process_kind: Some("background_command".into()),
        goal_id: None,
        label: "quiet build".into(),
        command: "build".into(),
        cwd: ".".into(),
        pid: std::process::id(),
        pgid: None,
        status: "running".into(),
        stdout_log: log.display().to_string(),
        stderr_log: String::new(),
        started_at,
        updated_at: started_at,
        timeout_seconds: 0,
        completion_notified: false,
    };
    store.save(&[task.clone()]).unwrap();
    (task, store)
}

/// 长时间无输出的无限时任务必须唤醒空闲会话，而不能永远停留在 Waiting。
#[tokio::test]
async fn regression_background_silence_wakes_the_session() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let state = StateStore::new(&paths).unwrap();
    state.init_files().unwrap();
    let (_, store) = quiet_task(&paths, state.session_id());
    let monitor = ExternalEventMonitor {
        paths,
        state,
        config: AppConfig::default(),
    };
    let polled = monitor.poll_once().await.unwrap();
    let ExternalEventPoll::Ready(ExternalEventWake::Completion(batch)) = polled else {
        panic!("安静后台任务应提醒主 Agent 检查进展");
    };
    assert!(batch.prompt().contains("quiet-task"));
    assert!(!batch.prompt().contains("以下后台工作已经结束"));
    assert!(!store.load().unwrap()[0].completion_notified);
}

/// 检查提醒确认后进入冷却，但同一任务结束时仍能收到独立完成回执。
#[tokio::test]
async fn background_attention_ack_preserves_final_completion_notice() {
    use crate::tools::command::{
        acknowledge_background_attention, poll_background_attention,
        poll_session_background_completions,
    };
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let (mut task, store) = quiet_task(&paths, "session-one");
    let initial = poll_background_attention(&paths, "session-one", None).unwrap();
    assert_eq!(initial.len(), 1);
    acknowledge_background_attention(&paths, "session-one", &[task.id.clone()]).unwrap();
    assert!(poll_background_attention(&paths, "session-one", None)
        .unwrap()
        .is_empty());
    assert!(!store.load().unwrap()[0].completion_notified);
    task.status = "exited".into();
    store.save(&[task.clone()]).unwrap();
    let (finished, running) =
        poll_session_background_completions(&paths, &AppConfig::default(), "session-one")
            .await
            .unwrap();
    assert_eq!(running, 0);
    assert_eq!(finished.len(), 1);
    assert!(!store.load().unwrap()[0].completion_notified);
}

/// 运行中提醒和确认均限制在所属会话，Goal 与普通会话的提醒互不串入。
#[test]
fn background_attention_is_scoped_to_session_and_goal() {
    use crate::tools::command::{acknowledge_background_attention, poll_background_attention};
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let (mut task, store) = quiet_task(&paths, "session-one");
    assert!(poll_background_attention(&paths, "session-two", None)
        .unwrap()
        .is_empty());
    acknowledge_background_attention(&paths, "session-two", &[task.id.clone()]).unwrap();
    assert_eq!(
        poll_background_attention(&paths, "session-one", None)
            .unwrap()
            .len(),
        1
    );
    task.goal_id = Some("goal-one".into());
    store.save(&[task]).unwrap();
    assert!(poll_background_attention(&paths, "session-one", None)
        .unwrap()
        .is_empty());
    assert!(
        poll_background_attention(&paths, "session-one", Some("goal-two"))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        poll_background_attention(&paths, "session-one", Some("goal-one"))
            .unwrap()
            .len(),
        1
    );
}

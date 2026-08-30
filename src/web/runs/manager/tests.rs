use super::*;
use crate::web::runs::checkpoint::RUN_HISTORY_CAPACITY;
use std::path::PathBuf;

/// 创建运行管理器测试路径。
///
/// 参数:
/// - `root`: 测试状态根目录
///
/// 返回:
/// - 隔离的 Sai 路径集合
fn test_paths(root: PathBuf) -> SaiPaths {
    SaiPaths {
        config_dir: root.join("config"),
        config_file: root.join("config/config.jsonc"),
        secrets_file: root.join("config/secrets.jsonc"),
        skills_dir: root.join("config/skills"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        state_dir: root.join("state"),
        pictures_dir: root.join("pictures"),
        fish_hook_file: root.join("fish/sai.fish"),
        bash_hook_file: root.join("shell/bash-hook.sh"),
        zsh_hook_file: root.join("shell/zsh-hook.zsh"),
        powershell_hook_file: root.join("shell/powershell-hook.ps1"),
    }
}

/// 创建指定状态的运行检查点。
///
/// 参数:
/// - `root`: 工作区目录
/// - `run_id`: 运行标识
/// - `status`: 运行状态
///
/// 返回:
/// - 测试运行检查点
fn test_checkpoint(
    root: &std::path::Path,
    run_id: &str,
    status: RunCheckpointStatus,
) -> RunCheckpoint {
    RunCheckpoint {
        info: ActiveRunInfo {
            run_id: run_id.to_string(),
            workspace_id: "workspace".to_string(),
            session_id: format!("session-{run_id}"),
            input: String::new(),
            image_urls: Vec::new(),
            status,
            discard_user_turn: false,
            restore_input: None,
        },
        workspace: WorkspaceInfo {
            id: "workspace".to_string(),
            name: "workspace".to_string(),
            path: root.display().to_string(),
            last_opened_at: String::new(),
        },
        request: StartRunRequest {
            kind: RunKind::Conversation,
            session_id: format!("session-{run_id}"),
            input: String::new(),
            agent_id: None,
            image_url: None,
            image_urls: Vec::new(),
            mode: None,
            provider_id: None,
            model: None,
            thinking_level: None,
        },
        status,
        updated_at: String::new(),
    }
}

/// 验证会话事件日志不随运行检查点淘汰：淘汰运行后仍能回读同一会话的事件。
#[tokio::test]
async fn keeps_session_journals_beyond_terminal_history_capacity() {
    let temp = tempfile::tempdir().unwrap();
    let manager = RunManager::new(&test_paths(temp.path().to_path_buf())).unwrap();
    for index in 0..=RUN_HISTORY_CAPACITY {
        let run_id = format!("run-{index}");
        manager
            .checkpoints
            .upsert(test_checkpoint(
                temp.path(),
                &run_id,
                RunCheckpointStatus::Completed,
            ))
            .unwrap();
    }
    // 最早的运行已经被检查点淘汰
    assert!(manager.checkpoints.get("run-0").is_none());
    let bus = manager.session_bus("workspace", "session-run-0").await;
    let journal = bus.journal();
    let _ = bus.emit(WebEvent::new(
        "run-0",
        "workspace",
        "session-run-0",
        "status.changed",
        json!({ "status": "working" }),
    ));
    let replayed = wait_for_events(&journal, "run-0").await;
    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].kind, "status.changed");
}

/// 验证同一会话的多个前端共享同一份事件日志。
#[tokio::test]
async fn session_bus_shares_one_journal_across_frontends() {
    let temp = tempfile::tempdir().unwrap();
    let manager = RunManager::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let first = manager.session_bus("workspace", "session").await;
    let second = manager.session_bus("workspace", "session").await;
    assert!(manager
        .buses
        .read()
        .await
        .entries
        .contains_key("workspace:session"));

    let _ = first.emit(WebEvent::new(
        "run-a",
        "workspace",
        "session",
        "status.changed",
        json!({ "status": "working" }),
    ));

    let shared = wait_for_events(&second.journal(), "run-a").await;
    assert_eq!(shared.len(), 1);
    assert_eq!(shared[0].payload["status"], "working");
}

/// 验证会话事件日志在重新打开后仍能按序号补发。
#[tokio::test]
async fn session_journal_replays_events_after_sequence() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    let manager = RunManager::new(&paths).unwrap();
    let bus = manager.session_bus("workspace", "session").await;
    for index in 0..3 {
        let _ = bus.emit(WebEvent::new(
            "run-1",
            "workspace",
            "session",
            "message.content.delta",
            json!({ "text": index.to_string() }),
        ));
    }
    let published = wait_for_events(&bus.journal(), "run-1").await;
    assert_eq!(published.len(), 3);

    // 丢弃内存状态，仅从磁盘恢复
    let reopened = RunManager::new(&paths).unwrap();
    let restored = reopened.session_bus("workspace", "session").await.journal();
    let backlog = restored.events_after(1);
    assert_eq!(backlog.len(), 2);
    assert_eq!(backlog[0].payload["text"], "1");
    assert!(restored.events_after(3).is_empty());
}

/// 验证会话历史清理后再次订阅会从空日志重新开始。
#[tokio::test]
async fn session_history_removal_resets_the_event_stream() {
    let temp = tempfile::tempdir().unwrap();
    let manager = RunManager::new(&test_paths(temp.path().to_path_buf())).unwrap();
    manager
        .checkpoints
        .upsert(test_checkpoint(
            temp.path(),
            "run-1",
            RunCheckpointStatus::Completed,
        ))
        .unwrap();
    let bus = manager.session_bus("workspace", "session-run-1").await;
    let _ = bus.emit(WebEvent::new(
        "run-1",
        "workspace",
        "session-run-1",
        "run.completed",
        json!({}),
    ));
    assert_eq!(wait_for_events(&bus.journal(), "run-1").await.len(), 1);
    drop(bus);

    manager
        .remove_session_history("workspace", "session-run-1")
        .await
        .unwrap();

    let recreated = manager.session_bus("workspace", "session-run-1").await;
    assert!(recreated.journal().events_after(0).is_empty());
}

/// 等待事件总线把入队事件落到会话日志上。
async fn wait_for_events(journal: &EventJournal, run_id: &str) -> Vec<WebEvent> {
    for _ in 0..100 {
        let events = journal
            .events_after(0)
            .into_iter()
            .filter(|event| event.run_id == run_id)
            .collect::<Vec<_>>();
        if !events.is_empty() {
            return events;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    Vec::new()
}

/// 验证同一会话的第二个前端加入后，仍能收到此后发生的事件。
#[tokio::test]
async fn late_watcher_receives_events_published_after_it_attaches() {
    let temp = tempfile::tempdir().unwrap();
    let manager = RunManager::new(&test_paths(temp.path().to_path_buf())).unwrap();
    manager
        .checkpoints
        .upsert(test_checkpoint(
            temp.path(),
            "run-recovered",
            RunCheckpointStatus::Running,
        ))
        .unwrap();
    // 1. 第一个前端经运行标识定位到所属会话
    let bus = manager.run_bus("run-recovered").await.unwrap();
    let mut first = bus.attach().unwrap();
    // 2. 第二个前端在同一会话上单独订阅
    let mut second = manager
        .run_bus("run-recovered")
        .await
        .unwrap()
        .attach()
        .unwrap();

    let _ = bus.emit(WebEvent::new(
        "run-recovered",
        "workspace",
        "session-run-recovered",
        "status.changed",
        json!({ "status": "working" }),
    ));

    let first_event = tokio::time::timeout(std::time::Duration::from_secs(1), first.events.recv())
        .await
        .unwrap()
        .unwrap();
    let second_event = tokio::time::timeout(std::time::Duration::from_secs(1), second.events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first_event.kind, "status.changed");
    assert_eq!(second_event.kind, "status.changed");
    assert_eq!(first_event.sequence, second_event.sequence);
}

#[tokio::test]
async fn removes_session_checkpoints_and_journals_together() {
    let temp = tempfile::tempdir().unwrap();
    let manager = RunManager::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let run_id = "run-history";
    let event_path = manager.session_event_path("workspace:session");
    manager
        .checkpoints
        .upsert(RunCheckpoint {
            info: ActiveRunInfo {
                run_id: run_id.to_string(),
                workspace_id: "workspace".to_string(),
                session_id: "session".to_string(),
                input: String::new(),
                image_urls: Vec::new(),
                status: RunCheckpointStatus::Completed,
                discard_user_turn: false,
                restore_input: None,
            },
            workspace: WorkspaceInfo {
                id: "workspace".to_string(),
                name: "workspace".to_string(),
                path: temp.path().display().to_string(),
                last_opened_at: String::new(),
            },
            request: StartRunRequest {
                kind: RunKind::Conversation,
                session_id: "session".to_string(),
                input: String::new(),
                agent_id: None,
                image_url: None,
                image_urls: Vec::new(),
                mode: None,
                provider_id: None,
                model: None,
                thinking_level: None,
            },
            status: RunCheckpointStatus::Completed,
            updated_at: String::new(),
        })
        .unwrap();
    let bus = manager.session_bus("workspace", "session").await;
    let _ = bus.emit(WebEvent::new(
        run_id,
        "workspace",
        "session",
        "run.completed",
        json!({}),
    ));
    assert!(wait_for_events(&bus.journal(), run_id).await.len() == 1);

    manager
        .remove_session_history("workspace", "session")
        .await
        .unwrap();

    assert!(manager.checkpoints.get(run_id).is_none());
    assert!(manager.run_bus(run_id).await.is_none());
    assert!(!event_path.exists());
    assert!(manager.buses.read().await.entries.is_empty());
}

/// 验证同一会话的第二次提交会进入持久化队列。
#[tokio::test]
async fn queues_second_submission_for_same_session() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    let manager = RunManager::new(&paths).unwrap();
    let workspace = WorkspaceInfo {
        id: "workspace".to_string(),
        name: "workspace".to_string(),
        path: temp.path().display().to_string(),
        last_opened_at: String::new(),
    };
    let key = session_key(&workspace.id, "session");
    let task = tokio::spawn(std::future::pending::<()>());
    manager.active.lock().await.insert(
        key,
        ActiveRun {
            info: ActiveRunInfo {
                run_id: "running".to_string(),
                workspace_id: workspace.id.clone(),
                session_id: "session".to_string(),
                input: "first".to_string(),
                image_urls: Vec::new(),
                status: RunCheckpointStatus::Running,
                discard_user_turn: false,
                restore_input: None,
            },
            handle: task,
            cancel_requested: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        },
    );

    let queued = manager
        .start(
            workspace,
            StartRunRequest {
                kind: RunKind::Conversation,
                session_id: "session".to_string(),
                input: "second".to_string(),
                agent_id: None,
                image_url: None,
                image_urls: Vec::new(),
                mode: None,
                provider_id: None,
                model: None,
                thinking_level: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(queued.status, RunCheckpointStatus::Queued);
    assert_eq!(
        manager.checkpoints.get(&queued.run_id).unwrap().status,
        RunCheckpointStatus::Queued
    );
}

/// 验证队列编辑和排序同时更新内存与持久化检查点。
#[tokio::test]
async fn updates_queued_submission_and_restores_new_order() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    let manager = RunManager::new(&paths).unwrap();
    let workspace = WorkspaceInfo {
        id: "workspace".to_string(),
        name: "workspace".to_string(),
        path: temp.path().display().to_string(),
        last_opened_at: String::new(),
    };
    let key = session_key(&workspace.id, "session");
    manager.active.lock().await.insert(
        key.clone(),
        ActiveRun {
            info: ActiveRunInfo {
                run_id: "running".to_string(),
                workspace_id: workspace.id.clone(),
                session_id: "session".to_string(),
                input: "active".to_string(),
                image_urls: Vec::new(),
                status: RunCheckpointStatus::Running,
                discard_user_turn: false,
                restore_input: None,
            },
            handle: tokio::spawn(std::future::pending::<()>()),
            cancel_requested: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        },
    );

    let mut queued_ids = Vec::new();
    for input in ["first", "second", "third"] {
        let info = manager
            .start(
                workspace.clone(),
                StartRunRequest {
                    kind: RunKind::Conversation,
                    session_id: "session".to_string(),
                    input: input.to_string(),
                    agent_id: None,
                    image_url: None,
                    image_urls: Vec::new(),
                    mode: None,
                    provider_id: None,
                    model: None,
                    thinking_level: None,
                },
            )
            .await
            .unwrap();
        queued_ids.push(info.run_id);
    }

    let moved_id = queued_ids[2].clone();
    manager
        .update_queued(
            &moved_id,
            QueuedRunUpdate {
                input: Some("edited third".to_string()),
                position: Some(0),
            },
        )
        .await
        .unwrap();

    let expected = vec![
        moved_id.clone(),
        queued_ids[0].clone(),
        queued_ids[1].clone(),
    ];
    let in_memory = manager
        .queued
        .lock()
        .await
        .get(&key)
        .unwrap()
        .iter()
        .map(|run| run.info.run_id.clone())
        .collect::<Vec<_>>();
    assert_eq!(in_memory, expected);
    assert_eq!(
        manager.checkpoints.get(&moved_id).unwrap().request.input,
        "edited third"
    );

    let empty_error = manager
        .update_queued(
            &queued_ids[0],
            QueuedRunUpdate {
                input: Some("   ".to_string()),
                position: None,
            },
        )
        .await
        .unwrap_err();
    assert!(empty_error.to_string().contains("message cannot be empty"));
    assert!(manager
        .update_queued(
            "running",
            QueuedRunUpdate {
                input: None,
                position: Some(0),
            },
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("queued run not found"));

    drop(manager);
    let restored = RunManager::new(&paths).unwrap();
    let restored_ids = restored
        .queued
        .lock()
        .await
        .get(&key)
        .unwrap()
        .iter()
        .map(|run| run.info.run_id.clone())
        .collect::<Vec<_>>();
    assert_eq!(restored_ids, expected);
    assert_eq!(
        restored.checkpoints.get(&moved_id).unwrap().info.input,
        "edited third"
    );
}

/// 【消息队列】【间隙投递】验证排队消息逐条合并并保留后续队首。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[tokio::test]
async fn message_queue_acknowledges_only_the_delivered_front_item() {
    let temp = tempfile::tempdir().unwrap();
    let manager = RunManager::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let workspace = WorkspaceInfo {
        id: "workspace".to_string(),
        name: "workspace".to_string(),
        path: temp.path().display().to_string(),
        last_opened_at: String::new(),
    };
    let key = session_key(&workspace.id, "session");
    manager.active.lock().await.insert(
        key.clone(),
        ActiveRun {
            info: ActiveRunInfo {
                run_id: "active-run".to_string(),
                workspace_id: workspace.id.clone(),
                session_id: "session".to_string(),
                input: "active".to_string(),
                image_urls: Vec::new(),
                status: RunCheckpointStatus::Running,
                discard_user_turn: false,
                restore_input: None,
            },
            handle: tokio::spawn(std::future::pending::<()>()),
            cancel_requested: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        },
    );

    let mut queued = Vec::new();
    for input in ["first queued", "second queued"] {
        queued.push(
            manager
                .start(
                    workspace.clone(),
                    StartRunRequest {
                        kind: RunKind::Conversation,
                        session_id: "session".to_string(),
                        input: input.to_string(),
                        agent_id: None,
                        image_url: None,
                        image_urls: Vec::new(),
                        mode: None,
                        provider_id: None,
                        model: None,
                        thinking_level: None,
                    },
                )
                .await
                .unwrap(),
        );
    }
    let source = WebMessageQueue::new(manager.clone(), key.clone(), "active-run".to_string());

    let first = source.peek().await.unwrap().unwrap();
    assert_eq!(first.id, queued[0].run_id);
    assert_eq!(first.display, "first queued");
    source.acknowledge(&first.id).await.unwrap();

    assert_eq!(
        manager.checkpoints.get(&first.id).unwrap().status,
        RunCheckpointStatus::Completed
    );
    let first_events =
        wait_for_events(&manager.run_bus(&first.id).await.unwrap().journal(), &first.id).await;
    assert!(first_events.iter().any(|event| {
        event.kind == "run.merged" && event.payload["target_run_id"] == "active-run"
    }));
    let second = source.peek().await.unwrap().unwrap();
    assert_eq!(second.id, queued[1].run_id);
    assert_eq!(second.display, "second queued");
    assert_eq!(manager.queued.lock().await[&key].len(), 1);

    source.acknowledge(&second.id).await.unwrap();
    assert!(source.peek().await.unwrap().is_none());
    manager
        .active
        .lock()
        .await
        .remove(&key)
        .unwrap()
        .handle
        .abort();
}

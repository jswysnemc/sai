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

#[tokio::test]
async fn keeps_only_recent_run_journals() {
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
        manager.insert_journal(run_id, EventJournal::new()).await;
    }
    assert!(manager.journal("run-0").await.is_none());
    assert!(manager
        .journal(&format!("run-{RUN_HISTORY_CAPACITY}"))
        .await
        .is_some());
}

#[tokio::test]
async fn keeps_active_journals_beyond_terminal_history_capacity() {
    let temp = tempfile::tempdir().unwrap();
    let manager = RunManager::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let first_journal = EventJournal::new();
    for index in 0..=RUN_HISTORY_CAPACITY {
        let run_id = format!("run-{index}");
        manager
            .checkpoints
            .upsert(test_checkpoint(
                temp.path(),
                &run_id,
                RunCheckpointStatus::Running,
            ))
            .unwrap();
        let journal = if index == 0 {
            first_journal.clone()
        } else {
            EventJournal::new()
        };
        manager.insert_journal(run_id, journal).await;
    }
    let shared = manager.journal("run-0").await.unwrap();
    let mut receiver = shared.subscribe();

    first_journal.publish(WebEvent::new(
        "run-0",
        "workspace",
        "session-run-0",
        "status.changed",
        json!({ "status": "working" }),
    ));

    let event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.kind, "status.changed");
}

#[tokio::test]
async fn journal_recovery_reuses_one_broadcast_channel() {
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
    let first = manager.journal("run-recovered").await.unwrap();
    let mut receiver = first.subscribe();
    let second = manager.journal("run-recovered").await.unwrap();

    second.publish(WebEvent::new(
        "run-recovered",
        "workspace",
        "session-run-recovered",
        "status.changed",
        json!({ "status": "working" }),
    ));

    let event = tokio::time::timeout(std::time::Duration::from_secs(1), receiver.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.kind, "status.changed");
}

#[tokio::test]
async fn removes_session_checkpoints_and_journals_together() {
    let temp = tempfile::tempdir().unwrap();
    let manager = RunManager::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let run_id = "run-history";
    let event_path = manager.checkpoints.event_path(run_id);
    let journal = EventJournal::persistent(event_path.clone());
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
    manager
        .insert_journal(run_id.to_string(), journal.clone())
        .await;
    journal.publish(WebEvent::new(
        run_id,
        "workspace",
        "session",
        "run.completed",
        json!({}),
    ));

    manager
        .remove_session_history("workspace", "session")
        .await
        .unwrap();

    assert!(manager.checkpoints.get(run_id).is_none());
    assert!(manager.journal(run_id).await.is_none());
    assert!(!event_path.exists());
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

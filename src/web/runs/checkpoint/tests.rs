use super::*;
use crate::web::runs::RunKind;
use std::path::PathBuf;

/// 创建运行检查点测试路径。
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

#[test]
/// 验证服务重启后的无回复输入只能消费一次。
fn interruption_recovery_is_consumed_once() {
    let temp = tempfile::tempdir().unwrap();
    let store = RunCheckpointStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let info = ActiveRunInfo {
        run_id: "run-1".to_string(),
        workspace_id: "workspace".to_string(),
        session_id: "session".to_string(),
        input: "edit me".to_string(),
        image_urls: Vec::new(),
        status: RunCheckpointStatus::Running,
        discard_user_turn: false,
        restore_input: None,
        insert_at: crate::web::runs::QueueInsertAt::Turn,
    };
    store
        .upsert(RunCheckpoint {
            info: info.clone(),
            workspace: WorkspaceInfo {
                id: info.workspace_id.clone(),
                name: "workspace".to_string(),
                path: temp.path().display().to_string(),
                last_opened_at: String::new(),
            },
            request: StartRunRequest {
                kind: RunKind::Conversation,
                session_id: info.session_id.clone(),
                input: info.input.clone(),
                agent_id: None,
                image_url: None,
                image_urls: Vec::new(),
                mode: None,
                provider_id: None,
                model: None,
                thinking_level: None,
                insert_at: crate::web::runs::QueueInsertAt::Turn,
            },
            status: RunCheckpointStatus::Running,
            updated_at: String::new(),
        })
        .unwrap();
    store
        .update_interruption("run-1", true, Some("edit me".to_string()))
        .unwrap();

    let first = store
        .take_interruption_recovery("workspace", "session")
        .unwrap();
    let second = store
        .take_interruption_recovery("workspace", "session")
        .unwrap();

    assert_eq!(first.unwrap().restore_input.as_deref(), Some("edit me"));
    assert!(second.is_none());
}

/// 创建终态检查点测试数据。
///
/// 参数:
/// - `root`: 工作区目录
/// - `run_id`: 运行标识
///
/// 返回:
/// - 完成状态的检查点
fn completed_checkpoint(root: &std::path::Path, run_id: &str) -> RunCheckpoint {
    RunCheckpoint {
        info: ActiveRunInfo {
            run_id: run_id.to_string(),
            workspace_id: "workspace".to_string(),
            session_id: "session".to_string(),
            input: String::new(),
            image_urls: Vec::new(),
            status: RunCheckpointStatus::Completed,
            discard_user_turn: false,
            restore_input: None,
            insert_at: crate::web::runs::QueueInsertAt::Turn,
        },
        workspace: WorkspaceInfo {
            id: "workspace".to_string(),
            name: "workspace".to_string(),
            path: root.display().to_string(),
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
            insert_at: crate::web::runs::QueueInsertAt::Turn,
        },
        status: RunCheckpointStatus::Completed,
        updated_at: String::new(),
    }
}

#[test]
fn prunes_old_terminal_checkpoints_and_their_journals() {
    let temp = tempfile::tempdir().unwrap();
    let store = RunCheckpointStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    for index in 0..=RUN_HISTORY_CAPACITY {
        let run_id = format!("run-{index}");
        let event_path = store.event_path(&run_id);
        std::fs::create_dir_all(event_path.parent().unwrap()).unwrap();
        std::fs::write(&event_path, "event\n").unwrap();
        store
            .upsert(completed_checkpoint(temp.path(), &run_id))
            .unwrap();
    }

    assert!(store.get("run-0").is_none());
    assert!(!store.event_path("run-0").exists());
    assert!(store.get(&format!("run-{RUN_HISTORY_CAPACITY}")).is_some());
    assert!(store
        .event_path(&format!("run-{RUN_HISTORY_CAPACITY}"))
        .exists());
}

#[test]
fn prunes_interrupted_checkpoints_with_pending_restore_input() {
    let temp = tempfile::tempdir().unwrap();
    let store = RunCheckpointStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    for index in 0..=RUN_HISTORY_CAPACITY {
        let run_id = format!("run-{index}");
        let mut checkpoint = completed_checkpoint(temp.path(), &run_id);
        checkpoint.status = RunCheckpointStatus::Interrupted;
        checkpoint.info.status = RunCheckpointStatus::Interrupted;
        checkpoint.info.discard_user_turn = true;
        checkpoint.info.restore_input = Some(format!("restore-{index}"));
        store.upsert(checkpoint).unwrap();
    }

    assert!(store.get("run-0").is_none());
    assert!(store.get("run-1").is_some());
    assert!(store.get(&format!("run-{RUN_HISTORY_CAPACITY}")).is_some());
}

#[test]
fn terminal_checkpoints_discard_input_and_image_payloads() {
    let temp = tempfile::tempdir().unwrap();
    let store = RunCheckpointStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let mut checkpoint = completed_checkpoint(temp.path(), "run-large");
    checkpoint.info.input = "large input".to_string();
    checkpoint.info.image_urls = vec!["data:image/png;base64,AAAA".to_string()];
    checkpoint.request.input = "large input".to_string();
    checkpoint.request.image_url = Some("data:image/png;base64,AAAA".to_string());
    checkpoint.request.image_urls = vec!["data:image/png;base64,BBBB".to_string()];

    store.upsert(checkpoint).unwrap();

    let stored = store.get("run-large").unwrap();
    assert!(stored.info.input.is_empty());
    assert!(stored.info.image_urls.is_empty());
    assert!(stored.request.input.is_empty());
    assert!(stored.request.image_url.is_none());
    assert!(stored.request.image_urls.is_empty());
}

#[test]
fn startup_removes_orphan_event_journals() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    let orphan = paths.state_dir.join("web/run-events/orphan.jsonl");
    std::fs::create_dir_all(orphan.parent().unwrap()).unwrap();
    std::fs::write(&orphan, "event\n").unwrap();

    let _store = RunCheckpointStore::new(&paths).unwrap();

    assert!(!orphan.exists());
}

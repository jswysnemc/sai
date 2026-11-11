use super::*;
use crate::web::runs::manager::{tests::test_checkpoint, QueueInsertAt, QueuedRun};

/// 【Web队列】【独立模式】无参数；验证权限或工具方式不同的排队项不会混入当前任务。
#[tokio::test]
async fn request_gap_preserves_distinct_permission_and_tool_modes() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(root.path());
    let manager = RunManager::new(&paths).unwrap();
    let mut active = test_checkpoint(root.path(), "active", RunCheckpointStatus::Running);
    active.request.mode = Some("yolo+code-model".into());
    manager.checkpoints.upsert(active).unwrap();
    for mode in [
        "yolo+native",
        "yolo+code-model-only",
        "plan+code-model",
        "plan+code-model-only",
    ] {
        let mut pending = test_checkpoint(root.path(), "pending", RunCheckpointStatus::Queued);
        pending.request.mode = Some(mode.into());
        pending.request.input = "independent task".into();
        pending.request.insert_at = QueueInsertAt::Request;
        pending.info.insert_at = QueueInsertAt::Request;
        manager.checkpoints.upsert(pending.clone()).unwrap();
        manager.queued.lock().await.insert(
            "scope".into(),
            [QueuedRun {
                info: pending.info,
                workspace: pending.workspace,
                request: pending.request,
            }]
            .into_iter()
            .collect(),
        );
        let source = WebMessageQueue::new(manager.clone(), "scope".into(), "active".into());
        assert!(
            source
                .peek_for_mode(crate::agent::AgentMode::Yolo.with_codemode(true))
                .await
                .unwrap()
                .is_none(),
            "mode {mode} was merged"
        );
        assert_eq!(
            manager.checkpoints.get("pending").unwrap().status,
            RunCheckpointStatus::Queued
        );
        manager
            .queued
            .lock()
            .await
            .get_mut("scope")
            .unwrap()
            .front_mut()
            .unwrap()
            .request
            .mode = Some("yolo+code-model".into());
        assert!(source
            .peek_for_mode(crate::agent::AgentMode::Yolo.with_codemode(true))
            .await
            .unwrap()
            .is_some());
        assert!(source
            .peek_for_mode(crate::agent::AgentMode::Plan.with_codemode(true))
            .await
            .unwrap()
            .is_none());
        manager
            .queued
            .lock()
            .await
            .get_mut("scope")
            .unwrap()
            .front_mut()
            .unwrap()
            .request
            .mode = Some("plan+code-model".into());
        assert!(source
            .peek_for_mode(crate::agent::AgentMode::Plan.with_codemode(true))
            .await
            .unwrap()
            .is_some());
        assert!(source
            .peek_for_mode(crate::agent::AgentMode::Audited.with_codemode(true))
            .await
            .unwrap()
            .is_none());
    }
}

/// 【Web队列】【默认策略】无参数；验证入队固定配置策略，显式普通模式优先且重启保持组合。
#[tokio::test]
async fn queued_runs_pin_configured_or_explicit_tool_modes() {
    use crate::config::{AppConfig, CodeMode};
    for (configured, requested, expected) in [
        (CodeMode::On, "yolo", "yolo+code-model"),
        (CodeMode::Only, "audited", "audited+code-model-only"),
        (CodeMode::Off, "plan", "plan+native"),
        (CodeMode::Only, "yolo+native", "yolo+native"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let paths = crate::paths::SaiPaths::for_tests(root.path());
        let mut config = AppConfig::default();
        config.codemode.mode = configured;
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(&paths.config_file, serde_json::to_vec(&config).unwrap()).unwrap();
        let manager = RunManager::new(&paths).unwrap();
        let pending = test_checkpoint(root.path(), "pending", RunCheckpointStatus::Queued);
        let key =
            crate::web::runs::manager::session_key(&pending.workspace.id, &pending.info.session_id);
        manager.queued.lock().await.insert(
            key,
            [QueuedRun {
                info: pending.info.clone(),
                workspace: pending.workspace.clone(),
                request: pending.request.clone(),
            }]
            .into_iter()
            .collect(),
        );
        let mut request = pending.request;
        request.input = "queued task".into();
        request.mode = Some(requested.into());
        let queued = manager.start(pending.workspace, request).await.unwrap();
        assert_eq!(queued.status, RunCheckpointStatus::Queued);
        assert_eq!(
            manager
                .checkpoints
                .get(&queued.run_id)
                .unwrap()
                .request
                .mode
                .as_deref(),
            Some(expected)
        );
        config.codemode.mode = CodeMode::Off;
        std::fs::write(&paths.config_file, serde_json::to_vec(&config).unwrap()).unwrap();
        let reopened = RunManager::new(&paths).unwrap();
        assert_eq!(
            reopened
                .checkpoints
                .get(&queued.run_id)
                .unwrap()
                .request
                .mode
                .as_deref(),
            Some(expected)
        );
    }
}

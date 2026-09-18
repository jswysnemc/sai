use super::*;

/// 归属隔离的默认行为：投给别的会话必须被拒绝。
///
/// 这是网格工具的安全底线——默认开放等于任何 agent 都能往任意会话注入消息。
#[tokio::test]
async fn send_outside_the_session_is_rejected_by_default() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (local, _) = session_in(&paths, &cwd, "local");
        let (other, _) = session_in(&paths, &cwd, "other");

        let error = registry_with_cross_session(&paths, &local, false)
            .call(
                "mesh_send",
                &format!(r#"{{"to":"session:{}","text":"hi"}}"#, other.id),
            )
            .await
            .unwrap_err();

        assert!(
            error.to_string().contains("outside this session"),
            "cross-session send must be rejected by default: {error}"
        );
    })
    .await;
}

/// 显式开启 `mesh.cross_session` 后允许投递到别的会话。
#[tokio::test]
async fn send_outside_the_session_is_allowed_when_cross_session_is_enabled() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (local, _) = session_in(&paths, &cwd, "local");
        let (other, _) = session_in(&paths, &cwd, "other");

        let output = registry_with_cross_session(&paths, &local, true)
            .call(
                "mesh_send",
                &format!(r#"{{"to":"session:{}","text":"hi"}}"#, other.id),
            )
            .await
            .unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["ok"], true, "{parsed}");
        assert!(parsed["correlation_id"].as_str().is_some(), "{parsed}");
    })
    .await;
}

/// 白名单 Agent（如"代码 Agent"的 enabled_tools）过滤注册表后，
/// `mesh.cross_session` 开关必须仍然生效：过滤若把会话归属与开关重置，
/// 即使配置了 true 也会被权限策略拦下。
#[tokio::test]
async fn cross_session_survives_the_agent_whitelist_filter() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (local, _) = session_in(&paths, &cwd, "local");
        let (other, _) = session_in(&paths, &cwd, "other");
        let state_dir = locate_session_dirs(&paths, &local.id).unwrap().1;

        // 白名单配置等价于"代码 Agent"的 enabled_tools，并显式开启跨会话
        let mut config = crate::config::AppConfig::default();
        config.mesh.cross_session = true;
        config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
            enabled_tools: vec!["mesh_send".to_string()],
            exclusive: false,
            deferred_tools: Vec::new(),
            skills_full: Vec::new(),
            skills_named: Vec::new(),
        });

        // 与真实路径一致：先注册（含会话归属），再过白名单过滤，最后绑定权限配置
        let mut registry = ToolRegistry::new();
        super::super::register(
            &mut registry,
            paths.clone(),
            state_dir.display().to_string(),
            local.id.clone(),
            config.mesh.cross_session,
        );
        registry.set_session_ownership(
            state_dir.display().to_string(),
            local.id.clone(),
            config.mesh.cross_session,
        );
        let mut filtered = crate::runner::submission_tools::apply_enabled_tools_filter(
            registry,
            &config,
            crate::runner::SubmissionSource::Repl,
        )
        .unwrap();
        filtered.set_permission_profile(crate::permission::PermissionProfile::new(
            crate::permission::PermissionProfileMode::Yolo,
            cwd.clone(),
            None,
        ));

        let output = filtered
            .call(
                "mesh_send",
                &format!(r#"{{"to":"session:{}","text":"hi"}}"#, other.id),
            )
            .await
            .unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["ok"], true, "{parsed}");
    })
    .await;
}

/// 会话内投递：mesh_send 之后进入会话队列待投递，确认后不再重复注入。
#[tokio::test]
async fn send_queues_an_active_receipt_without_recv() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, state_dir) = session_in(&paths, &cwd, "local");
        let registry = registry_with_cross_session(&paths, &session, false);
        assert!(
            !registry.contains("mesh_recv"),
            "mesh_recv must not be registered"
        );

        let sent: Value = serde_json::from_str(
            &registry
                .call(
                    "mesh_send",
                    &format!(r#"{{"to":"session:{}","text":"ping"}}"#, session.id),
                )
                .await
                .unwrap(),
        )
        .unwrap();
        let correlation_id = sent["correlation_id"].as_str().unwrap().to_string();

        let pending = super::super::next_pending(&state_dir, &session.id)
            .expect("sent message should be queued as an active receipt");
        assert_eq!(
            pending.correlation_id.as_deref(),
            Some(correlation_id.as_str())
        );
        assert_eq!(pending.text, "ping");

        super::super::acknowledge_mesh_messages(&state_dir, std::slice::from_ref(&pending.id));
        assert!(
            super::super::next_pending(&state_dir, &session.id).is_none(),
            "acknowledged mesh messages must not be re-delivered"
        );
    })
    .await;
}

/// 投递成功后立即返回，不再阻塞等待回复。
#[tokio::test]
async fn mesh_send_returns_without_waiting_for_a_reply() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let cwd = temp.path().join("workspace");
    std::fs::create_dir_all(&cwd).unwrap();

    crate::runtime_cwd::scope(cwd.clone(), async {
        let (session, _) = session_in(&paths, &cwd, "local");

        let output = registry_with_cross_session(&paths, &session, false)
            .call(
                "mesh_send",
                &format!(r#"{{"to":"session:{}","text":"ping"}}"#, session.id),
            )
            .await
            .unwrap();
        let parsed: Value = serde_json::from_str(&output).unwrap();

        assert_eq!(parsed["ok"], true, "{parsed}");
        assert!(parsed["correlation_id"].as_str().is_some());
        assert!(parsed.get("timed_out").is_none());
        assert!(parsed.get("reply").is_none());
        assert!(parsed.get("expect_reply").is_none());
    })
    .await;
}

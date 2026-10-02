use super::*;

/// 【计划模式】【测试会话】参数为隔离路径与初始模式，返回不调用模型的真实 Agent。
fn agent(paths: &SaiPaths, mode: AgentMode) -> Agent {
    let mut config = AppConfig::default();
    config.skills.enabled = false;
    config.load_instruction_files = false;
    let state = StateStore::new(paths).unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, paths).unwrap();
    Agent::new(config, paths, state, client, ToolRegistry::new(), mode).unwrap()
}

/// 【计划模式】【审批回归】计划全文持久化并展示，批准后恢复模式和共享句柄；无参数和返回值。
#[tokio::test]
async fn approved_plan_restores_execution_and_shared_mode() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut agent = agent(&paths, AgentMode::Audited);
    let handle = agent.live_mode_handle();
    let entered = agent
        .execute_plan_tool("enter_plan_mode", "{}", &mut |_| Ok(()))
        .await
        .unwrap();
    assert!(!entered.failed);
    assert_eq!(agent.mode(), AgentMode::Plan);
    let plan = "# Implementation\n\nExplore files.\n\n".repeat(100);
    let arguments = serde_json::json!({"title":"Implementation","plan":plan}).to_string();
    let execution = agent
        .execute_plan_tool("exit_plan_mode", &arguments, &mut |event| {
            if let AgentEvent::QuestionRequested(pending) = event {
                assert_eq!(pending.plan.as_deref(), Some(plan.trim()));
                crate::question::resolve_question(
                    &pending.id,
                    crate::question::QuestionResponse::Answered(vec![vec![
                        crate::plan::APPROVE.into()
                    ]]),
                )?;
            }
            Ok(())
        })
        .await
        .unwrap();
    assert!(!execution.failed, "{}", execution.output);
    assert_eq!(agent.mode(), AgentMode::Audited);
    assert!(std::sync::Arc::ptr_eq(&handle, &agent.live_mode_handle()));
    assert!(agent.tools.contains("write_file"));
    let saved = crate::plan::store::load(agent.state.state_dir())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.status, "approved");
    assert_eq!(saved.plan, plan.trim());
}

/// 【计划模式】【反馈回归】取消与修改意见不退出只读模式，也不会返回批准结果；无参数和返回值。
#[tokio::test]
async fn cancelled_or_revised_plan_stays_readonly() {
    for answer in [None, Some("请补充数据库迁移步骤")] {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let mut agent = agent(&paths, AgentMode::Plan);
        let result = agent
            .execute_plan_tool(
                "exit_plan_mode",
                r##"{"title":"Design","plan":"# Plan\n\nSteps and verification"}"##,
                &mut |event| {
                    if let AgentEvent::QuestionRequested(pending) = event {
                        let response =
                            answer.map_or(crate::question::QuestionResponse::Cancelled, |value| {
                                crate::question::QuestionResponse::Answered(vec![
                                    vec![value.into()],
                                ])
                            });
                        crate::question::resolve_question(&pending.id, response)?;
                    }
                    Ok(())
                },
            )
            .await
            .unwrap();
        assert!(!result.failed);
        assert_eq!(agent.mode(), AgentMode::Plan);
        assert!(result.output.contains("\"approved\":false"));
    }
}

/// 【计划模式】【参数回归】非计划模式和空计划不能触发审批；无参数和返回值。
#[tokio::test]
async fn invalid_plan_never_requests_approval() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut agent = agent(&paths, AgentMode::Yolo);
    for mode in [AgentMode::Yolo, AgentMode::Plan] {
        agent.apply_live_mode(mode);
        let execution = agent
            .execute_plan_tool(
                "exit_plan_mode",
                r#"{"title":"Empty","plan":" "}"#,
                &mut |_| panic!("invalid plan must not request approval"),
            )
            .await
            .unwrap();
        assert!(execution.failed);
        assert_eq!(agent.mode(), mode);
    }
}

/// 【计划模式】【过期审批】等待期间模式已改变时，旧审批不能再次改变权限；无参数和返回值。
#[tokio::test]
async fn stale_plan_approval_cannot_override_current_mode() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut agent = agent(&paths, AgentMode::Plan);
    let handle = agent.live_mode_handle();
    let execution = agent
        .execute_plan_tool(
            "exit_plan_mode",
            r#"{"title":"Design","plan":"Plan details"}"#,
            &mut |event| {
                if let AgentEvent::QuestionRequested(pending) = event {
                    handle.store(
                        AgentMode::AutoAudit.as_u8(),
                        std::sync::atomic::Ordering::SeqCst,
                    );
                    crate::question::resolve_question(
                        &pending.id,
                        crate::question::QuestionResponse::Answered(vec![vec![
                            crate::plan::APPROVE.into(),
                        ]]),
                    )?;
                }
                Ok(())
            },
        )
        .await
        .unwrap();
    assert!(!execution.failed);
    assert!(execution.output.contains("\"approved\":false"));
    assert_eq!(agent.mode(), AgentMode::AutoAudit);
}

/// 【计划模式】【目录升级】旧非独占白名单保留规划入口，独占白名单不扩权；无参数和返回值。
#[test]
fn plan_tools_survive_legacy_profile_without_expanding_exclusive_profile() {
    for exclusive in [false, true] {
        let mut registry = ToolRegistry::new();
        crate::tools::register_ask_question(&mut registry);
        let mut config = AppConfig::default();
        config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
            enabled_tools: vec!["read_file".into()],
            exclusive,
            ..Default::default()
        });
        let filtered = crate::runner::apply_enabled_tools_filter(
            registry,
            &config,
            crate::runner::SubmissionSource::Web,
        )
        .unwrap();
        assert_eq!(filtered.contains("enter_plan_mode"), !exclusive);
        assert_eq!(filtered.contains("exit_plan_mode"), !exclusive);
    }
}

use super::*;

/// 【上下文压缩】【消息续接】压缩后发送两轮消息，后续请求必须保留新问题、回答和图片。
/// @returns 无；使用隔离状态及实际 Agent 请求组装，无网络请求
#[test]
fn messages_after_compaction_remain_in_followup_requests() {
    verify_followup_requests(false);
}

/// 【上下文压缩】【自动续接】覆盖自动触发与重复压缩后的正式请求。
/// @returns 无；使用同一消息续接断言
#[test]
fn messages_after_auto_compaction_remain_in_followup_requests() {
    verify_followup_requests(true);
}

/// 【上下文压缩】【请求验证】按指定触发方式压缩并验证新消息、图片和回答。
/// @param automatic 是否通过上下文预算自动触发
/// @returns 无；断言失败时测试终止
fn verify_followup_requests(automatic: bool) {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.prompt_sections.state_contract = false;
    config.prompt_sections.mode_reminder = false;
    config.skills.enabled = false;
    config.load_instruction_files = false;
    let state = StateStore::new(&paths).unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
    let mut agent = Agent::new(
        config,
        &paths,
        state,
        client,
        ToolRegistry::new(),
        AgentMode::Yolo,
    )
    .unwrap();
    for index in 0..4 {
        let id = format!("old-{index}");
        agent.state.start_turn(&id, "old question").unwrap();
        agent.state.complete_turn(&id, "old answer", None).unwrap();
    }
    for cycle in 0..2 {
        let compact = if automatic {
            agent.state.select_compaction_for_messages(
                &[ChatMessage::plain(
                    "user",
                    "context pressure ".repeat(2_000),
                )],
                1_000,
                false,
            )
        } else {
            agent.state.select_manual_compaction(0)
        }
        .unwrap()
        .expect("应存在可压缩轮次");
        agent
            .state
            .apply_compaction(&compact, "previous context summary")
            .unwrap();
        let first_id = format!("new-{cycle}");
        let input = format!("new question after summary {cycle}");
        let answer = format!("new answer after summary {cycle}");
        let images = vec!["data:image/png;base64,aW1hZ2U=".to_string()];
        agent
            .state
            .start_turn_with_images(&first_id, &input, &images)
            .unwrap();
        let first = agent
            .chat_messages_for_turn(
                &first_id,
                &input,
                &images,
                None,
                Some("<system-reminder>new tool exposure</system-reminder>"),
            )
            .unwrap();
        assert!(serde_json::to_string(first.last().unwrap())
            .unwrap()
            .contains(&input));
        agent.state.complete_turn(&first_id, &answer, None).unwrap();
        let next_id = format!("followup-{cycle}");
        agent
            .state
            .start_turn(&next_id, "continue with that answer")
            .unwrap();
        let next = agent
            .chat_messages_for_turn(&next_id, "continue with that answer", &[], None, None)
            .unwrap();
        let serialized = serde_json::to_string(&next).unwrap();
        assert!(
            serialized.contains(&input),
            "带注入前缀的新用户消息必须进入下一次请求"
        );
        assert_eq!(serialized.matches(&input).count(), 1, "新用户消息不得重复");
        assert!(serialized.contains(&answer), "新助手回答必须进入下一次请求");
        assert!(serialized.contains(&images[0]), "新消息图片必须保留");
        agent
            .state
            .complete_turn(&next_id, "continued", None)
            .unwrap();
    }
}

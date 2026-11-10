use super::*;

/// 【目标】【流式提交】完整粘贴目标命令在运行期间立即生效，不进入聊天队列。
/// 参数: 无；返回: 无，命令误发或附件丢失时断言失败。
#[test]
fn goal_whole_paste_updates_bound_session_during_current_turn() {
    let temp = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(temp.path());
    let ctx =
        StreamCommandContext::capture(&paths, temp.path().display().to_string(), AgentMode::Yolo);
    let mut runtime = ReplRuntime::new(
        5000,
        crate::render::transcript::TranscriptRenderOptions {
            reasoning_mode: crate::render::ReasoningDisplayMode::Summary,
            tool_call_mode: crate::render::ToolCallDisplayMode::Summary,
        },
    );
    let command = format!("/goal {} --tokens 5000", "完整目标需求\n".repeat(30));
    let draft = runtime.stream_draft_mut();
    draft
        .clipboard
        .paste_text_into_input(&mut draft.text, &mut draft.cursor, command.clone());

    assert_eq!(
        dispatch_stream_command(&mut runtime, &ctx).unwrap(),
        StreamInputAction::Continue
    );
    assert!(runtime.stream_draft().text.is_empty());
    assert!(runtime.queued_control_commands().is_empty());
    assert!(runtime.take_submission_queue().is_empty());
    let store = crate::goal::GoalStore::new(temp.path().join("goal.json"));
    let goal = store.get().unwrap().unwrap();
    assert_eq!(goal.objective, "完整目标需求\n".repeat(30).trim());
    assert_eq!(goal.token_budget, Some(5000));
    for (command, expected) in [
        ("/goal pause", crate::goal::GoalStatus::Paused),
        ("/goal resume", crate::goal::GoalStatus::Active),
    ] {
        runtime.stream_draft_mut().text = command.into();
        dispatch_stream_command(&mut runtime, &ctx).unwrap();
        assert_eq!(store.get().unwrap().unwrap().status, expected);
    }
    runtime.stream_draft_mut().text = "/goal edit revised objective".into();
    dispatch_stream_command(&mut runtime, &ctx).unwrap();
    let edited = store.get().unwrap().unwrap();
    assert_eq!(edited.objective, "revised objective");
    assert_eq!(edited.id, goal.id);
    assert_eq!(edited.token_budget, goal.token_budget);
    assert!(runtime.take_submission_queue().is_empty());
}

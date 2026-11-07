use super::*;

/// 【目标】【流式提交】完整粘贴命令仍遵循运行期间禁用策略，不进入聊天队列。
/// 参数: 无；返回: 无，命令误发或附件丢失时断言失败。
#[test]
fn goal_whole_paste_keeps_draft_until_current_turn_finishes() {
    let temp = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(temp.path());
    let ctx = StreamCommandContext::capture(&paths, "goal-test".into(), AgentMode::Yolo);
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
    let marker = draft.text.clone();
    assert_eq!(
        dispatch_stream_command(&mut runtime, &ctx).unwrap(),
        StreamInputAction::Continue
    );
    assert_eq!(runtime.stream_draft().text, marker);
    assert!(runtime.queued_control_commands().is_empty());
    assert!(runtime.take_submission_queue().is_empty());
    let draft = runtime.stream_draft();
    assert_eq!(
        draft.clipboard.echo_text_for_submit(&draft.text).text,
        command
    );

    // 1. 【目标】【控制队列】显式入队时仍按展开后的命令分类
    assert!(runtime.enqueue_stream_draft(AgentMode::Yolo).unwrap());
    assert!(runtime.take_submission_queue().is_empty());
    assert_eq!(runtime.take_next_control_command(), Some(command));
}

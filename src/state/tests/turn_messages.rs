use super::{test_paths, StateStore};
use crate::llm::ChatContent;
use crate::state::turn_messages::{NewTurnMessage, TurnMessageKind};

/// 【会话历史】【消息间隙】验证间隙消息按工具边界持久化并恢复。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn persists_and_projects_messages_inside_one_turn() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    let store = StateStore::new(&paths).unwrap();
    let session_id = store.session_id().to_string();
    store.start_turn("turn-1", "开始任务").unwrap();
    store
        .record_tool_call_started_with_context(
            "turn-1",
            1,
            1,
            Some("先执行工具"),
            "call-1",
            "read_file",
            "{}",
        )
        .unwrap();
    store
        .record_tool_result_completed("turn-1", "call-1", true, "工具结果", None, None, 4)
        .unwrap();
    store
        .record_turn_message(NewTurnMessage {
            turn_id: "turn-1".to_string(),
            after_tool_seq: 1,
            kind: TurnMessageKind::Assistant,
            model_content: "等待后台结果".to_string(),
            display_content: "等待后台结果".to_string(),
            reasoning: None,
            image_urls: Vec::new(),
        })
        .unwrap();
    store
        .record_turn_message(NewTurnMessage {
            turn_id: "turn-1".to_string(),
            after_tool_seq: 1,
            kind: TurnMessageKind::ExternalCompletion,
            model_content: "<external-completion-events>done</external-completion-events>"
                .to_string(),
            display_content: "子智能体已完成".to_string(),
            reasoning: None,
            image_urls: Vec::new(),
        })
        .unwrap();
    store.complete_turn("turn-1", "最终答复", None).unwrap();
    drop(store);

    let reopened = StateStore::for_session(&paths, &session_id).unwrap();
    let projection = reopened.project_history(None).unwrap();
    let roles = projection
        .messages
        .iter()
        .map(|message| message.role.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        roles,
        vec![
            "user",
            "assistant",
            "tool",
            "assistant",
            "user",
            "assistant"
        ]
    );
    assert!(matches!(
        projection.messages[4].content.as_ref(),
        Some(ChatContent::Text(content)) if content.contains("external-completion-events")
    ));
    let persisted = reopened.turn_messages("turn-1").unwrap();
    assert_eq!(persisted.len(), 2);
    assert_eq!(persisted[1].display_content, "子智能体已完成");
}

/// 【会话历史】【消息交付】验证失败请求可以撤销尚未确认的间隙消息。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn removes_an_uncommitted_turn_message() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn-1", "开始任务").unwrap();
    let message = store
        .record_turn_message(NewTurnMessage {
            turn_id: "turn-1".to_string(),
            after_tool_seq: 0,
            kind: TurnMessageKind::QueuedUser,
            model_content: "追加要求".to_string(),
            display_content: "追加要求".to_string(),
            reasoning: None,
            image_urls: Vec::new(),
        })
        .unwrap();

    assert!(store.remove_turn_message(&message.id).unwrap());
    assert!(store.turn_messages("turn-1").unwrap().is_empty());
}

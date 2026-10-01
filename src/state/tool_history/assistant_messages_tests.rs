use super::*;
use crate::llm::{ToolCall, ToolCallFunction};
use crate::paths::SaiPaths;
use crate::state::turn_messages::{NewTurnMessage, TurnMessageKind};
use serde_json::json;

/// 【助手历史】【回归夹具】构造含正文、思考与原始块的工具回复；无参数，返回完整结果。
fn tool_result() -> ChatResult {
    ChatResult {
        content: "准备读取".into(),
        reasoning: Some("先检查文件".into()),
        provider_content: Some(ProviderAssistantContent::Anthropic(vec![
            json!({"type":"thinking","thinking":"先检查文件","signature":"opaque-signature"}),
            json!({"type":"text","text":"准备读取"}),
            json!({"type":"tool_use","id":"call-1","name":"read_file","input":{"path":"a"}}),
        ])),
        tool_calls: vec![ToolCall {
            id: "call-1".into(),
            kind: "function".into(),
            function: ToolCallFunction {
                name: "read_file".into(),
                arguments: "{\"path\":\"a\"}".into(),
            },
        }],
        usage: None,
        duration_ms: 0,
        ttft_ms: 0,
    }
}

/// 【助手历史】【跨轮回归】完整子轮与隐藏提醒持久化后原样重放；无参数或返回值。
#[test]
fn replays_assistant_content_and_reminders_after_reopening() {
    let directory = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(directory.path());
    let state = StateStore::new(&paths).unwrap();
    let session = state.session_id().to_owned();
    state.start_turn("turn", "读取").unwrap();
    let result = tool_result();
    state
        .save_assistant_message("turn", AssistantMessageKey::Tool("call-1"), &result)
        .unwrap();
    state
        .record_tool_call_started("turn", 1, "call-1", "read_file", "{\"path\":\"a\"}")
        .unwrap();
    state
        .record_tool_result_completed(
            "turn",
            "call-1",
            true,
            crate::state::tool_history::ToolResultOutput {
                result_preview: "file",
                result_ref: None,
                error: None,
                original_chars: 4,
            },
        )
        .unwrap();
    state
        .record_turn_message(NewTurnMessage {
            turn_id: "turn".into(),
            after_tool_seq: 1,
            kind: TurnMessageKind::ContextReminder,
            model_content: "<system-reminder>pending todo</system-reminder>".into(),
            display_content: String::new(),
            reasoning: None,
            image_urls: vec![],
        })
        .unwrap();
    state.complete_turn("turn", "完成", None).unwrap();
    drop(state);
    let state = StateStore::for_session(&paths, &session).unwrap();
    let projected = state.project_history(None).unwrap().messages;
    let assistant = projected
        .iter()
        .find(|message| message.tool_calls.is_some())
        .unwrap();
    assert!(matches!(&assistant.content, Some(ChatContent::Text(text)) if text == &result.content));
    assert_eq!(assistant.provider_content, result.provider_content);
    assert_eq!(assistant.reasoning_content, result.reasoning);
    let reminder_index = projected.iter().position(|message| matches!(&message.content, Some(ChatContent::Text(text)) if text.contains("pending todo"))).unwrap();
    assert_eq!(projected[reminder_index - 1].role, "tool");
    assert_eq!(projected[reminder_index].role, "user");
    assert!(state.session_timeline(10).unwrap()[0].messages.is_empty());
    assert!(!serde_json::to_string(assistant)
        .unwrap()
        .contains("opaque-signature"));
}

/// 【助手历史】【兼容回归】没有新原文记录时沿用旧数据，删除轮次时清理原文；无参数或返回值。
#[test]
fn supports_legacy_rows_and_cascades_turn_deletion() {
    let directory = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(directory.path());
    let state = StateStore::new(&paths).unwrap();
    state.start_turn("turn", "旧消息").unwrap();
    state.complete_turn("turn", "旧回复", None).unwrap();
    assert!(state.project_history(None).unwrap().messages.iter().any(
        |message| matches!(&message.content, Some(ChatContent::Text(text)) if text == "旧回复")
    ));
    state
        .save_assistant_message("turn", AssistantMessageKey::Final, &tool_result())
        .unwrap();
    state
        .conv_db
        .conn
        .lock()
        .unwrap()
        .execute("DELETE FROM turns WHERE turn_id = 'turn'", [])
        .unwrap();
    assert!(load(&state.conv_db, "turn").unwrap().0.is_empty());
}

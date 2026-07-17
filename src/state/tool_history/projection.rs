use super::repository::load_tool_exchanges_for_turn;
use crate::llm::{ChatMessage, ToolCall, ToolCallFunction};
use crate::state::tool_history::project_legacy_tool_report_messages;
use crate::state::turns::{Turn, TurnStatus};
use crate::state::ConversationDb;
use anyhow::Result;

impl crate::state::StateStore {
    /// 重建当前运行轮次已经完成的工具调用与结果消息。
    ///
    /// 参数:
    /// - `turn_id`: 当前运行轮次标识
    ///
    /// 返回:
    /// - 不含重复用户消息和最终占位助手消息的工具交换消息
    pub(crate) fn project_running_turn_tool_messages(
        &self,
        turn_id: &str,
    ) -> Result<Vec<ChatMessage>> {
        let Some(turn) = self
            .conv_db
            .load_turns()?
            .into_iter()
            .find(|turn| turn.turn_id == turn_id)
        else {
            return Ok(Vec::new());
        };
        let mut messages =
            project_turn_messages_with_tool_history(&self.conv_db, &self.session_id, &[turn])?;
        if messages
            .first()
            .is_some_and(|message| message.role == "user")
        {
            messages.remove(0);
        }
        if messages.last().is_some_and(|message| {
            message.role == "assistant" && message.tool_calls.as_ref().is_none_or(Vec::is_empty)
        }) {
            messages.pop();
        }
        Ok(messages)
    }
}

/// 从 tail turns 和工具历史构造 provider 历史消息。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `turns`: checkpoint 后仍保留的轮次
///
/// 返回:
/// - provider 可直接发送的历史消息
pub(in crate::state) fn project_turn_messages_with_tool_history(
    db: &ConversationDb,
    session_id: &str,
    turns: &[Turn],
) -> Result<Vec<ChatMessage>> {
    let mut messages = Vec::new();
    for turn in turns {
        append_turn_messages(db, session_id, turn, &mut messages)?;
    }
    Ok(messages)
}

/// 追加单个轮次的 provider 消息。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `turn`: 待投影轮次
/// - `messages`: 输出消息列表
///
/// 返回:
/// - 追加是否成功
fn append_turn_messages(
    db: &ConversationDb,
    session_id: &str,
    turn: &Turn,
    messages: &mut Vec<ChatMessage>,
) -> Result<()> {
    messages.push(ChatMessage::plain("user", turn.user_content.clone()));
    let exchanges = load_tool_exchanges_for_turn(db, session_id, &turn.turn_id)?;
    if exchanges.is_empty() {
        append_assistant_context_messages(turn, messages);
        append_interrupted_turn_marker(turn, messages);
        return Ok(());
    }
    let tool_calls = exchanges
        .iter()
        .map(|exchange| ToolCall {
            id: exchange.call.provider_call_id.clone(),
            kind: "function".to_string(),
            function: ToolCallFunction {
                name: exchange.call.tool_name.clone(),
                arguments: exchange.call.arguments.clone(),
            },
        })
        .collect::<Vec<_>>();
    messages.push(ChatMessage::assistant("", Some(tool_calls)));
    for exchange in exchanges {
        messages.push(ChatMessage::tool(
            exchange.call.provider_call_id.clone(),
            tool_result_content(&exchange),
        ));
    }
    append_assistant_context_messages(turn, messages);
    append_interrupted_turn_marker(turn, messages);
    Ok(())
}

/// 追加助手最终回复和旧工具报告。
///
/// 参数:
/// - `turn`: 待投影轮次
/// - `messages`: 输出消息列表
///
/// 返回:
/// - 无
fn append_assistant_context_messages(turn: &Turn, messages: &mut Vec<ChatMessage>) {
    if !turn.assistant_content.trim().is_empty() {
        messages.push(ChatMessage::plain(
            "assistant",
            turn.assistant_content.clone(),
        ));
    }
    messages.extend(project_legacy_tool_report_messages(&turn.tool_reports));
}

/// 为中断轮次追加模型可见的稳定边界。
///
/// 参数:
/// - `turn`: 待投影轮次
/// - `messages`: 输出消息列表
///
/// 返回:
/// - 无
fn append_interrupted_turn_marker(turn: &Turn, messages: &mut Vec<ChatMessage>) {
    if turn.status != TurnStatus::Interrupted {
        return;
    }
    messages.push(ChatMessage::plain(
        "user",
        "<turn_aborted>\nThe user interrupted the previous turn on purpose. Tools or commands may have partially executed. Do not repeat them unless the user explicitly requests a retry.\n</turn_aborted>",
    ));
}

/// 构造 provider 可见工具结果内容。
///
/// 参数:
/// - `exchange`: 工具调用交换记录
///
/// 返回:
/// - provider 可见工具结果文本
fn tool_result_content(exchange: &super::model::ToolExchangeRecord) -> String {
    if let Some(replacement) = &exchange.replacement {
        return replacement.replacement.clone();
    }
    if let Some(result) = &exchange.result {
        return result.result_preview.clone();
    }
    match exchange.call.status {
        super::model::ToolCallStatus::Interrupted => {
            "tool error: tool call was interrupted before a result was recorded. Do not retry unless the user explicitly asks.".to_string()
        }
        _ => {
            "tool error: tool result is missing from durable history. Do not retry unless the user explicitly asks.".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tool_history::repository::{
        insert_tool_call, insert_tool_result, upsert_tool_output_replacement,
    };
    use crate::state::tool_history::schema::create_tool_history_tables;
    use crate::state::tool_history::{
        NewToolCallRecord, NewToolOutputReplacement, NewToolResultRecord,
    };
    use crate::state::turns::TurnStatus;

    fn db() -> (tempfile::TempDir, ConversationDb) {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        let conn = db.conn.lock().unwrap();
        create_tool_history_tables(&conn).unwrap();
        drop(conn);
        (temp, db)
    }

    fn turn() -> Turn {
        Turn {
            turn_id: "turn_1".to_string(),
            seq: 1,
            user_content: "read file".to_string(),
            user_timestamp: "2026-01-01T00:00:00Z".to_string(),
            assistant_content: "done".to_string(),
            assistant_reasoning: None,
            assistant_timestamp: Some("2026-01-01T00:00:01Z".to_string()),
            status: TurnStatus::Completed,
            tool_reports: Vec::new(),
        }
    }

    #[test]
    fn projects_tool_calls_and_reuses_replacement() {
        let (_temp, db) = db();
        insert_tool_call(
            &db,
            NewToolCallRecord {
                session_id: "default".to_string(),
                turn_id: "turn_1".to_string(),
                seq: 1,
                provider_call_id: "call_1".to_string(),
                tool_name: "read_file".to_string(),
                arguments: "{\"path\":\"a\"}".to_string(),
            },
        )
        .unwrap();
        insert_tool_result(
            &db,
            NewToolResultRecord {
                session_id: "default".to_string(),
                turn_id: "turn_1".to_string(),
                provider_call_id: "call_1".to_string(),
                ok: true,
                result_preview: "preview".to_string(),
                result_ref: Some("tool-results/call_1.txt".to_string()),
                error: None,
                original_chars: 100,
            },
        )
        .unwrap();
        upsert_tool_output_replacement(
            &db,
            NewToolOutputReplacement {
                provider_call_id: "call_1".to_string(),
                session_id: "default".to_string(),
                replacement: "stable preview".to_string(),
                original_chars: 100,
                result_ref: "tool-results/call_1.txt".to_string(),
                policy: "context_clip".to_string(),
            },
        )
        .unwrap();

        let messages = project_turn_messages_with_tool_history(&db, "default", &[turn()]).unwrap();

        assert_eq!(messages.len(), 4);
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].tool_calls.as_ref().unwrap()[0].id, "call_1");
        assert_eq!(messages[2].role, "tool");
        assert!(matches!(
            messages[2].content.as_ref(),
            Some(crate::llm::ChatContent::Text(text)) if text == "stable preview"
        ));
        assert_eq!(messages[3].role, "assistant");
    }

    #[test]
    fn rebuilds_running_turn_without_duplicate_user_or_pending_assistant() {
        let (_temp, db) = db();
        db.start_turn("turn_1", "inspect").unwrap();
        insert_tool_call(
            &db,
            NewToolCallRecord {
                session_id: "default".to_string(),
                turn_id: "turn_1".to_string(),
                seq: 1,
                provider_call_id: "call_1".to_string(),
                tool_name: "read_file".to_string(),
                arguments: "{}".to_string(),
            },
        )
        .unwrap();
        insert_tool_result(
            &db,
            NewToolResultRecord {
                session_id: "default".to_string(),
                turn_id: "turn_1".to_string(),
                provider_call_id: "call_1".to_string(),
                ok: true,
                result_preview: "result".to_string(),
                result_ref: None,
                error: None,
                original_chars: 6,
            },
        )
        .unwrap();
        let store = crate::state::StateStore {
            base_state_dir: _temp.path().to_path_buf(),
            session_id: "default".to_string(),
            state_dir: _temp.path().to_path_buf(),
            conv_db: std::sync::Arc::new(db),
        };

        let messages = store.project_running_turn_tool_messages("turn_1").unwrap();

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "assistant");
        assert_eq!(messages[1].role, "tool");
    }
}

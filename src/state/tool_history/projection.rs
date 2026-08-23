use super::repository::load_tool_exchanges_for_turn;
use crate::llm::{ChatMessage, ToolCall, ToolCallFunction};
use crate::state::tool_history::project_legacy_tool_report_messages;
use crate::state::turn_messages::{load_turn_messages_for_turn, TurnMessageRecord};
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
        // 已被摘要覆盖的工具调用不再回放，否则轮次内压缩不会产生任何节省
        let skip_calls = self
            .running_turn_compaction_boundary()?
            .filter(|(compacted_turn_id, _)| compacted_turn_id == turn_id)
            .map(|(_, calls)| calls)
            .unwrap_or_default();
        let mut messages = project_turn_messages_with_tool_history_skipping(
            &self.conv_db,
            &self.session_id,
            &[turn],
            skip_calls,
        )?;
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
    project_turn_messages_with_tool_history_skipping(db, session_id, turns, 0)
}

/// 构造 provider 历史消息，并跳过已被摘要覆盖的工具调用。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `turns`: 待投影轮次
/// - `skip_calls`: 从最早开始跳过的工具调用条数
///
/// 返回:
/// - provider 可直接发送的历史消息
pub(in crate::state) fn project_turn_messages_with_tool_history_skipping(
    db: &ConversationDb,
    session_id: &str,
    turns: &[Turn],
    skip_calls: usize,
) -> Result<Vec<ChatMessage>> {
    let mut messages = Vec::new();
    for turn in turns {
        append_turn_messages(db, session_id, turn, skip_calls, &mut messages)?;
    }
    Ok(messages)
}

/// 追加单个轮次的 provider 消息。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `turn`: 待投影轮次
/// - `skip_calls`: 从最早开始跳过的工具调用条数
/// - `messages`: 输出消息列表
///
/// 返回:
/// - 追加是否成功
fn append_turn_messages(
    db: &ConversationDb,
    session_id: &str,
    turn: &Turn,
    skip_calls: usize,
    messages: &mut Vec<ChatMessage>,
) -> Result<()> {
    let provider_user_content = db.provider_user_content(&turn.turn_id, &turn.user_content)?;
    let user_message = if turn.user_image_urls.is_empty() {
        ChatMessage::plain("user", provider_user_content)
    } else {
        ChatMessage::user_with_images(provider_user_content, turn.user_image_urls.clone())
    };
    messages.push(user_message);
    let exchanges = load_tool_exchanges_for_turn(db, session_id, &turn.turn_id)?;
    let mut inter_messages = load_turn_messages_for_turn(db, &turn.turn_id)?;
    // 跳过已压缩部分时必须落在 assistant 子轮边界上，否则会留下孤立的 tool 结果
    let exchanges = skip_compacted_exchanges(&exchanges, skip_calls);
    if skip_calls > 0 {
        inter_messages.retain(|message| message.after_tool_seq > skip_calls);
    }
    if exchanges.is_empty() {
        append_turn_inter_messages(&inter_messages, messages);
        append_assistant_context_messages(turn, messages);
        append_interrupted_turn_marker(turn, messages);
        return Ok(());
    }
    append_tool_exchange_messages(exchanges, &inter_messages, messages);
    append_assistant_context_messages(turn, messages);
    append_interrupted_turn_marker(turn, messages);
    Ok(())
}

/// 跳过已被摘要覆盖的工具交换，并把切点对齐到子轮边界。
///
/// provider 要求 assistant 的 tool_calls 与后续 tool 结果一一配对。若切点落在
/// 子轮中间，剩余部分会以孤立的 tool 结果开头，请求直接被拒。这里把切点前移到
/// 下一个完整子轮的起点。
///
/// 参数:
/// - `exchanges`: 该轮次的全部工具交换，按发生顺序
/// - `skip_calls`: 期望跳过的条数
///
/// 返回:
/// - 剩余的工具交换切片
fn skip_compacted_exchanges(
    exchanges: &[super::model::ToolExchangeRecord],
    skip_calls: usize,
) -> &[super::model::ToolExchangeRecord] {
    if skip_calls == 0 || exchanges.is_empty() {
        return exchanges;
    }
    if skip_calls >= exchanges.len() {
        return &[];
    }
    // 切点落在子轮中间时前移到下一个子轮起点，保证 tool_calls 与结果成套保留
    let boundary_round = exchanges[skip_calls].call.assistant_round;
    let starts_new_round = exchanges[skip_calls - 1].call.assistant_round != boundary_round;
    if starts_new_round {
        return &exchanges[skip_calls..];
    }
    match exchanges[skip_calls..]
        .iter()
        .position(|exchange| exchange.call.assistant_round != boundary_round)
    {
        Some(offset) => &exchanges[skip_calls + offset..],
        // 切点之后全属同一子轮：整体丢弃，否则会留下没有 assistant 消息的孤立结果
        None => &[],
    }
}

/// 按原始模型子轮重建 assistant 工具调用与 tool 结果消息。
///
/// 参数:
/// - `exchanges`: 当前对话轮次的工具交换记录
/// - `messages`: 输出消息列表
///
/// 返回:
/// - 无
fn append_tool_exchange_messages(
    exchanges: &[super::model::ToolExchangeRecord],
    inter_messages: &[TurnMessageRecord],
    messages: &mut Vec<ChatMessage>,
) {
    let mut message_index = 0usize;
    append_inter_messages_through(inter_messages, &mut message_index, 0, messages);
    let mut start = 0usize;
    while start < exchanges.len() {
        let assistant_round = exchanges[start].call.assistant_round;
        let end = exchanges[start + 1..]
            .iter()
            .position(|exchange| exchange.call.assistant_round != assistant_round)
            .map(|offset| start + 1 + offset)
            .unwrap_or(exchanges.len());
        let round = &exchanges[start..end];
        // 1. 同一模型子轮的并行工具调用保持在一条 assistant 消息中
        let tool_calls = round
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
        let reasoning = round[0].call.assistant_reasoning.clone();
        messages.push(ChatMessage::assistant("", Some(tool_calls)).with_reasoning(reasoning));
        // 2. assistant 工具调用后紧跟同一子轮的全部工具结果
        for exchange in round {
            messages.push(ChatMessage::tool(
                exchange.call.provider_call_id.clone(),
                tool_result_content(exchange),
            ));
        }
        let boundary = round
            .last()
            .map(|exchange| exchange.call.seq)
            .unwrap_or_default();
        append_inter_messages_through(inter_messages, &mut message_index, boundary, messages);
        start = end;
    }
    append_turn_inter_messages(&inter_messages[message_index..], messages);
}

/// 追加不晚于指定工具序号的轮次内消息。
///
/// 参数:
/// - `inter_messages`: 轮次内消息
/// - `message_index`: 尚未追加消息的起始位置
/// - `tool_seq`: 当前已经追加的最后一个工具序号
/// - `messages`: provider 消息输出
///
/// 返回:
/// - 无
fn append_inter_messages_through(
    inter_messages: &[TurnMessageRecord],
    message_index: &mut usize,
    tool_seq: usize,
    messages: &mut Vec<ChatMessage>,
) {
    let start = *message_index;
    while *message_index < inter_messages.len()
        && inter_messages[*message_index].after_tool_seq <= tool_seq
    {
        *message_index += 1;
    }
    append_turn_inter_messages(&inter_messages[start..*message_index], messages);
}

/// 把持久化的轮次内消息转换为 provider 消息。
///
/// 参数:
/// - `inter_messages`: 待追加消息
/// - `messages`: provider 消息输出
///
/// 返回:
/// - 无
fn append_turn_inter_messages(
    inter_messages: &[TurnMessageRecord],
    messages: &mut Vec<ChatMessage>,
) {
    for message in inter_messages {
        let projected = if message.kind.role() == "user" && !message.image_urls.is_empty() {
            ChatMessage::user_with_images(message.model_content.clone(), message.image_urls.clone())
        } else {
            ChatMessage::plain(message.kind.role(), message.model_content.clone())
        }
        .with_reasoning(message.reasoning.clone());
        messages.push(projected);
    }
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
    // 1. 最终回复必须带上本轮思考，DeepSeek 带 tools 的后续请求不能缺这个字段
    if !turn.assistant_content.trim().is_empty() || turn.assistant_reasoning.is_some() {
        messages.push(
            ChatMessage::plain("assistant", turn.assistant_content.clone())
                .with_reasoning(turn.assistant_reasoning.clone()),
        );
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
        insert_tool_call, insert_tool_call_with_context, insert_tool_result,
        upsert_tool_output_replacement,
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
            user_image_urls: Vec::new(),
            user_timestamp: "2026-01-01T00:00:00Z".to_string(),
            assistant_content: "done".to_string(),
            assistant_reasoning: None,
            assistant_timestamp: Some("2026-01-01T00:00:01Z".to_string()),
            status: TurnStatus::Completed,
            tool_reports: Vec::new(),
            duration_ms: 0,
            parent_turn_id: None,
            model: None,
            error: None,
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

    /// 【上下文缓存】【供应商历史】验证 provider 状态事件会重放且不修改原始用户输入。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn projects_provider_state_content_separately_from_user_content() {
        let (_temp, db) = db();
        db.start_turn("turn_1", "visible request").unwrap();
        db.set_provider_user_content("turn_1", "runtime\n\nvisible request")
            .unwrap();
        db.complete_turn("turn_1", "done", None).unwrap();

        let messages =
            project_turn_messages_with_tool_history(&db, "default", &db.load_turns().unwrap())
                .unwrap();

        assert!(matches!(
            messages[0].content.as_ref(),
            Some(crate::llm::ChatContent::Text(text)) if text == "runtime\n\nvisible request"
        ));
        assert_eq!(db.load_turns().unwrap()[0].user_content, "visible request");
    }

    /// 【会话历史】【DeepSeek】验证工具子轮按原始思考内容重建。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn projects_reasoning_for_each_tool_round() {
        let (_temp, db) = db();
        for (seq, round, call_id, reasoning) in [
            (1, 1, "call_1", "先查询日期"),
            (2, 2, "call_2", "再查询天气"),
        ] {
            insert_tool_call_with_context(
                &db,
                NewToolCallRecord {
                    session_id: "default".to_string(),
                    turn_id: "turn_1".to_string(),
                    seq,
                    provider_call_id: call_id.to_string(),
                    tool_name: "read_file".to_string(),
                    arguments: "{}".to_string(),
                },
                round,
                Some(reasoning),
            )
            .unwrap();
            insert_tool_result(
                &db,
                NewToolResultRecord {
                    session_id: "default".to_string(),
                    turn_id: "turn_1".to_string(),
                    provider_call_id: call_id.to_string(),
                    ok: true,
                    result_preview: format!("result-{round}"),
                    result_ref: None,
                    error: None,
                    original_chars: 8,
                },
            )
            .unwrap();
        }

        let messages = project_turn_messages_with_tool_history(&db, "default", &[turn()]).unwrap();

        assert_eq!(messages.len(), 6);
        assert_eq!(messages[1].reasoning_content.as_deref(), Some("先查询日期"));
        assert_eq!(messages[3].reasoning_content.as_deref(), Some("再查询天气"));
        assert_eq!(messages[2].role, "tool");
        assert_eq!(messages[4].role, "tool");
    }

    /// 【会话历史】【DeepSeek】验证最终回复带回本轮思考。
    #[test]
    fn projects_final_assistant_reasoning() {
        let (_temp, db) = db();
        let mut completed = turn();
        completed.assistant_content = "已经装完".to_string();
        completed.assistant_reasoning = Some("安装成功，可以向用户汇报".to_string());

        let messages =
            project_turn_messages_with_tool_history(&db, "default", &[completed]).unwrap();

        assert_eq!(messages[1].role, "assistant");
        assert_eq!(
            messages[1].reasoning_content.as_deref(),
            Some("安装成功，可以向用户汇报")
        );
        assert!(matches!(
            messages[1].content.as_ref(),
            Some(crate::llm::ChatContent::Text(text)) if text == "已经装完"
        ));
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

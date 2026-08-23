use super::history::{append_content, append_reasoning};
use crate::render::transcript::TranscriptStore;
use crate::state::{SessionTimelineTurn, TimelineToolEntry, TimelineTurnMessage};

/// 将一轮历史按流式发生顺序写入 transcript。
///
/// 有工具级思考时按 `assistant_round` 交错重放；旧会话没有分轮思考时
/// 仍先整段思考、再工具、再正文，避免把思考挪到工具后面。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `turn`: 持久化轮次
///
/// 返回:
/// - 无
pub(super) fn replay_turn(transcript: &mut TranscriptStore, turn: &SessionTimelineTurn) {
    if turn.tools.iter().any(|tool| {
        tool.reasoning
            .as_deref()
            .is_some_and(|text| !text.trim().is_empty())
    }) {
        replay_interleaved(transcript, turn);
        return;
    }
    append_reasoning(transcript, turn.assistant.reasoning.as_deref());
    append_tools(transcript, &turn.tools);
    replay_remaining_messages(transcript, &turn.messages, 0);
    append_content(transcript, &turn.assistant.content);
}

/// 按模型子轮交错重放思考、工具与轮次内消息。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `turn`: 持久化轮次
///
/// 返回:
/// - 无
fn replay_interleaved(transcript: &mut TranscriptStore, turn: &SessionTimelineTurn) {
    let mut message_index = 0usize;
    let mut emitted_reasoning = Vec::new();
    let mut last_tool_seq = 0usize;
    let mut start = 0usize;
    while start < turn.tools.len() {
        let round_id = turn.tools[start].assistant_round;
        let end = turn.tools[start + 1..]
            .iter()
            .position(|tool| tool.assistant_round != round_id)
            .map(|offset| start + 1 + offset)
            .unwrap_or(turn.tools.len());
        let round = &turn.tools[start..end];
        if let Some(reasoning) = round
            .iter()
            .find_map(|tool| tool.reasoning.as_deref())
            .filter(|text| !text.trim().is_empty())
        {
            append_reasoning(transcript, Some(reasoning));
            emitted_reasoning.push(reasoning.to_string());
        }
        append_tools(transcript, round);
        last_tool_seq = round.last().map(|tool| tool.seq).unwrap_or(last_tool_seq);
        replay_messages_through(
            transcript,
            &turn.messages,
            &mut message_index,
            last_tool_seq,
        );
        start = end;
    }
    replay_remaining_messages(transcript, &turn.messages, message_index);
    if let Some(reasoning) = turn
        .assistant
        .reasoning
        .as_deref()
        .filter(|text| !text.trim().is_empty())
    {
        if !emitted_reasoning.iter().any(|seen| seen == reasoning) {
            append_reasoning(transcript, Some(reasoning));
        }
    }
    append_content(transcript, &turn.assistant.content);
}

/// 追加一组历史工具及其结果。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `tools`: 待追加的工具
///
/// 返回:
/// - 无
fn append_tools(transcript: &mut TranscriptStore, tools: &[TimelineToolEntry]) {
    for tool in tools {
        transcript.push_history_tool_call(tool.name.clone(), tool.arguments.clone());
        if tool.status == "running" {
            transcript.push_tool_result(
                tool.name.clone(),
                false,
                "The tool call was not completed in the previous session".to_string(),
            );
            continue;
        }
        let output = if !tool.output.trim().is_empty() {
            tool.output.clone()
        } else {
            tool.error.clone().unwrap_or_default()
        };
        let ok = tool.ok.unwrap_or(tool.status == "completed");
        transcript.push_tool_result(tool.name.clone(), ok, output);
    }
}

/// 追加不晚于指定工具序号的轮次内消息。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `messages`: 轮次内消息
/// - `index`: 尚未追加的起始下标
/// - `tool_seq`: 当前已经追加的最后一个工具序号
///
/// 返回:
/// - 无
fn replay_messages_through(
    transcript: &mut TranscriptStore,
    messages: &[TimelineTurnMessage],
    index: &mut usize,
    tool_seq: usize,
) {
    let start = *index;
    while *index < messages.len() && messages[*index].after_tool_seq <= tool_seq {
        *index += 1;
    }
    replay_remaining_messages(transcript, &messages[start..*index], 0);
}

/// 追加剩余轮次内消息。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `messages`: 待追加消息
/// - `start`: 起始下标
///
/// 返回:
/// - 无
fn replay_remaining_messages(
    transcript: &mut TranscriptStore,
    messages: &[TimelineTurnMessage],
    start: usize,
) {
    for message in &messages[start..] {
        append_reasoning(transcript, message.reasoning.as_deref());
        if message.content.trim().is_empty() {
            continue;
        }
        if message.role == "assistant" {
            append_content(transcript, &message.content);
        } else {
            transcript.push_automatic_echo(message.content.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::history::append_timeline;
    use crate::render::activity_animation::strip_ansi_for_test;
    use crate::render::transcript::{TranscriptRenderOptions, TranscriptStore};
    use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};
    use crate::state::{SessionTimelineTurn, TimelineMessage, TimelineToolEntry};

    fn options() -> TranscriptRenderOptions {
        TranscriptRenderOptions {
            reasoning_mode: ReasoningDisplayMode::Full,
            tool_call_mode: ToolCallDisplayMode::Summary,
        }
    }

    fn tool(seq: usize, round: usize, name: &str, reasoning: Option<&str>) -> TimelineToolEntry {
        TimelineToolEntry {
            id: format!("tool-{seq}"),
            assistant_round: round,
            seq,
            name: name.to_string(),
            arguments: "{\"path\":\"a.rs\"}".to_string(),
            status: "completed".to_string(),
            output: "ok".to_string(),
            reasoning: reasoning.map(str::to_string),
            ok: Some(true),
            error: None,
            result_ref: None,
            original_chars: None,
            created_at: String::new(),
            completed_at: None,
            permission: None,
        }
    }

    fn turn_with_tools(
        reasoning: Option<&str>,
        content: &str,
        tools: Vec<TimelineToolEntry>,
    ) -> SessionTimelineTurn {
        SessionTimelineTurn {
            turn_id: "turn-1".to_string(),
            seq: 1,
            status: "completed".to_string(),
            user: TimelineMessage {
                timestamp: String::new(),
                content: "ask".to_string(),
                reasoning: None,
                image_urls: Vec::new(),
            },
            assistant: TimelineMessage {
                timestamp: String::new(),
                content: content.to_string(),
                reasoning: reasoning.map(str::to_string),
                image_urls: Vec::new(),
            },
            tools,
            messages: Vec::new(),
            automatic: false,
            duration_ms: 0,
            ttft_ms: 0,
            usage: None,
            error: None,
            injected_content: None,
        }
    }

    /// 【终端】【历史重放】分轮思考应按「思考 → 工具 → 思考 → 工具 → 正文」重放。
    #[test]
    fn reload_interleaves_reasoning_with_tools() {
        let mut transcript = TranscriptStore::new(100);
        append_timeline(
            &mut transcript,
            &[turn_with_tools(
                Some("final report"),
                "done",
                vec![
                    tool(1, 1, "read_file", Some("look around")),
                    tool(2, 2, "read_file", Some("check tests")),
                ],
            )],
        );
        let plain = transcript
            .display_tail(120, &options())
            .iter()
            .map(|line| strip_ansi_for_test(line.as_str()))
            .collect::<Vec<_>>();
        let look = plain.iter().position(|line| line.contains("look around"));
        let first_tool = plain.iter().position(|line| line.contains('•'));
        let check = plain.iter().position(|line| line.contains("check tests"));
        let second_tool = plain
            .iter()
            .enumerate()
            .rev()
            .find(|(_, line)| line.contains('•'))
            .map(|(index, _)| index);
        let final_think = plain.iter().position(|line| line.contains("final report"));
        let body = plain.iter().position(|line| line.contains("done"));
        assert!(look < first_tool, "{plain:?}");
        assert!(first_tool < check, "{plain:?}");
        assert!(check < second_tool, "{plain:?}");
        assert!(second_tool < final_think, "{plain:?}");
        assert!(final_think < body, "{plain:?}");
    }

    /// 【终端】【历史重放】没有工具级思考的旧会话仍先整段思考再工具。
    #[test]
    fn reload_without_round_reasoning_keeps_legacy_order() {
        let mut transcript = TranscriptStore::new(100);
        append_timeline(
            &mut transcript,
            &[turn_with_tools(
                Some("legacy think"),
                "done",
                vec![tool(1, 0, "read_file", None)],
            )],
        );
        let plain = transcript
            .display_tail(120, &options())
            .iter()
            .map(|line| strip_ansi_for_test(line.as_str()))
            .collect::<Vec<_>>();
        let think = plain.iter().position(|line| line.contains("legacy think"));
        let tool_line = plain.iter().position(|line| line.contains('•'));
        let body = plain.iter().position(|line| line.contains("done"));
        assert!(think < tool_line, "{plain:?}");
        assert!(tool_line < body, "{plain:?}");
    }
}

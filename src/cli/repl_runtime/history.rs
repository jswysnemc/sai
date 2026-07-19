use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::render::transcript::{TranscriptMode, TranscriptStore};
use crate::state::SessionTimelineTurn;

/// 将持久化会话时间线追加到 TUI transcript。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `turns`: 按时间顺序排列的会话轮次
///
/// 返回:
/// - 无
pub(super) fn append_timeline(transcript: &mut TranscriptStore, turns: &[SessionTimelineTurn]) {
    for turn in turns {
        append_user_message(transcript, turn);
        append_reasoning(transcript, turn.assistant.reasoning.as_deref());
        append_tools(transcript, turn);
        append_content(transcript, &turn.assistant.content);
    }
}

/// 追加历史用户输入，自动轮次使用蓝色圆点。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `turn`: 待渲染的历史轮次
///
/// 返回:
/// - 无
fn append_user_message(transcript: &mut TranscriptStore, turn: &SessionTimelineTurn) {
    if turn.user.content.trim().is_empty() {
        return;
    }
    let mode = if turn.automatic {
        TranscriptMode::Automatic
    } else {
        TranscriptMode::Yolo
    };
    transcript.push_user_echo(mode, history_user_text(turn));
}

/// 将内部自动输入转换为不暴露控制标记的历史文本。
///
/// 参数:
/// - `turn`: 待转换的历史轮次
///
/// 返回:
/// - 用户可见的历史输入文本
fn history_user_text(turn: &SessionTimelineTurn) -> String {
    if !turn.automatic {
        return turn.user.content.clone();
    }
    let content = turn.user.content.trim();
    const EXTERNAL_OPEN: &str = "<external-completion-events>";
    const EXTERNAL_CLOSE: &str = "</external-completion-events>";
    if let Some(inner) = content
        .strip_prefix(EXTERNAL_OPEN)
        .and_then(|value| value.strip_suffix(EXTERNAL_CLOSE))
    {
        let heading = "Background work completed; continuing the conversation automatically";
        let details = inner
            .trim()
            .split_once("\n\n")
            .map(|(_, details)| details.trim())
            .unwrap_or_else(|| inner.trim());
        return format!("{heading}\n\n{details}");
    }
    "Goal continuation".to_string()
}

/// 追加历史推理内容。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `reasoning`: 可选推理文本
///
/// 返回:
/// - 无
fn append_reasoning(transcript: &mut TranscriptStore, reasoning: Option<&str>) {
    let Some(reasoning) = reasoning.filter(|value| !value.trim().is_empty()) else {
        return;
    };
    transcript.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: reasoning.to_string(),
    });
    transcript.finalize_live_tail();
}

/// 追加历史工具调用及其结果。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `turn`: 包含工具历史的轮次
///
/// 返回:
/// - 无
fn append_tools(transcript: &mut TranscriptStore, turn: &SessionTimelineTurn) {
    for tool in &turn.tools {
        transcript.push_history_tool_call(tool.name.clone(), tool.arguments.clone());
        if tool.status == "running" {
            let output = "The tool call was not completed in the previous session";
            transcript.push_tool_result(tool.name.clone(), false, output.to_string());
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

/// 追加历史助手回复内容。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `content`: 助手回复文本
///
/// 返回:
/// - 无
fn append_content(transcript: &mut TranscriptStore, content: &str) {
    if content.trim().is_empty() {
        return;
    }
    transcript.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Content,
        text: content.to_string(),
    });
    transcript.finalize_live_tail();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::transcript::{TranscriptRenderOptions, TranscriptStore};
    use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};
    use crate::state::{SessionTimelineTurn, TimelineMessage, TimelineToolEntry};

    fn options() -> TranscriptRenderOptions {
        TranscriptRenderOptions {
            reasoning_mode: ReasoningDisplayMode::Full,
            tool_call_mode: ToolCallDisplayMode::Summary,
        }
    }

    fn turn(automatic: bool, user: &str) -> SessionTimelineTurn {
        SessionTimelineTurn {
            turn_id: "turn-1".to_string(),
            seq: 1,
            status: "completed".to_string(),
            user: TimelineMessage {
                timestamp: String::new(),
                content: user.to_string(),
                reasoning: None,
            },
            assistant: TimelineMessage {
                timestamp: String::new(),
                content: "已完成".to_string(),
                reasoning: None,
            },
            tools: vec![TimelineToolEntry {
                id: "tool-1".to_string(),
                name: "read_file".to_string(),
                arguments: "{\"path\":\"README.md\"}".to_string(),
                status: "completed".to_string(),
                output: "内容".to_string(),
                ok: Some(true),
                error: None,
                result_ref: None,
                original_chars: None,
                created_at: String::new(),
                completed_at: None,
                permission: None,
            }],
            automatic,
        }
    }

    #[test]
    fn history_uses_blue_mode_for_automatic_turns() {
        let mut transcript = TranscriptStore::new(100);
        append_timeline(
            &mut transcript,
            &[turn(
                true,
                "<external-completion-events>子 Agent 已完成</external-completion-events>",
            )],
        );
        let rendered = transcript
            .display_tail(120, &options())
            .iter()
            .map(|line| line.as_str())
            .collect::<String>();

        assert!(rendered.contains("\x1b[38;5;39m●"));
        assert!(rendered.contains("子 Agent 已完成"));
        assert!(rendered.contains("已完成"));
    }

    #[test]
    fn history_uses_yellow_mode_for_user_turns() {
        let mut transcript = TranscriptStore::new(100);
        append_timeline(&mut transcript, &[turn(false, "检查项目")]);
        let rendered = transcript
            .display_tail(120, &options())
            .iter()
            .map(|line| line.as_str())
            .collect::<String>();

        assert!(rendered.contains("\x1b[38;5;208m●"));
        assert!(rendered.contains("检查项目"));
    }
}

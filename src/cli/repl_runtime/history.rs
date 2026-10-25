use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::render::transcript::{TranscriptMode, TranscriptStore};
use crate::state::{SessionTimelineCompaction, SessionTimelineTurn};

/// 将持久化会话时间线追加到 TUI transcript。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `turns`: 按时间顺序排列的会话轮次
///
/// 返回:
/// - 无
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn append_timeline(transcript: &mut TranscriptStore, turns: &[SessionTimelineTurn]) {
    append_timeline_with_compaction(transcript, turns, None);
}

/// 将持久化会话时间线与可选压缩摘要追加到 TUI transcript。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `turns`: 按时间顺序排列的会话轮次
/// - `compaction`: 最新压缩摘要；有内容时先于历史轮次渲染
///
/// 返回:
/// - 无
pub(super) fn append_timeline_with_compaction(
    transcript: &mut TranscriptStore,
    turns: &[SessionTimelineTurn],
    compaction: Option<&SessionTimelineCompaction>,
) {
    if let Some(compaction) = compaction {
        append_compaction(transcript, compaction);
    }
    for turn in turns {
        append_user_message(transcript, turn);
        super::history_replay::replay_turn(transcript, turn);
        append_turn_summary(transcript, turn);
    }
}

/// 【会话恢复】【轮次总览】用持久化的耗时与用量补回每轮末尾的总览分割线。
///
/// 现场渲染时总览来自运行期快照，不写入时间线；恢复时据轮次自带的统计重建，
/// 退出再进入后每轮的分割线与统计因此仍在。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `turn`: 已重放的历史轮次
///
/// 返回:
/// - 无
fn append_turn_summary(transcript: &mut TranscriptStore, turn: &SessionTimelineTurn) {
    if let Some(summary) = crate::render::session_summary::render_history_turn_summary(
        turn.duration_ms,
        turn.ttft_ms,
        turn.usage.as_ref(),
        &turn.assistant.timestamp,
    ) {
        transcript.push_turn_summary(summary);
    }
}

/// 追加历史压缩摘要单元。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `compaction`: 压缩展示数据
///
/// 返回:
/// - 无
fn append_compaction(transcript: &mut TranscriptStore, compaction: &SessionTimelineCompaction) {
    let summary = compaction.summary.trim();
    if summary.is_empty() {
        return;
    }
    transcript.push_compaction_finished(compaction.applied, None, None, Some(summary.to_string()));
}

/// 追加历史用户输入，所有自动轮次使用工具通知，真实用户消息保留普通回显。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `turn`: 待渲染的历史轮次
///
/// 返回:
/// - 无
fn append_user_message(transcript: &mut TranscriptStore, turn: &SessionTimelineTurn) {
    if turn.user.content.trim().is_empty() && turn.user.image_urls.is_empty() {
        return;
    }
    if turn.automatic {
        if turn.user.image_urls.is_empty() && crate::render::is_external_notice(&turn.user.content)
        {
            transcript.push_background_notice(turn.user.content.clone());
        } else {
            transcript.push_conversation_notice(
                crate::render::ConversationNoticeKind::GoalContinuation,
                history_user_text(turn),
                &turn.user.image_urls,
            );
        }
        return;
    }
    transcript.push_user_input(
        TranscriptMode::Yolo,
        crate::render::input_atom::InputEcho::with_images(
            history_user_text(turn),
            &turn.user.image_urls,
        ),
    );
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
    if content.starts_with("Goal 自动续轮") || content.starts_with("Goal continuation:") {
        return content.to_string();
    }
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
pub(super) fn append_reasoning(transcript: &mut TranscriptStore, reasoning: Option<&str>) {
    let Some(reasoning) = reasoning.filter(|value| !value.trim().is_empty()) else {
        return;
    };
    transcript.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: reasoning.to_string(),
    });
    transcript.finalize_live_tail();
}

/// 追加历史助手回复内容。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `content`: 助手回复文本
///
/// 返回:
/// - 无
pub(super) fn append_content(transcript: &mut TranscriptStore, content: &str) {
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

    /// 【终端】【历史图片测试】仅含图片的历史轮次也恢复用户回显和可展开段落。
    /// 参数: 无；返回: 无。
    #[test]
    fn history_restores_image_only_user_message() {
        let mut turn = turn(false, "");
        turn.user
            .image_urls
            .push("data:image/png;base64,fixture".into());
        let mut transcript = TranscriptStore::new(100);
        append_user_message(&mut transcript, &turn);
        let document =
            transcript.render_fullscreen(80, &options(), &std::collections::HashSet::new());
        assert_eq!(document.anchors.len(), 1);
        assert_eq!(document.paragraphs.len(), 1);
        let text = document
            .lines
            .iter()
            .map(|line| line.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("[image 1]"));
        assert!(!text.contains("data:image"));
    }

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
                image_urls: Vec::new(),
            },
            assistant: TimelineMessage {
                timestamp: String::new(),
                content: "已完成".to_string(),
                reasoning: None,
                image_urls: Vec::new(),
            },
            tools: vec![TimelineToolEntry {
                id: "tool-1".to_string(),
                assistant_round: 1,
                seq: 1,
                name: "read_file".to_string(),
                arguments: "{\"path\":\"README.md\"}".to_string(),
                status: "completed".to_string(),
                output: "内容".to_string(),
                reasoning: None,
                ok: Some(true),
                error: None,
                result_ref: None,
                original_chars: None,
                created_at: String::new(),
                completed_at: None,
                permission: None,
            }],
            messages: Vec::new(),
            automatic,
            duration_ms: 0,
            ttft_ms: 0,
            usage: None,
            error: None,
            injected_content: None,
        }
    }

    /// 历史子任务保留身份、最新终态和详情入口，不退化成普通工具行。
    #[test]
    fn history_subagent_restores_panel_entry_and_detail_view() {
        let owner = format!("history-panel-{}", rand::random::<u64>());
        let (snapshot, _cancel) = crate::tools::subagent_state::create_subagent_for_owner(
            &owner,
            "检查项目".into(),
            "explore".into(),
            8,
        );
        let mut historical = turn(false, "查看子任务");
        historical.tools[0].name = "subagent".into();
        historical.tools[0].arguments = r#"{"action":"start","description":"检查项目"}"#.into();
        historical.tools[0].output =
            serde_json::json!({"ok": true, "subagent": snapshot}).to_string();
        crate::tools::subagent_state::finish_subagent(
            &snapshot.id,
            "interrupted",
            None,
            Some("The session ended before completion".into()),
            None,
        );

        let mut transcript = TranscriptStore::new(100);
        append_timeline(&mut transcript, &[historical]);
        let entries = transcript.subagent_overview();
        assert_eq!(entries.len(), 1, "历史子任务必须保留面板入口");
        assert_eq!(entries[0].status, "interrupted");
        assert!(!entries[0].running);
        assert!(transcript.enter_subagent_view(entries[0].cell_index));
        let view = transcript
            .display_tail(100, &options())
            .iter()
            .map(|line| line.as_str())
            .collect::<String>();
        assert!(
            view.contains("The session ended before completion"),
            "{view}"
        );
        assert!(!view.contains("Working"));
        crate::tools::subagent_state::clear_subagents_for_owner(&owner);
    }

    #[test]
    fn history_restores_background_notice_without_a_user_navigation_point() {
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

        assert!(!rendered.contains("\x1b[38;5;39m●"));
        assert!(transcript
            .render_fullscreen(120, &options(), &std::collections::HashSet::new())
            .anchors
            .is_empty());
        assert!(
            crate::render::activity_animation::strip_ansi_for_test(&rendered)
                .contains("Background work completed")
        );
        let full = TranscriptRenderOptions {
            tool_call_mode: ToolCallDisplayMode::Full,
            ..options()
        };
        let details = transcript
            .display_tail(120, &full)
            .iter()
            .map(|line| line.as_str())
            .collect::<String>();
        assert!(details.contains("子 Agent 已完成"));
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

    /// 【终端】【自动历史测试】目标续轮及带目标前缀的完成回执均恢复为工具，不能产生用户导航点。
    /// 参数: 无；返回: 无。
    #[test]
    fn automatic_history_messages_never_add_user_navigation_anchors() {
        for (source, expected) in [
            ("Goal 自动续轮：Inspect layout", "Continuing goal"),
            ("Goal context\n\n<external-completion-events>\n后台命令：echo done\n状态：completed\n</external-completion-events>", "Background command completed"),
        ] {
            let mut transcript = TranscriptStore::new(200);
            append_user_message(&mut transcript, &turn(true, source));
            let document = transcript.render_fullscreen(100, &options(), &std::collections::HashSet::new());
            assert!(document.anchors.is_empty());
            let plain = document.lines.iter().map(|line| crate::render::activity_animation::strip_ansi_for_test(line.as_str())).collect::<Vec<_>>().join("\n");
            assert!(plain.contains(expected), "{plain}");
            assert!(!plain.contains("Goal context"), "{plain}");
        }
    }
}

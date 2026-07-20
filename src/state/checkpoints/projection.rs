use super::model::{CheckpointStats, CompactionCheckpoint, ProjectedHistory};
use super::repository::{count_checkpoints, load_latest_checkpoint};
use crate::llm::ChatMessage;
use crate::state::tool_history::{
    project_legacy_tool_report_messages, project_turn_messages_with_tool_history,
};
use crate::state::turns::ConversationDb;
use crate::state::turns::{turns_to_entries, Turn};
use anyhow::Result;

/// 从 checkpoint 和 tail turns 构造历史投影。
///
/// 参数:
/// - `checkpoint`: 最近 checkpoint
/// - `tail_turns`: checkpoint 后仍保留的原始轮次
///
/// 返回:
/// - 会话历史投影
pub(in crate::state) fn project_history_from_parts(
    checkpoint: Option<CompactionCheckpoint>,
    tail_turns: Vec<Turn>,
) -> ProjectedHistory {
    let checkpoint_count = usize::from(checkpoint.is_some());
    let messages = turns_to_messages(&tail_turns);
    project_history_from_parts_with_messages(checkpoint, tail_turns, messages, checkpoint_count)
}

/// 从 checkpoint、数量和 tail turns 构造历史投影。
///
/// 参数:
/// - `checkpoint`: 最近 checkpoint
/// - `tail_turns`: checkpoint 后仍保留的原始轮次
/// - `checkpoint_count`: checkpoint 总数
///
/// 返回:
/// - 会话历史投影
fn project_history_from_parts_with_messages(
    checkpoint: Option<CompactionCheckpoint>,
    tail_turns: Vec<Turn>,
    messages: Vec<ChatMessage>,
    checkpoint_count: usize,
) -> ProjectedHistory {
    let checkpoint_context = checkpoint.as_ref().map(checkpoint_context_message);
    let has_checkpoint = checkpoint.is_some();
    let stats = CheckpointStats {
        checkpoint_count: if has_checkpoint {
            checkpoint_count.max(1)
        } else {
            0
        },
        covered_turns: checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.source_turn_count)
            .unwrap_or_default(),
        tail_turns: tail_turns.len(),
        latest_checkpoint_at: checkpoint.map(|checkpoint| checkpoint.created_at),
    };
    ProjectedHistory {
        checkpoint_context,
        entries: turns_to_entries(tail_turns),
        messages,
        stats,
    }
}

/// 从数据库构造历史投影。
///
/// 参数:
/// - `db`: 对话数据库
/// - `exclude_turn_id`: 可选排除轮次
///
/// 返回:
/// - 会话历史投影
pub(in crate::state) fn project_history(
    db: &ConversationDb,
    session_id: &str,
    exclude_turn_id: Option<&str>,
) -> Result<ProjectedHistory> {
    let conn = db.conn.lock().unwrap();
    let checkpoint_count = count_checkpoints(&conn)?;
    let checkpoint = load_latest_checkpoint(&conn)?;
    drop(conn);
    let after_seq = checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.compacted_to_seq)
        .unwrap_or_default();
    let tail_turns = db.load_turns_after_seq(after_seq, exclude_turn_id)?;
    let messages = project_turn_messages_with_tool_history(db, session_id, &tail_turns)?;
    Ok(project_history_from_parts_with_messages(
        checkpoint,
        tail_turns,
        messages,
        checkpoint_count,
    ))
}

/// 构造 provider 可见 checkpoint 上下文。
///
/// 参数:
/// - `checkpoint`: 最近 checkpoint
///
/// 返回:
/// - 系统上下文文本
fn checkpoint_context_message(checkpoint: &CompactionCheckpoint) -> String {
    format!(
        "<conversation-checkpoint>\n<metadata id=\"{}\" covered_from_seq=\"{}\" covered_to_seq=\"{}\" />\n<summary>\n{}\n</summary>\n<recent>\n{}\n</recent>\n</conversation-checkpoint>",
        checkpoint.id,
        checkpoint.compacted_from_seq,
        checkpoint.compacted_to_seq,
        checkpoint.summary.trim(),
        checkpoint.recent.trim()
    )
}

/// 将轮次转换为无工具结构的 provider 消息。
///
/// 参数:
/// - `turns`: 待转换轮次
///
/// 返回:
/// - provider 历史消息列表
fn turns_to_messages(turns: &[Turn]) -> Vec<ChatMessage> {
    let mut messages = Vec::with_capacity(turns.len() * 3);
    for turn in turns {
        messages.push(ChatMessage::plain("user", turn.user_content.clone()));
        if !turn.assistant_content.trim().is_empty() {
            messages.push(ChatMessage::plain(
                "assistant",
                turn.assistant_content.clone(),
            ));
        }
        messages.extend(project_legacy_tool_report_messages(&turn.tool_reports));
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::project_history_from_parts;
    use crate::state::checkpoints::{CheckpointReason, CompactionCheckpoint};
    use crate::state::turns::{Turn, TurnStatus};

    fn turn(seq: i64, turn_id: &str) -> Turn {
        Turn {
            turn_id: turn_id.to_string(),
            seq,
            user_content: format!("user {seq}"),
            user_image_urls: Vec::new(),
            user_timestamp: "2026-01-01T00:00:00Z".to_string(),
            assistant_content: format!("assistant {seq}"),
            assistant_reasoning: None,
            assistant_timestamp: Some("2026-01-01T00:00:01Z".to_string()),
            status: TurnStatus::Completed,
            tool_reports: Vec::new(),
        }
    }

    #[test]
    fn projects_checkpoint_context_and_tail_entries() {
        let checkpoint = CompactionCheckpoint {
            id: "cp_1".to_string(),
            seq: 10,
            compacted_from_seq: 1,
            compacted_to_seq: 2,
            summary: "summary".to_string(),
            recent: "recent".to_string(),
            source_turn_count: 2,
            reason: CheckpointReason::Auto,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let projected = project_history_from_parts(Some(checkpoint), vec![turn(3, "turn_3")]);

        assert!(projected
            .checkpoint_context
            .as_ref()
            .unwrap()
            .contains("<conversation-checkpoint>"));
        assert_eq!(projected.entries.len(), 2);
        assert_eq!(projected.stats.covered_turns, 2);
        assert_eq!(projected.stats.tail_turns, 1);
    }
}

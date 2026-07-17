use super::model::{ProjectionEstimate, ProjectionKind, ProjectionStats};
use crate::state::checkpoints::{count_checkpoints, load_latest_checkpoint};
use crate::state::session_snapshot;
use crate::state::{CompactionSummary, RecoverySnapshot, StateStore};
use anyhow::Result;

pub(super) struct SessionSummaryProjectionParts {
    pub estimate: ProjectionEstimate,
    pub stats: ProjectionStats,
    pub compaction: Option<CompactionSummary>,
    pub recovery: RecoverySnapshot,
}

/// 构造轻量命令摘要投影部件。
///
/// 参数:
/// - `store`: 当前状态仓储
/// - `context_limit_chars`: 当前模型上下文窗口字符数
///
/// 返回:
/// - 不包含 provider 消息投影的摘要部件
pub(super) fn build_session_summary_projection_parts(
    store: &StateStore,
    context_limit_chars: usize,
) -> Result<SessionSummaryProjectionParts> {
    let (checkpoint_count, checkpoint) = store
        .conv_db
        .with_conn(|conn| Ok((count_checkpoints(conn)?, load_latest_checkpoint(conn)?)))?;
    let after_seq = checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.compacted_to_seq)
        .unwrap_or_default();
    let turn_stats = store.conv_db.session_summary_turn_stats(after_seq)?;
    let compaction = store.load_authoritative_compaction_summary()?;
    let usage = store.usage_snapshot()?;
    let recovery = store.recovery_snapshot()?;
    let summary_context_chars = checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint_context_chars(checkpoint))
        .or_else(|| {
            compaction
                .as_ref()
                .map(|summary| summary.summary.trim().chars().count())
        })
        .unwrap_or_default();
    let state_context_chars = summary_context_chars + turn_stats.context_chars;
    let covered_turns = checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.source_turn_count)
        .unwrap_or_default();
    let latest_checkpoint_at = checkpoint.map(|checkpoint| checkpoint.created_at);
    let compacted_turns = covered_turns.max(
        compaction
            .as_ref()
            .map(|summary| summary.compacted_turns)
            .unwrap_or_default(),
    );
    Ok(SessionSummaryProjectionParts {
        estimate: ProjectionEstimate {
            message_chars: 0,
            state_context_chars,
            context_limit_chars,
            context_ratio: session_snapshot::context_ratio(
                state_context_chars,
                context_limit_chars,
            ),
        },
        stats: ProjectionStats {
            session_id: store.session_id().to_string(),
            turn_count: covered_turns + turn_stats.tail_turn_count,
            has_compaction_summary: compaction.is_some() || covered_turns > 0,
            compacted_turns,
            checkpoint_count,
            checkpoint_covered_turns: covered_turns,
            tail_turns: turn_stats.tail_turn_count,
            latest_checkpoint_at,
            usage,
        },
        compaction,
        recovery,
    })
}

/// 估算 checkpoint 上下文文本长度。
///
/// 参数:
/// - `checkpoint`: 最近 checkpoint
///
/// 返回:
/// - 与 checkpoint 上下文注入近似一致的字符数
fn checkpoint_context_chars(checkpoint: &crate::state::checkpoints::CompactionCheckpoint) -> usize {
    format!(
        "<conversation-checkpoint>\n<metadata id=\"{}\" covered_from_seq=\"{}\" covered_to_seq=\"{}\" />\n<summary>\n{}\n</summary>\n<recent>\n{}\n</recent>\n</conversation-checkpoint>",
        checkpoint.id,
        checkpoint.compacted_from_seq,
        checkpoint.compacted_to_seq,
        checkpoint.summary.trim(),
        checkpoint.recent.trim()
    )
    .chars()
    .count()
}

#[allow(dead_code)]
const _: ProjectionKind = ProjectionKind::SessionSummary;

use super::{estimate_chat_messages_tokens, CompactionRequest};
use crate::llm::ChatMessage;
use crate::state::checkpoints::{
    project_history_from_parts, CheckpointReason, CompactionCheckpoint,
};
use crate::state::request_projection::ProjectedRequest;
use crate::state::tool_history::project_turn_messages_with_tool_history;
use crate::state::turns::Turn;
use crate::state::StateStore;
use anyhow::Result;

impl StateStore {
    /// 估算压缩写入后重新投影的 provider 请求 token 数。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `summary`: 待写入 checkpoint 的摘要正文
    /// - `projection`: 当前 provider 请求投影视图
    /// - `exclude_turn_id`: 当前运行中轮次标识
    ///
    /// 返回:
    /// - 重新投影后的 provider 请求 token 数估算
    pub(in crate::state) fn estimate_reprojected_context_chars_after_compaction(
        &self,
        request: &CompactionRequest,
        summary: &str,
        projection: &ProjectedRequest,
        exclude_turn_id: Option<&str>,
    ) -> Result<usize> {
        let current_history_chars = self.visible_history_context_chars(exclude_turn_id)?;
        let next_history_chars =
            self.projected_history_chars_after_compaction(request, summary, exclude_turn_id)?;
        Ok(projection
            .estimate
            .message_chars
            .saturating_sub(current_history_chars)
            .saturating_add(next_history_chars))
    }

    /// 估算当前 provider 可见历史上下文 token 数。
    ///
    /// 参数:
    /// - `exclude_turn_id`: 当前运行中轮次标识
    ///
    /// 返回:
    /// - 当前历史上下文 token 数
    pub(in crate::state) fn visible_history_context_chars(
        &self,
        exclude_turn_id: Option<&str>,
    ) -> Result<usize> {
        let history = self.project_history(exclude_turn_id)?;
        let summary_context = history
            .checkpoint_context
            .or(self.compaction_summary_context()?);
        Ok(history_messages_chars(
            summary_context.as_deref(),
            history.messages,
        ))
    }

    /// 估算应用压缩后的 provider 可见历史上下文 token 数。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `summary`: 待写入 checkpoint 的摘要正文
    /// - `exclude_turn_id`: 当前运行中轮次标识
    ///
    /// 返回:
    /// - 压缩后的历史上下文 token 数
    pub(in crate::state) fn projected_history_chars_after_compaction(
        &self,
        request: &CompactionRequest,
        summary: &str,
        exclude_turn_id: Option<&str>,
    ) -> Result<usize> {
        let checkpoint = self.pending_checkpoint_for_budget(request, summary)?;
        let tail_turns = self.tail_turns_after_compaction(request, exclude_turn_id)?;
        let tail_messages =
            project_turn_messages_with_tool_history(&self.conv_db, &self.session_id, &tail_turns)?;
        let history = project_history_from_parts(Some(checkpoint), tail_turns);
        let messages = if tail_messages.is_empty() {
            history.messages
        } else {
            tail_messages
        };
        Ok(history_messages_chars(
            history.checkpoint_context.as_deref(),
            messages,
        ))
    }

    /// 构造仅用于预算估算的待写入 checkpoint。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `summary`: 待写入 checkpoint 的摘要正文
    ///
    /// 返回:
    /// - 待投影 checkpoint
    fn pending_checkpoint_for_budget(
        &self,
        request: &CompactionRequest,
        summary: &str,
    ) -> Result<CompactionCheckpoint> {
        let (from_seq, to_seq) = request
            .seq_range()
            .ok_or_else(|| anyhow::anyhow!("compaction request has no turns"))?;
        let previous_count = self
            .load_authoritative_compaction_summary()?
            .map(|summary| summary.compacted_turns)
            .unwrap_or_default();
        Ok(CompactionCheckpoint {
            id: "cp_pending_budget_check".to_string(),
            seq: to_seq,
            compacted_from_seq: from_seq,
            compacted_to_seq: to_seq,
            summary: summary.trim().to_string(),
            recent: request.recent_context(),
            source_turn_count: request.source_turn_count_after_compaction(previous_count),
            reason: CheckpointReason::Auto,
            created_at: "1970-01-01T00:00:00Z".to_string(),
        })
    }

    /// 构造压缩写入后的 tail turns。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `exclude_turn_id`: 当前运行中轮次标识
    ///
    /// 返回:
    /// - 压缩后仍保留的 tail turns
    fn tail_turns_after_compaction(
        &self,
        request: &CompactionRequest,
        exclude_turn_id: Option<&str>,
    ) -> Result<Vec<Turn>> {
        let (_, to_seq) = request
            .seq_range()
            .ok_or_else(|| anyhow::anyhow!("compaction request has no turns"))?;
        Ok(self
            .conv_db
            .load_turns()?
            .into_iter()
            .filter(|turn| turn.seq > to_seq)
            .filter(|turn| Some(turn.turn_id.as_str()) != exclude_turn_id)
            .collect())
    }
}

/// 估算 provider 历史消息 token 数。
///
/// 参数:
/// - `summary_context`: checkpoint 或 legacy summary 上下文
/// - `entries`: 历史消息入口
///
/// 返回:
/// - 历史消息 token 数
fn history_messages_chars(
    summary_context: Option<&str>,
    history_messages: Vec<ChatMessage>,
) -> usize {
    let mut messages = Vec::new();
    if let Some(context) = summary_context {
        messages.push(ChatMessage::system(context));
    }
    messages.extend(history_messages);
    if messages.is_empty() {
        return 0;
    }
    estimate_chat_messages_tokens(&messages)
}

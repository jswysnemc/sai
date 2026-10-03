use super::{estimate_chat_messages_tokens, CompactionRequest};
use crate::llm::ChatMessage;
use crate::state::checkpoints::{
    project_history_with_checkpoint, CheckpointReason, CompactionCheckpoint,
};
use crate::state::request_projection::ProjectedRequest;
use crate::state::StateStore;
use anyhow::Result;

impl StateStore {
    /// 【上下文】【预算预检】用同一投影器计算已持久化历史在压缩前后的 token 差值
    /// 参数: request 为覆盖范围，summary 为新摘要，projection 为实际请求，exclude_turn_id 为独立拼接的运行轮次；返回压缩后请求占用
    pub(in crate::state) fn estimate_reprojected_context_chars_after_compaction(
        &self,
        request: &CompactionRequest,
        summary: &str,
        projection: &ProjectedRequest,
        exclude_turn_id: Option<&str>,
    ) -> Result<usize> {
        let checkpoint = {
            let conn = self.conv_db.conn.lock().unwrap();
            crate::state::checkpoints::load_latest_checkpoint(&conn)?
        };
        let mut current = self.budget_history_messages(checkpoint, exclude_turn_id)?;
        let current_tools = projection
            .messages
            .iter()
            .filter(|message| message.role == "tool")
            .filter_map(|message| {
                message
                    .tool_call_id
                    .as_deref()
                    .map(|id| (id, &message.content))
            })
            .collect::<std::collections::HashMap<_, _>>();
        // 1. 【上下文】【预算预检】使用已经发送的正文，包括局部摘要与陈旧结果替换
        for message in current.iter_mut().filter(|message| message.role == "tool") {
            if let Some(content) = message
                .tool_call_id
                .as_deref()
                .and_then(|id| current_tools.get(id))
            {
                message.content = (*content).clone();
            }
        }
        let next_checkpoint = self.pending_checkpoint_for_budget(request, summary)?;
        let mut next = self.budget_history_messages(Some(next_checkpoint), exclude_turn_id)?;
        if projection.context_blocks {
            self.apply_context_blocks(&mut next)?;
        }
        // 2. 【上下文】【预算预检】运行轮次的已覆盖子轮同样参与扣减，系统提示与临时消息留在原预算中
        Ok(projection
            .estimate
            .message_chars
            .saturating_sub(estimate_chat_messages_tokens(&current))
            .saturating_add(estimate_chat_messages_tokens(&next)))
    }

    /// 【上下文】【历史预算】估算当前正式历史，不含独立拼接的运行轮次
    /// 参数: exclude_turn_id 为排除轮次；返回历史 token 数
    pub(in crate::state) fn visible_history_context_chars(
        &self,
        exclude_turn_id: Option<&str>,
    ) -> Result<usize> {
        let history = self.project_history(exclude_turn_id)?;
        let summary = history
            .checkpoint_context
            .or(self.compaction_summary_context()?);
        let mut messages = Vec::new();
        if let Some(summary) = summary {
            messages.push(ChatMessage::system(summary));
        }
        messages.extend(history.messages);
        Ok(estimate_chat_messages_tokens(&messages))
    }

    /// 【上下文】【历史预算】估算未启用局部摘要时全局压缩后的历史
    /// 参数: request 为范围，summary 为摘要，exclude_turn_id 为独立拼接轮次；返回 token 数
    pub(in crate::state) fn projected_history_chars_after_compaction(
        &self,
        request: &CompactionRequest,
        summary: &str,
        exclude_turn_id: Option<&str>,
    ) -> Result<usize> {
        let checkpoint = self.pending_checkpoint_for_budget(request, summary)?;
        Ok(estimate_chat_messages_tokens(
            &self.budget_history_messages(Some(checkpoint), exclude_turn_id)?,
        ))
    }

    /// 【上下文】【统一预算投影】重建指定 checkpoint 下的历史及运行工具子轮
    /// 参数: checkpoint 为当前或待提交摘要，exclude_turn_id 为单独拼接的轮次；返回持久历史消息，不含临时提醒
    fn budget_history_messages(
        &self,
        checkpoint: Option<CompactionCheckpoint>,
        exclude_turn_id: Option<&str>,
    ) -> Result<Vec<ChatMessage>> {
        let skip_calls = checkpoint
            .as_ref()
            .filter(|checkpoint| checkpoint.running_turn_id.as_deref() == exclude_turn_id)
            .map(|checkpoint| checkpoint.running_turn_compacted_calls)
            .unwrap_or_default();
        let count = usize::from(checkpoint.is_some());
        let history = project_history_with_checkpoint(
            &self.conv_db,
            &self.session_id,
            exclude_turn_id,
            checkpoint,
            count,
        )?;
        let summary = history
            .checkpoint_context
            .or(self.compaction_summary_context()?);
        let mut messages = Vec::new();
        if let Some(summary) = summary {
            messages.push(ChatMessage::system(summary));
        }
        messages.extend(history.messages);
        if let Some(turn_id) = exclude_turn_id {
            messages.extend(self.project_running_turn_tools_at_boundary(turn_id, skip_calls)?);
        }
        Ok(messages)
    }

    /// 【上下文】【预算快照】构造不写入数据库的待提交 checkpoint
    /// 参数: request 为压缩范围，summary 为摘要正文；返回与正式提交使用相同边界的快照
    fn pending_checkpoint_for_budget(
        &self,
        request: &CompactionRequest,
        summary: &str,
    ) -> Result<CompactionCheckpoint> {
        let previous = {
            let conn = self.conv_db.conn.lock().unwrap();
            crate::state::checkpoints::load_latest_checkpoint(&conn)?
        };
        let boundary = previous
            .as_ref()
            .map(|item| item.compacted_to_seq)
            .unwrap_or_default();
        let previous_count = previous
            .as_ref()
            .map(|item| item.source_turn_count)
            .unwrap_or_default();
        let (from_seq, to_seq) = request.seq_range().unwrap_or((boundary, boundary));
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
            running_turn_id: request
                .running_turn
                .as_ref()
                .map(|running| running.turn_id.clone()),
            running_turn_compacted_calls: request
                .running_turn
                .as_ref()
                .map(|running| running.compacted_calls)
                .unwrap_or_default(),
        })
    }
}

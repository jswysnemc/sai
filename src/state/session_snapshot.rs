use super::compaction::CompactionSummary;
use super::context_epoch::ContextEpochSummary;
use super::failure_recovery::RecoverySnapshot;
use super::request_projection::DynamicContextSource;
use super::session_memory::summary::SessionMemorySummary;
use super::tool_history::ToolHistorySummary;
use super::usage::UsageSnapshot;
use super::StateStore;
use crate::runtime_recovery::RuntimeRecoverySummary;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct ActiveRunSummary {
    pub owner: String,
    pub pid: u32,
    pub started_at: String,
    pub lock_path: String,
}

#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub turn_count: usize,
    pub checkpoint_count: usize,
    pub checkpoint_covered_turns: usize,
    pub tail_turns: usize,
    pub latest_checkpoint_at: Option<String>,
    pub latest_checkpoint_reason: Option<String>,
    pub context_chars: usize,
    pub context_limit_chars: usize,
    pub context_ratio: f32,
    pub context_prompt_tokens: usize,
    pub context_window_tokens: usize,
    pub context_token_ratio: f32,
    pub usage: UsageSnapshot,
    pub compaction: Option<CompactionSummary>,
    pub recovery: RecoverySnapshot,
    pub context_epoch: Option<ContextEpochSummary>,
    pub session_memory: Option<SessionMemorySummary>,
    pub tool_history: ToolHistorySummary,
    pub runtime_recovery: RuntimeRecoverySummary,
    pub dynamic_sources: Vec<DynamicContextSource>,
    pub projection_warnings: Vec<String>,
    pub active_run: Option<ActiveRunSummary>,
    /// 最近一轮从首次思考/正文到结束的耗时（毫秒）；未设置时为 0
    pub last_turn_duration_ms: u64,
}

impl StateStore {
    /// 读取当前会话状态快照。
    ///
    /// 参数:
    /// - `context_limit_chars`: 当前模型上下文窗口字符数
    ///
    /// 返回:
    /// - 会话状态快照
    pub fn session_snapshot(&self, context_limit_chars: usize) -> Result<SessionSnapshot> {
        let projection = self.project_session_summary(context_limit_chars)?;
        self.audit_runtime_sequence_gaps()?;
        crate::runtime_recovery::audit_dead_process_owners(&self.conv_db, &self.session_id)?;
        crate::runtime_recovery::audit_stale_subagent_owners(
            &self.conv_db,
            &self.session_id,
            std::process::id(),
        )?;
        let estimated_context_chars = projection.estimate.state_context_chars;
        let usage = projection.stats.usage;
        let api_prompt_tokens = usage
            .last_conversation_usage
            .as_ref()
            .map(|usage| usage.prompt_tokens as usize)
            .filter(|tokens| *tokens > 0);
        // 有 provider 回报时优先使用真实 prompt_tokens；否则用 o200k 预估当前会话上下文。
        let context_prompt_tokens = match api_prompt_tokens {
            Some(tokens) => tokens,
            None => self.estimate_session_context_tokens()?,
        };
        let mut projection_warnings: Vec<String> = projection
            .warnings
            .iter()
            .map(|warning| warning.message.clone())
            .collect();
        if projection.stats.checkpoint_count >= 2 {
            projection_warnings.push(
                "当前会话已经多次压缩，上下文细节可能逐步损失；复杂任务建议新建会话继续"
                    .to_string(),
            );
        }
        let session_memory = super::session_memory::repository::load_memory(
            &self.conv_db,
            &projection.stats.session_id,
        )?
        .map(super::session_memory::summary::summarize_memory);
        let latest_checkpoint_reason = {
            let conn = self.conv_db.conn.lock().unwrap();
            super::checkpoints::load_latest_checkpoint(&conn)?
                .map(|checkpoint| match checkpoint.reason {
                    super::checkpoints::CheckpointReason::Auto => "auto",
                    super::checkpoints::CheckpointReason::Manual => "manual",
                    super::checkpoints::CheckpointReason::Legacy => "legacy",
                })
                .map(str::to_string)
        };
        Ok(SessionSnapshot {
            session_id: projection.stats.session_id,
            turn_count: projection.stats.turn_count,
            checkpoint_count: projection.stats.checkpoint_count,
            checkpoint_covered_turns: projection.stats.checkpoint_covered_turns,
            tail_turns: projection.stats.tail_turns,
            latest_checkpoint_at: projection.stats.latest_checkpoint_at,
            latest_checkpoint_reason,
            context_chars: estimated_context_chars,
            context_limit_chars,
            context_ratio: context_ratio(estimated_context_chars, context_limit_chars),
            context_prompt_tokens,
            context_window_tokens: context_limit_chars,
            context_token_ratio: context_ratio(context_prompt_tokens, context_limit_chars),
            usage,
            compaction: projection.compaction,
            recovery: projection.recovery,
            context_epoch: self.context_epoch_summary()?,
            session_memory,
            tool_history: self.tool_history_summary()?,
            runtime_recovery: crate::runtime_recovery::session_summary(
                &self.conv_db,
                &self.session_id,
            )?,
            dynamic_sources: Vec::new(),
            projection_warnings,
            active_run: None,
            last_turn_duration_ms: 0,
        })
    }

    /// 在没有 provider usage 时，用 o200k 预估当前会话上下文 token。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 预估 prompt token 数
    fn estimate_session_context_tokens(&self) -> Result<usize> {
        let history = super::checkpoints::project_history(&self.conv_db, &self.session_id, None)?;
        let mut parts = Vec::new();
        if let Some(context) = history.checkpoint_context.as_ref() {
            parts.push(context.clone());
        }
        for message in &history.messages {
            if let Ok(serialized) = serde_json::to_string(message) {
                parts.push(serialized);
            }
        }
        let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
        Ok(crate::token_estimate::estimate_texts_tokens(&refs) as usize)
    }
}

/// 计算上下文占用比例。
///
/// 参数:
/// - `context_chars`: 当前上下文估算字符数
/// - `context_limit_chars`: 当前模型上下文预算字符数
///
/// 返回:
/// - 上下文占用比例
pub fn context_ratio(context_chars: usize, context_limit_chars: usize) -> f32 {
    if context_limit_chars == 0 {
        return 0.0;
    }
    context_chars as f32 / context_limit_chars as f32
}

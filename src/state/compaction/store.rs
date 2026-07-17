use super::{CompactionRequest, CompactionSummary};
use crate::llm::ChatMessage;
use crate::state::request_projection::{
    estimate_projected_request_chars, project_provider_turn_from_messages, ProjectedRequest,
};
use crate::state::tool_history::build_budgeted_summary_history;
use crate::state::StateStore;
use anyhow::{bail, Result};

const SUMMARY_PROMPT_FIXED_RESERVE_CHARS: usize = 512;

/// 压缩应用结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionApplyOutcome {
    Applied,
    RejectedOverBudget,
}

/// 压缩写入前的预算预检结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactionBudgetCheck {
    pub context_chars: usize,
    pub context_limit_chars: usize,
    pub result_chars: usize,
}

impl CompactionBudgetCheck {
    /// 判断压缩后重新投影是否超过预算。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否超过上下文预算
    pub fn is_over_budget(&self) -> bool {
        self.result_chars > self.context_limit_chars
    }
}

impl StateStore {
    /// 按当前请求上下文选择统一压缩轮次。
    ///
    /// 参数:
    /// - `messages`: 当前请求消息列表
    /// - `context_limit_tokens`: 当前模型上下文窗口 token 数
    /// - `force`: 是否由手动入口强制触发
    ///
    /// 返回:
    /// - 压缩请求；自动入口未达到九成或旧轮次不足时返回空
    #[allow(dead_code)]
    pub fn select_compaction_for_messages(
        &self,
        messages: &[ChatMessage],
        context_limit_tokens: usize,
        force: bool,
    ) -> Result<Option<CompactionRequest>> {
        let projection = project_provider_turn_from_messages(messages, 0, context_limit_tokens);
        self.select_compaction_for_projection(&projection, force)
    }

    /// 按 provider 请求投影视图选择统一压缩轮次。
    ///
    /// 参数:
    /// - `projection`: 当前 provider 请求投影视图
    /// - `force`: 是否由手动入口强制触发
    ///
    /// 返回:
    /// - 压缩请求；自动入口未达到九成或旧轮次不足时返回空
    pub fn select_compaction_for_projection(
        &self,
        projection: &ProjectedRequest,
        force: bool,
    ) -> Result<Option<CompactionRequest>> {
        let current_context_tokens = estimate_projected_request_chars(projection);
        let context_limit_tokens = projection.estimate.context_limit_chars;
        let turns = self.conv_db.load_turns()?;
        let previous_summary = self
            .load_authoritative_compaction_summary()?
            .map(|summary| summary.summary);
        Ok(super::select_compaction(
            &turns,
            previous_summary,
            current_context_tokens,
            context_limit_tokens,
            force,
        ))
    }

    /// 测试用：使用统一手动策略选择旧轮次。
    ///
    /// 参数:
    /// - `_legacy_keep_tail_turns`: 旧测试参数，不再影响统一策略
    ///
    /// 返回:
    /// - 压缩请求，没有可压缩旧轮次时返回空
    #[cfg(test)]
    pub fn select_manual_compaction(
        &self,
        _legacy_keep_tail_turns: usize,
    ) -> Result<Option<CompactionRequest>> {
        let turns = self.conv_db.load_turns()?;
        let previous_summary = self
            .load_authoritative_compaction_summary()?
            .map(|summary| summary.summary);
        Ok(super::select_compaction(
            &turns,
            previous_summary,
            0,
            1,
            true,
        ))
    }

    /// 构造带工具历史预算的压缩摘要提示词。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `context_limit_chars`: 当前模型上下文窗口字符数
    ///
    /// 返回:
    /// - 可发送给压缩模型的提示词
    pub(crate) fn build_compaction_summary_prompt(
        &self,
        request: &CompactionRequest,
        context_limit_chars: usize,
    ) -> Result<String> {
        let overhead = super::prompt::build_summary_prompt_from_history(
            request.previous_summary.as_deref(),
            "",
        )
        .chars()
        .count();
        let history_budget = context_limit_chars
            .saturating_sub(overhead)
            .saturating_sub(super::summary_char_limit(context_limit_chars))
            .saturating_sub(SUMMARY_PROMPT_FIXED_RESERVE_CHARS);
        let history = build_budgeted_summary_history(
            &self.conv_db,
            &self.session_id,
            Some(&self.state_dir),
            &request.compact_turns,
            history_budget,
        )?;
        let prompt = super::prompt::build_summary_prompt_from_history(
            request.previous_summary.as_deref(),
            &history.history,
        );
        let prompt_chars = prompt.chars().count();
        if history.replacement_missing_count > 0 {
            self.record_recovery_failure(
                request.compact_turn_ids.last().map(String::as_str),
                crate::state::FailureKind::ToolHistoryReplacementMissing,
                crate::state::RecoveryStatus::Observed,
                &format!(
                    "压缩摘要输入发现 {} 个工具输出引用缺少稳定 replacement，已回退使用 result_preview",
                    history.replacement_missing_count
                ),
                0,
                prompt_chars,
                context_limit_chars,
            )?;
        }
        if history.result_ref_missing_file_count > 0 {
            self.record_recovery_failure(
                request.compact_turn_ids.last().map(String::as_str),
                crate::state::FailureKind::ToolHistoryReplacementMissing,
                crate::state::RecoveryStatus::Observed,
                &format!(
                    "压缩摘要输入发现 {} 个工具完整输出引用文件缺失，已回退使用 result_preview",
                    history.result_ref_missing_file_count
                ),
                0,
                prompt_chars,
                context_limit_chars,
            )?;
        }
        if prompt_chars > context_limit_chars
            || (history.history.is_empty() && request.turn_count() > 0)
        {
            bail!(
                "tool history summary prompt over budget: prompt_chars={prompt_chars}, context_limit_chars={context_limit_chars}, history_budget_chars={history_budget}"
            );
        }
        Ok(prompt)
    }

    /// 应用自动压缩结果。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `summary`: 模型生成的摘要正文
    ///
    /// 返回:
    /// - 应用是否成功
    pub fn apply_compaction(&self, request: &CompactionRequest, summary: &str) -> Result<()> {
        self.apply_compaction_with_reason(
            request,
            summary,
            crate::state::checkpoints::CheckpointReason::Auto,
        )
    }

    /// 使用明确原因应用压缩结果。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `summary`: 模型生成的摘要正文
    /// - `reason`: 自动或手动压缩原因
    ///
    /// 返回:
    /// - 应用是否成功
    fn apply_compaction_with_reason(
        &self,
        request: &CompactionRequest,
        summary: &str,
        reason: crate::state::checkpoints::CheckpointReason,
    ) -> Result<()> {
        let previous_count = {
            let conn = self.conv_db.conn.lock().unwrap();
            crate::state::checkpoints::load_latest_checkpoint(&conn)?
                .map(|checkpoint| checkpoint.source_turn_count)
                .unwrap_or_default()
        };
        let source_turn_count = request.source_turn_count_after_compaction(previous_count);
        crate::state::checkpoints::apply_checkpoint_compaction(
            &self.conv_db,
            request,
            summary,
            source_turn_count,
            reason,
        )?;
        if let Err(error) =
            super::save_summary(&self.compaction_summary_file(), summary, source_turn_count)
        {
            self.record_recovery_failure(
                request.compact_turn_ids.last().map(String::as_str),
                crate::state::FailureKind::CompactionMirrorFailed,
                crate::state::RecoveryStatus::Observed,
                &format!("权威 checkpoint 已提交，但旧摘要兼容镜像写入失败: {error:#}"),
                0,
                0,
                0,
            )?;
        }
        self.resolve_active_compaction_failures()?;
        Ok(())
    }

    /// 在预算内应用压缩结果。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `summary`: 模型生成的摘要正文
    /// - `context_chars`: 当前上下文字符估算
    /// - `context_limit_chars`: 上下文预算字符数
    ///
    /// 返回:
    /// - 压缩应用结果
    pub fn apply_compaction_with_budget_guard(
        &self,
        request: &CompactionRequest,
        summary: &str,
        projection: &ProjectedRequest,
        exclude_turn_id: Option<&str>,
    ) -> Result<CompactionApplyOutcome> {
        let budget = self.compaction_budget_check(request, summary, projection, exclude_turn_id)?;
        if budget.is_over_budget() {
            self.record_auto_compaction_failure(
                request.compact_turn_ids.last().map(String::as_str),
                crate::state::FailureKind::CompactionOverBudget,
                &format!(
                    "compaction reprojected provider request over budget: result_chars={}, context_limit_chars={}",
                    budget.result_chars, budget.context_limit_chars
                ),
                budget.context_chars,
                budget.context_limit_chars,
            )?;
            return Ok(CompactionApplyOutcome::RejectedOverBudget);
        }
        self.apply_compaction(request, summary)?;
        Ok(CompactionApplyOutcome::Applied)
    }

    /// 使用统一投影预算检查应用手动压缩结果。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `summary`: 模型生成的摘要正文
    /// - `projection`: 当前 provider 请求投影视图
    /// - `exclude_turn_id`: 当前运行中轮次标识
    ///
    /// 返回:
    /// - 压缩应用结果
    pub fn apply_manual_compaction_with_projection_guard(
        &self,
        request: &CompactionRequest,
        summary: &str,
        projection: &ProjectedRequest,
        exclude_turn_id: Option<&str>,
    ) -> Result<CompactionApplyOutcome> {
        let budget = self.compaction_budget_check(request, summary, projection, exclude_turn_id)?;
        if budget.is_over_budget() {
            self.record_manual_compaction_failure(
                crate::state::FailureKind::CompactionOverBudget,
                &format!(
                    "manual compaction reprojected provider request over budget: result_chars={}, context_limit_chars={}",
                    budget.result_chars, budget.context_limit_chars
                ),
                budget.context_chars,
                budget.context_limit_chars,
            )?;
            return Ok(CompactionApplyOutcome::RejectedOverBudget);
        }
        self.apply_compaction_with_reason(
            request,
            summary,
            crate::state::checkpoints::CheckpointReason::Manual,
        )?;
        Ok(CompactionApplyOutcome::Applied)
    }

    /// 在预算内应用手动压缩结果。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `summary`: 模型生成的摘要正文
    /// - `context_limit_chars`: 上下文预算字符数
    ///
    /// 返回:
    /// - 压缩应用结果
    #[allow(dead_code)]
    pub fn apply_manual_compaction_with_budget_guard(
        &self,
        request: &CompactionRequest,
        summary: &str,
        context_limit_chars: usize,
    ) -> Result<CompactionApplyOutcome> {
        let budget = self.manual_compaction_budget_check(request, summary, context_limit_chars)?;
        if budget.is_over_budget() {
            self.record_manual_compaction_failure(
                crate::state::FailureKind::CompactionOverBudget,
                &format!(
                    "manual compaction reprojected history over budget: result_chars={}, context_limit_chars={}",
                    budget.result_chars, budget.context_limit_chars
                ),
                budget.context_chars,
                budget.context_limit_chars,
            )?;
            return Ok(CompactionApplyOutcome::RejectedOverBudget);
        }
        self.apply_compaction_with_reason(
            request,
            summary,
            crate::state::checkpoints::CheckpointReason::Manual,
        )?;
        Ok(CompactionApplyOutcome::Applied)
    }

    /// 预检压缩写入后的 provider 请求预算。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `summary`: 模型生成的摘要正文
    /// - `projection`: 当前 provider 请求投影视图
    /// - `exclude_turn_id`: 当前运行中轮次标识
    ///
    /// 返回:
    /// - 预算预检结果
    pub fn compaction_budget_check(
        &self,
        request: &CompactionRequest,
        summary: &str,
        projection: &ProjectedRequest,
        exclude_turn_id: Option<&str>,
    ) -> Result<CompactionBudgetCheck> {
        let result_chars = self.estimate_reprojected_context_chars_after_compaction(
            request,
            summary,
            projection,
            exclude_turn_id,
        )?;
        Ok(CompactionBudgetCheck {
            context_chars: projection.estimate.message_chars,
            context_limit_chars: projection.estimate.context_limit_chars,
            result_chars,
        })
    }

    /// 预检手动压缩写入后的历史预算。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `summary`: 模型生成的摘要正文
    /// - `context_limit_chars`: 上下文预算字符数
    ///
    /// 返回:
    /// - 预算预检结果
    #[allow(dead_code)]
    pub fn manual_compaction_budget_check(
        &self,
        request: &CompactionRequest,
        summary: &str,
        context_limit_chars: usize,
    ) -> Result<CompactionBudgetCheck> {
        let context_chars = self.visible_history_context_chars(None)?;
        let result_chars = self.projected_history_chars_after_compaction(request, summary, None)?;
        Ok(CompactionBudgetCheck {
            context_chars,
            context_limit_chars,
            result_chars,
        })
    }

    /// 读取可注入上下文的压缩摘要消息。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 压缩摘要上下文消息
    pub fn compaction_summary_context(&self) -> Result<Option<String>> {
        Ok(self
            .load_authoritative_compaction_summary()?
            .map(|summary| super::summary_context_message(&summary.summary)))
    }

    /// 从 checkpoint 读取权威摘要，旧文件仅作为迁移兼容回退。
    ///
    /// 返回:
    /// - 当前权威压缩摘要
    pub(crate) fn load_authoritative_compaction_summary(
        &self,
    ) -> Result<Option<CompactionSummary>> {
        let checkpoint = {
            let conn = self.conv_db.conn.lock().unwrap();
            crate::state::checkpoints::load_latest_checkpoint(&conn)?
        };
        if let Some(checkpoint) = checkpoint {
            return Ok(Some(CompactionSummary {
                updated_at: checkpoint.created_at,
                compacted_turns: checkpoint.source_turn_count,
                summary: checkpoint.summary,
            }));
        }
        self.load_compaction_summary()
    }

    /// 读取压缩摘要。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 压缩摘要
    pub(crate) fn load_compaction_summary(&self) -> Result<Option<CompactionSummary>> {
        super::load_summary(&self.compaction_summary_file())
    }

    /// 清理压缩摘要。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 清理是否成功
    pub fn clear_compaction_summary(&self) -> Result<()> {
        super::clear_summary(&self.compaction_summary_file())
    }
}

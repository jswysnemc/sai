use super::{CompactionRequest, CompactionSummary};
use crate::llm::ChatMessage;
use crate::state::request_projection::{
    estimate_projected_request_chars, project_provider_turn_from_messages, ProjectedRequest,
};
use crate::state::tool_history::build_budgeted_summary_history_with_running;
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
        self.select_compaction_for_projection_with(
            &projection,
            force,
            super::CompactionBudgetPolicy::DEFAULT,
        )
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
        self.select_compaction_for_projection_with(
            projection,
            force,
            super::CompactionBudgetPolicy::DEFAULT,
        )
    }

    /// 按会话策略选择统一压缩轮次。
    ///
    /// 参数:
    /// - `projection`: 当前 provider 请求投影视图
    /// - `force`: 是否由手动入口强制触发
    /// - `policy`: 会话级压缩触发策略
    ///
    /// 返回:
    /// - 压缩请求；自动入口未达到阈值或旧轮次不足时返回空
    pub fn select_compaction_for_projection_with(
        &self,
        projection: &ProjectedRequest,
        force: bool,
        policy: super::CompactionBudgetPolicy,
    ) -> Result<Option<CompactionRequest>> {
        let current_context_tokens = estimate_projected_request_chars(projection);
        let context_limit_tokens = projection.estimate.context_limit_chars;
        let turns = self.conv_db.active_branch_turns()?;
        let previous_summary = self
            .load_authoritative_compaction_summary()?
            .map(|summary| summary.summary);
        // 运行中轮次的工具调用同样参与压缩，需要知道其中已记录多少条
        let (running_turn_call_count, already_compacted) = self.running_turn_call_counts(&turns)?;
        let request = super::select_compaction_with(
            &turns,
            previous_summary,
            running_turn_call_count,
            current_context_tokens,
            context_limit_tokens,
            force,
            policy,
        );
        // 压缩边界按累计记录：第二次压缩要接着上一次的位置往后推进
        Ok(request.map(|request| request.with_compacted_call_offset(already_compacted)))
    }

    /// 统计运行中轮次的工具调用总数与已被摘要覆盖的条数。
    ///
    /// 参数:
    /// - `turns`: 当前活动分支的全部轮次
    ///
    /// 返回:
    /// - （尚未覆盖的调用条数，已覆盖的调用条数）；没有运行中轮次时均为 0
    fn running_turn_call_counts(
        &self,
        turns: &[crate::state::turns::Turn],
    ) -> Result<(usize, usize)> {
        let Some(running) = turns
            .iter()
            .find(|turn| turn.status == crate::state::turns::TurnStatus::Running)
        else {
            return Ok((0, 0));
        };
        let total = self.tool_call_count_for_turn(&running.turn_id)?;
        let already_compacted = self
            .running_turn_compaction_boundary()?
            .filter(|(turn_id, _)| turn_id == &running.turn_id)
            .map(|(_, calls)| calls)
            .unwrap_or_default();
        Ok((total.saturating_sub(already_compacted), already_compacted))
    }

    /// 读取当前 checkpoint 记录的运行中轮次压缩边界。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 运行中轮次标识与已覆盖的工具调用条数
    pub(crate) fn running_turn_compaction_boundary(&self) -> Result<Option<(String, usize)>> {
        let conn = self.conv_db.conn.lock().unwrap();
        let checkpoint = crate::state::checkpoints::load_latest_checkpoint(&conn)?;
        drop(conn);
        Ok(checkpoint.and_then(|checkpoint| {
            checkpoint
                .running_turn_id
                .map(|turn_id| (turn_id, checkpoint.running_turn_compacted_calls))
        }))
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
        let turns = self.conv_db.active_branch_turns()?;
        let previous_summary = self
            .load_authoritative_compaction_summary()?
            .map(|summary| summary.summary);
        let (running_turn_call_count, already_compacted) = self.running_turn_call_counts(&turns)?;
        Ok(super::select_compaction_with(
            &turns,
            previous_summary,
            running_turn_call_count,
            0,
            1,
            true,
            super::CompactionBudgetPolicy::DEFAULT,
        )
        .map(|request| request.with_compacted_call_offset(already_compacted)))
    }

    /// 构造带工具历史预算的压缩摘要提示词。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `context_limit_chars`: 当前模型上下文窗口字符数
    /// - `template`: 上下文压缩的系统提示词和输入模板
    ///
    /// 返回:
    /// - 可发送给压缩模型的提示词
    pub(crate) fn build_compaction_summary_prompt(
        &self,
        request: &CompactionRequest,
        context_limit_chars: usize,
        template: &crate::config::PromptTemplateConfig,
    ) -> Result<crate::prompts::template::RenderedPrompt> {
        let empty_prompt = super::prompt::render_summary_prompt(
            template,
            request.previous_summary.as_deref(),
            "",
        )?;
        let overhead = empty_prompt.total_chars();
        let history_budget = context_limit_chars
            .saturating_sub(overhead)
            .saturating_sub(super::summary_char_limit(context_limit_chars))
            .saturating_sub(SUMMARY_PROMPT_FIXED_RESERVE_CHARS);
        // 运行中轮次的待压缩区间同样进入摘要输入，否则摘要覆盖不到即将删除的内容
        let running_turn = self.running_turn_for_summary(request)?;
        let history = build_budgeted_summary_history_with_running(
            &self.conv_db,
            &self.session_id,
            Some(&self.state_dir),
            &request.compact_turns,
            running_turn
                .as_ref()
                .map(|(turn, skip, take)| (turn, *skip, *take)),
            history_budget,
        )?;
        let prompt = super::prompt::render_summary_prompt(
            template,
            request.previous_summary.as_deref(),
            &history.history,
        )?;
        let prompt_chars = prompt.total_chars();
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

    /// 解析运行中轮次在本次摘要输入中的区间。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    ///
    /// 返回:
    /// - （轮次、跳过条数、覆盖条数）；本次不压缩运行轮次时为空
    fn running_turn_for_summary(
        &self,
        request: &CompactionRequest,
    ) -> Result<Option<(crate::state::turns::Turn, usize, usize)>> {
        let Some(running) = request.running_turn.as_ref() else {
            return Ok(None);
        };
        let Some(turn) = self
            .conv_db
            .active_branch_turns()?
            .into_iter()
            .find(|turn| turn.turn_id == running.turn_id)
        else {
            return Ok(None);
        };
        // 上一次压缩已覆盖的部分不再重复送入摘要
        let already_compacted = self
            .running_turn_compaction_boundary()?
            .filter(|(turn_id, _)| turn_id == &running.turn_id)
            .map(|(_, calls)| calls)
            .unwrap_or_default();
        let take = running.compacted_calls.saturating_sub(already_compacted);
        Ok((take > 0).then_some((turn, already_compacted, take)))
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
        // 1. 压缩后历史已变，旧的主对话 prompt_tokens 不再代表当前上下文
        // 2. 清空后 session_snapshot / 系统用量会回退到基于投影历史的实时估算
        self.clear_last_usage()?;
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

use super::{Agent, AgentEvent};
use crate::llm::{ChatMessage, OpenAiCompatibleClient};
use crate::perf_trace::PerfTrace;
use crate::state::request_projection::{
    estimate_projected_request_chars, project_provider_turn_from_messages,
};
use crate::state::{
    classify_context_pressure, compaction_trigger_chars, ContextPressure, FailureKind,
    RecoveryStatus, StateStore, ToolResultMaintenanceMode,
};
use anyhow::Result;

impl Agent {
    /// 按上下文压力分级维护会话：先免费改写陈旧工具结果，仍超限再摘要压缩。
    ///
    /// 分级策略参考 DeepSeek-Reasonix：陈旧工具结果可以重新获取，改写它们
    /// 不需要调用摘要模型，是免费的上下文回收。六成压力先裁剪首尾；九成
    /// 压力先整体折叠，折叠省出的空间足够回落到阈值以下时，本轮的付费摘要
    /// 调用被完全跳过。
    ///
    /// 参数:
    /// - `turn_id`: 当前运行中轮次标识
    /// - `messages`: 当前即将发送给模型的消息列表
    ///
    /// 返回:
    /// - 历史是否被改写（改写后调用方需重建消息）
    pub(super) async fn compact_conversation_if_needed(
        &self,
        turn_id: &str,
        messages: &[ChatMessage],
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<bool> {
        let projection = project_provider_turn_from_messages(messages, 0, self.context_char_budget);
        if !self.state.should_attempt_auto_compaction()? {
            return Ok(false);
        }
        let context_chars = estimate_projected_request_chars(&projection);
        let context_limit_chars = projection.estimate.context_limit_chars;
        let maintained = match classify_context_pressure(context_chars, context_limit_chars) {
            ContextPressure::Relaxed => return Ok(false),
            ContextPressure::SnipStale => {
                // 1. 免费档：只裁剪，不触发摘要压缩
                let stats = self
                    .state
                    .maintain_stale_tool_results(ToolResultMaintenanceMode::Snip)?;
                return Ok(stats.rewritten > 0);
            }
            ContextPressure::Compact => {
                // 2. 压缩档：先折叠陈旧结果，省够空间就跳过付费摘要
                let stats = self
                    .state
                    .maintain_stale_tool_results(ToolResultMaintenanceMode::Prune)?;
                if stats.rewritten > 0
                    && context_chars.saturating_sub(stats.saved_chars)
                        < compaction_trigger_chars(context_limit_chars)
                {
                    return Ok(true);
                }
                stats.rewritten > 0
            }
        };
        let Some(request) = self
            .state
            .select_compaction_for_projection(&projection, false)?
        else {
            return Ok(maintained);
        };
        let applied = self
            .execute_compaction(&request, &projection, Some(turn_id), false, on_event)
            .await?;
        Ok(applied || maintained)
    }

    /// provider 上下文溢出后尝试一次压缩恢复。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `messages`: 触发溢出的 provider 消息
    /// - `err`: provider 错误
    /// - `input`: 当前用户输入
    /// - `image_urls`: 图片 data URL 列表
    /// - `memory_index_prompt`: 可选记忆索引注入文本
    /// - `auto_meme_reminder`: 可选自动表情包提醒
    /// - `on_event`: 压缩流式事件回调
    ///
    /// 返回:
    /// - 是否已经压缩并允许重试
    pub(super) async fn recover_after_provider_overflow(
        &mut self,
        turn_id: &str,
        messages: &[ChatMessage],
        err: &anyhow::Error,
        input: &str,
        image_urls: &[String],
        memory_index_prompt: Option<&str>,
        auto_meme_reminder: Option<&str>,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<bool> {
        let projection = project_provider_turn_from_messages(messages, 0, self.context_char_budget);
        self.state.record_provider_overflow_recovery(
            Some(turn_id),
            FailureKind::ProviderOverflow,
            RecoveryStatus::Recovering,
            &format!("{err:#}"),
            projection.estimate.message_chars,
            projection.estimate.context_limit_chars,
        )?;
        let Some(request) = self
            .state
            .select_compaction_for_projection(&projection, true)?
        else {
            self.record_overflow_retry_failed(turn_id, messages, err)?;
            return Ok(false);
        };
        if !self
            .execute_compaction(&request, &projection, Some(turn_id), false, on_event)
            .await?
        {
            self.record_overflow_retry_failed(turn_id, messages, err)?;
            return Ok(false);
        }
        let mut reprojected = self.chat_messages_for_turn(
            turn_id,
            input,
            image_urls,
            memory_index_prompt,
            auto_meme_reminder,
        )?;
        reprojected.extend(self.state.project_running_turn_tool_messages(turn_id)?);
        let reprojected_projection =
            project_provider_turn_from_messages(&reprojected, 0, self.context_char_budget);
        self.state.record_provider_overflow_recovery(
            Some(turn_id),
            FailureKind::ProviderOverflow,
            RecoveryStatus::Reprojected,
            "provider overflow recovery compacted history and rebuilt request projection",
            reprojected_projection.estimate.message_chars,
            reprojected_projection.estimate.context_limit_chars,
        )?;
        Ok(true)
    }

    /// 记录 provider overflow 重试失败。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `messages`: 失败时 provider 消息
    /// - `err`: provider 错误
    ///
    /// 返回:
    /// - 写入是否成功
    pub(super) fn record_overflow_retry_failed(
        &self,
        turn_id: &str,
        messages: &[ChatMessage],
        err: &anyhow::Error,
    ) -> Result<()> {
        let projection = project_provider_turn_from_messages(messages, 0, self.context_char_budget);
        self.state.record_provider_overflow_recovery(
            Some(turn_id),
            FailureKind::OverflowRetryFailed,
            RecoveryStatus::Terminal,
            &format!("{err:#}"),
            projection.estimate.message_chars,
            projection.estimate.context_limit_chars,
        )
    }

    /// 后台启动 Session Memory 模型提取。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(super) fn spawn_session_memory_extraction(&self) {
        // 1. 全局记忆关闭时跳过会话记忆提取
        if !self.config.memory_config().enabled {
            return;
        }
        let state = self.state.clone();
        let paths = self.paths.clone();
        let context_char_budget = self.context_char_budget;
        // 记忆提取可配置独立模型；未配置时沿用当前会话 client
        let client = match self.config.session_memory_runtime_config() {
            Ok(runtime_config) => {
                match crate::llm::OpenAiCompatibleClient::from_config(&runtime_config, &paths) {
                    Ok(client) => client,
                    Err(err) => {
                        eprintln!("[sai] session memory extraction client fallback: {err:#}");
                        self.client.clone()
                    }
                }
            }
            Err(err) => {
                eprintln!("[sai] session memory extraction config fallback: {err:#}");
                self.client.clone()
            }
        };
        tokio::spawn(async move {
            let _ =
                extract_session_memory_with_model(state, client, paths, context_char_budget).await;
        });
    }
}

/// 使用独立模型请求提取 Session Memory。
///
/// 参数:
/// - `state`: 状态仓储
/// - `client`: 模型客户端
/// - `context_char_budget`: 当前主模型上下文窗口字符预算
///
/// 返回:
/// - 提取是否成功
async fn extract_session_memory_with_model(
    state: StateStore,
    client: OpenAiCompatibleClient,
    paths: crate::paths::SaiPaths,
    context_char_budget: usize,
) -> Result<bool> {
    let mut perf = PerfTrace::new("session-memory");
    perf.mark("start");
    let Some(input) = state.prepare_session_memory_model_extraction(context_char_budget)? else {
        perf.mark("skip");
        return Ok(false);
    };
    perf.mark("prepared");
    let messages = vec![
        ChatMessage::system(
            "You are a session memory extraction worker. Update durable conversation memory only. Do not answer the user task.",
        ),
        ChatMessage::plain("user", input.prompt.clone()),
    ];
    let _http_debug_session = crate::llm::HttpDebugSessionGuard::new(state.session_id());
    let result = match client
        .chat_stream_events(messages, Vec::new(), |_| Ok(()))
        .await
    {
        Ok(result) => result,
        Err(err) => {
            state.record_session_memory_model_extraction_failure(&input, &format!("{err:#}"))?;
            perf.mark("failed");
            return Ok(false);
        }
    };
    perf.mark("model done");
    if let Some(usage) = &result.usage {
        state.add_auxiliary_usage(usage)?;
        let _ = crate::usage_history::record_model_call(
            &paths,
            crate::usage_history::UsageRecordInput {
                provider_id: client.provider_id(),
                provider_name: client.provider_name(),
                model: client.model(),
                source: "session_memory",
                operation: "extract",
                status: "success",
                usage: Some(usage),
                usage_source: "provider_reported",
                started_at: chrono::Utc::now().timestamp(),
                duration_ms: 0,
                session_id: Some(state.session_id()),
                error_kind: None,
            },
        );
    }
    state.apply_session_memory_model_extraction(&input, &result.content)?;
    perf.mark("applied");
    Ok(true)
}

/// 识别压缩失败类型。
///
/// 参数:
/// - `err`: 压缩错误
///
/// 返回:
/// - 失败类型
pub(super) fn classify_compaction_error(err: &anyhow::Error) -> FailureKind {
    let message = format!("{err:#}");
    if message.contains("compaction summary is empty") {
        FailureKind::EmptySummary
    } else if message.contains("tool history summary prompt over budget") {
        FailureKind::ToolHistoryPromptOverBudget
    } else {
        FailureKind::CompactionLlmFailed
    }
}

/// 判断 provider 错误是否属于上下文溢出。
///
/// 参数:
/// - `err`: provider 错误
///
/// 返回:
/// - 是否属于上下文溢出
pub(super) fn is_context_overflow_error(err: &anyhow::Error) -> bool {
    let message = format!("{err:#}").to_ascii_lowercase();
    [
        "context_length_exceeded",
        "maximum context",
        "context window",
        "context length",
        "too many tokens",
        "tokens exceed",
        "prompt is too long",
        "input is too long",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_empty_compaction_summary() {
        let err = anyhow::anyhow!("compaction summary is empty");

        assert_eq!(classify_compaction_error(&err), FailureKind::EmptySummary);
    }

    #[test]
    fn detects_context_overflow_errors() {
        let err = anyhow::anyhow!(
            "chat completions stream request failed (400): context_length_exceeded"
        );

        assert!(is_context_overflow_error(&err));
    }

    #[test]
    fn ignores_non_overflow_provider_errors() {
        let err = anyhow::anyhow!("chat completions stream request failed (401): invalid key");

        assert!(!is_context_overflow_error(&err));
    }
}

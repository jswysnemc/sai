use super::{Agent, AgentEvent};
use crate::llm::{ChatMessage, OpenAiCompatibleClient};
use crate::perf_trace::PerfTrace;
use crate::state::request_projection::project_provider_turn_from_messages;
use crate::state::{FailureKind, RecoveryStatus, StateStore};
use anyhow::Result;

impl Agent {
    /// 按当前请求上下文估算自动压缩旧会话。
    ///
    /// 参数:
    /// - `turn_id`: 当前运行中轮次标识
    /// - `messages`: 当前即将发送给模型的消息列表
    ///
    /// 返回:
    /// - 是否执行了压缩
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
        let Some(request) = self
            .state
            .select_compaction_for_projection(&projection, false)?
        else {
            return Ok(false);
        };
        self.execute_compaction(&request, &projection, Some(turn_id), false, on_event)
            .await
    }

    /// provider 上下文溢出后尝试一次压缩恢复。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `messages`: 触发溢出的 provider 消息
    /// - `err`: provider 错误
    /// - `input`: 当前用户输入
    /// - `image_urls`: 图片 data URL 列表
    /// - `association_prompt`: 可选关联记忆上下文
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
        association_prompt: Option<&str>,
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
            association_prompt,
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
        let state = self.state.clone();
        let client = self.client.clone();
        let paths = self.paths.clone();
        let context_char_budget = self.context_char_budget;
        tokio::spawn(async move {
            let _ = extract_session_memory_with_model(state, client, paths, context_char_budget).await;
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

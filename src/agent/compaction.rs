use super::{Agent, AgentEvent, CompactionError};
use crate::i18n::text as t;
use crate::llm::{ChatMessage, ChatStreamEvent, ChatStreamKind};
use crate::state::request_projection::{project_provider_turn_from_messages, ProjectedRequest};
use crate::state::{CompactionApplyOutcome, CompactionRequest};
use anyhow::{Context, Result};

/// 手动压缩执行结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompactionRunOutcome {
    pub turn_count: usize,
    pub applied: bool,
}

impl Agent {
    /// 在工具轮次之间按需压缩，并重建当前运行轮次消息。
    ///
    /// 参数:
    /// - `tool_round`: 当前工具轮次
    /// - `turn_id`: 当前运行轮次标识
    /// - `messages`: 当前内存消息列表
    /// - `input`: 当前用户输入
    /// - `image_urls`: 当前用户图片
    /// - `association_prompt`: 关联记忆上下文
    /// - `auto_meme_reminder`: 自动表情提醒
    /// - `on_event`: 运行事件回调
    /// - `perf`: 性能追踪器
    ///
    /// 返回:
    /// - 是否应用了中途压缩
    pub(super) async fn compact_between_tool_rounds(
        &mut self,
        tool_round: usize,
        turn_id: &str,
        messages: &mut Vec<ChatMessage>,
        input: &str,
        image_urls: &[String],
        association_prompt: Option<&str>,
        auto_meme_reminder: Option<&str>,
        on_event: &mut impl FnMut(super::AgentEvent) -> Result<()>,
        perf: &mut crate::perf_trace::PerfTrace,
    ) -> Result<bool> {
        if tool_round <= 1
            || !self
                .compact_conversation_if_needed(turn_id, messages, on_event)
                .await?
        {
            return Ok(false);
        }
        let trailing_runtime_messages = messages
            .iter()
            .rev()
            .take_while(|message| message.role == "system")
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev();
        *messages = self.chat_messages_for_turn(
            turn_id,
            input,
            image_urls,
            association_prompt,
            auto_meme_reminder,
        )?;
        messages.extend(self.state.project_running_turn_tool_messages(turn_id)?);
        messages.extend(trailing_runtime_messages);
        perf.mark(&format!(
            "round {tool_round} rebuilt after mid-turn compaction"
        ));
        Ok(true)
    }

    /// 使用统一策略立即手动压缩当前会话。
    ///
    /// 参数:
    /// - `on_event`: 压缩流式事件回调
    ///
    /// 返回:
    /// - 手动压缩轮次数量与应用状态
    pub async fn compact_conversation_now(
        &self,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<CompactionRunOutcome> {
        let base = self.chat_base_context_projection(None)?;
        let projection =
            project_provider_turn_from_messages(&base.messages, 0, self.context_char_budget);
        let Some(request) = self
            .state
            .select_compaction_for_projection(&projection, true)?
        else {
            on_event(AgentEvent::CompactionStarted {
                turn_count: 0,
                model: self.compaction_model_label.clone(),
            })?;
            on_event(AgentEvent::CompactionFinished {
                applied: false,
                summary: None,
                error: None,
            })?;
            return Ok(CompactionRunOutcome {
                turn_count: 0,
                applied: false,
            });
        };
        let turn_count = request.turn_count();
        let applied = self
            .execute_compaction(&request, &projection, None, true, on_event)
            .await?;
        Ok(CompactionRunOutcome {
            turn_count,
            applied,
        })
    }

    /// 执行一次统一压缩并发送完整生命周期事件。
    ///
    /// 参数:
    /// - `request`: 已选择的旧轮次
    /// - `projection`: 压缩前 provider 请求投影
    /// - `exclude_turn_id`: 重新投影时排除的运行中轮次
    /// - `manual`: 是否由手动入口触发
    /// - `on_event`: 压缩流式事件回调
    ///
    /// 返回:
    /// - 压缩结果是否已经应用
    pub(super) async fn execute_compaction(
        &self,
        request: &CompactionRequest,
        projection: &ProjectedRequest,
        exclude_turn_id: Option<&str>,
        manual: bool,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<bool> {
        on_event(AgentEvent::CompactionStarted {
            turn_count: request.turn_count(),
            model: self.compaction_model_label.clone(),
        })?;
        let summary = match self.create_compaction_summary(request, on_event).await {
            Ok(summary) => summary,
            Err(error) => {
                self.record_compaction_failure(request, projection, manual, &error)?;
                on_event(AgentEvent::CompactionFinished {
                    applied: false,
                    summary: None,
                    error: Some(compaction_error(&error)),
                })?;
                return Ok(false);
            }
        };
        let outcome = if manual {
            self.state.apply_manual_compaction_with_projection_guard(
                request,
                &summary,
                projection,
                exclude_turn_id,
            )?
        } else {
            self.state.apply_compaction_with_budget_guard(
                request,
                &summary,
                projection,
                exclude_turn_id,
            )?
        };
        match outcome {
            CompactionApplyOutcome::Applied => {
                on_event(AgentEvent::CompactionFinished {
                    applied: true,
                    summary: Some(summary),
                    error: None,
                })?;
                Ok(true)
            }
            CompactionApplyOutcome::RejectedOverBudget => {
                let error = anyhow::anyhow!(
                    "compaction result still exceeds the active model context window"
                );
                on_event(AgentEvent::CompactionFinished {
                    applied: false,
                    summary: None,
                    error: Some(compaction_error(&error)),
                })?;
                Ok(false)
            }
        }
    }

    /// 使用压缩模型生成一次会话摘要。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `on_event`: 压缩流式事件回调
    ///
    /// 返回:
    /// - 校验通过的摘要正文
    async fn create_compaction_summary(
        &self,
        request: &CompactionRequest,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<String> {
        let prompt = self
            .state
            .build_compaction_summary_prompt(request, self.context_char_budget)?;
        let messages = vec![
            ChatMessage::system(
                "Summarize the supplied conversation for future turns. Return concise, faithful Markdown only and do not answer the user task.",
            ),
            ChatMessage::plain("user", prompt),
        ];
        let summary = self
            .request_compaction_summary(messages, on_event)
            .await
            .context("compaction model request failed")?;
        crate::state::validate_summary(
            &summary,
            crate::state::summary_char_limit(self.context_char_budget),
        )?;
        Ok(summary)
    }

    /// 请求压缩模型并转发正文增量。
    ///
    /// 参数:
    /// - `messages`: 压缩模型消息
    /// - `on_event`: 压缩流式事件回调
    ///
    /// 返回:
    /// - 完整摘要正文
    async fn request_compaction_summary(
        &self,
        messages: Vec<ChatMessage>,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<String> {
        let _http_debug_session = crate::llm::HttpDebugSessionGuard::new(self.state.session_id());
        let result = self
            .compaction_client
            .chat_stream_events(messages, Vec::new(), |event| match event {
                ChatStreamEvent::Chunk(chunk)
                    if chunk.kind == ChatStreamKind::Content && !chunk.text.is_empty() =>
                {
                    on_event(AgentEvent::CompactionDelta { text: chunk.text })
                }
                ChatStreamEvent::Chunk(_) | ChatStreamEvent::ToolCallProgress(_) => Ok(()),
            })
            .await?;
        if let Some(usage) = &result.usage {
            self.state.add_auxiliary_usage(usage)?;
        }
        Ok(result.content.trim().to_string())
    }

    /// 记录自动或手动压缩模型失败。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `projection`: 压缩前请求投影
    /// - `manual`: 是否为手动触发
    /// - `error`: 压缩错误
    ///
    /// 返回:
    /// - 写入是否成功
    fn record_compaction_failure(
        &self,
        request: &CompactionRequest,
        projection: &ProjectedRequest,
        manual: bool,
        error: &anyhow::Error,
    ) -> Result<()> {
        let kind = super::recovery::classify_compaction_error(error);
        let detail = format!("{error:#}");
        if manual {
            self.state.record_manual_compaction_failure(
                kind,
                &detail,
                projection.estimate.message_chars,
                projection.estimate.context_limit_chars,
            )
        } else {
            self.state.record_auto_compaction_failure(
                request.compact_turn_ids.last().map(String::as_str),
                kind,
                &detail,
                projection.estimate.message_chars,
                projection.estimate.context_limit_chars,
            )
        }
    }
}

/// 构造压缩失败的概要与详细错误。
///
/// 参数:
/// - `error`: 原始错误链
///
/// 返回:
/// - 用户可见压缩错误
fn compaction_error(error: &anyhow::Error) -> CompactionError {
    CompactionError {
        message: t("context compaction failed", "上下文压缩失败").to_string(),
        detail: format!("{error:#}"),
    }
}

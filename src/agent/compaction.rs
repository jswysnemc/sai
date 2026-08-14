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
            .take_while(|message| {
                message.role == "system"
                    || super::tool_attachments::is_pending_model_attachment(message)
            })
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
        &mut self,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<CompactionRunOutcome> {
        if self.uses_external_engine() {
            return self.compact_external_conversation(on_event).await;
        }
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
        let summary = match self.create_compaction_summary(request, projection, on_event).await {
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
                self.record_evicted_turns(request);
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

    /// 生成一次会话摘要并补齐程序填充的部分。
    ///
    /// 模型只写九节里的八节，第 6 节的用户原话与末尾的回读指引由程序补上：
    /// 这两处的价值都在于逐字准确，交给模型转述等于把唯一的保真部分也变成有损的。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `projection`: 压缩前 provider 请求投影
    /// - `on_event`: 压缩流式事件回调
    ///
    /// 返回:
    /// - 装配完成的摘要正文
    async fn create_compaction_summary(
        &self,
        request: &CompactionRequest,
        projection: &ProjectedRequest,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<String> {
        let summary = self
            .request_model_summary(request, projection, on_event)
            .await?;
        Ok(self.finalize_summary(request, &summary))
    }

    /// 把刚被压缩掉的轮次原文留档，供后续回读。
    ///
    /// 摘要是有损的，这份留档是它唯一的补救途径，也是摘要末尾那句
    /// 回读指引所指向的实际内容。写失败只记录不打断：压缩本身已经成功，
    /// 为留档失败回滚会让上下文继续溢出。
    ///
    /// 参数:
    /// - `request`: 本次压缩请求
    ///
    /// 返回:
    /// - 无
    fn record_evicted_turns(&self, request: &CompactionRequest) {
        let mut evicted = Vec::new();
        for turn in &request.compact_turns {
            if !turn.user_content.trim().is_empty() {
                evicted.push(crate::memory::EvictedTurn {
                    timestamp: turn.user_timestamp.clone(),
                    role: "user".to_string(),
                    content: turn.user_content.clone(),
                });
            }
            if !turn.assistant_content.trim().is_empty() {
                evicted.push(crate::memory::EvictedTurn {
                    timestamp: turn.assistant_timestamp.clone().unwrap_or_default(),
                    role: "assistant".to_string(),
                    content: turn.assistant_content.clone(),
                });
            }
        }
        if let Err(error) = self.memory.remember_evicted_turns(&evicted) {
            eprintln!("[sai] 压缩轮次留档失败: {error:#}");
        }
    }

    /// 把程序填充的小节与回读指引合进模型产出。
    ///
    /// 用户原话节的预算按摘要上限的四分之一切，并受绝对上限约束：
    /// 小窗口下摘要本身就短，这一节不能反过来把正文挤掉。
    ///
    /// 参数:
    /// - `request`: 压缩请求，提供被压缩轮次的用户原话
    /// - `summary`: 模型产出的摘要正文
    ///
    /// 返回:
    /// - 装配完成的摘要正文
    fn finalize_summary(&self, request: &CompactionRequest, summary: &str) -> String {
        let budget = crate::state::summary_char_limit(self.context_char_budget)
            .saturating_div(4)
            .min(super::compaction_schema::DEFAULT_USER_SECTION_BUDGET);
        let user_section =
            super::compaction_schema::user_messages_section(&request.compact_turns, budget);
        let pointer = super::compaction_schema::transcript_pointer(
            &self.state.session_id(),
            self.evicted_context_lookup_available(),
        );
        super::compaction_schema::assemble(summary, &user_section, pointer.as_deref())
    }

    /// 判断被压缩轮次的回读工具当前是否可用。
    ///
    /// 记忆关闭或工具整体禁用时 search_evicted_context 不会注册，
    /// 此时指向它只会让下一轮调用一个不存在的工具。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 工具可用时为真
    fn evicted_context_lookup_available(&self) -> bool {
        self.tools_enabled && self.config.memory_config().enabled
    }

    /// 使用压缩模型生成一次会话摘要。
    ///
    /// 优先复用会话前缀：摘要请求原样回放本轮消息、只在末尾追加指令，
    /// 供应商的热缓存因而可以一路命中。压缩模型与会话模型不同源、
    /// 或窗口余量不足以再放一次全量回放时，直接走裁剪过的独立请求。
    ///
    /// 参数:
    /// - `request`: 压缩请求
    /// - `projection`: 压缩前 provider 请求投影
    /// - `on_event`: 压缩流式事件回调
    ///
    /// 返回:
    /// - 校验通过的摘要正文
    async fn request_model_summary(
        &self,
        request: &CompactionRequest,
        projection: &ProjectedRequest,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<String> {
        if let Some(replay) = self.build_replay_summary_request(projection) {
            match self
                .summarize_with_messages(replay.messages, replay.definitions, "replay", on_event)
                .await
            {
                Ok(summary) => return Ok(summary),
                // 回放路径把会话自己的系统提示词与工具一并送出，模型有可能
                // 继续干活而不是写笔记。压缩本身不该因为这条优化而变得更易失败，
                // 因此退回独立请求再试一次，代价是多一次调用
                Err(error) => {
                    let _ = self.state.record_compaction_replay_fallback(&format!("{error:#}"));
                }
            }
        }
        let prompt = self.state.build_compaction_summary_prompt(
            request,
            self.context_char_budget,
            &self.config.prompt.templates.compaction,
        )?;
        let messages = vec![
            ChatMessage::system(prompt.system),
            ChatMessage::plain("user", prompt.user),
        ];
        self.summarize_with_messages(messages, Vec::new(), "standalone", on_event)
            .await
    }

    /// 发起一次摘要请求并校验结果。
    ///
    /// 参数:
    /// - `messages`: 压缩模型消息
    /// - `definitions`: 随请求发送的工具定义
    /// - `operation`: 用量统计里区分回放与独立请求的标记
    /// - `on_event`: 压缩流式事件回调
    ///
    /// 返回:
    /// - 校验通过的摘要正文
    async fn summarize_with_messages(
        &self,
        messages: Vec<ChatMessage>,
        definitions: Vec<crate::llm::ToolDefinition>,
        operation: &'static str,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<String> {
        let summary = self
            .request_compaction_summary(messages, definitions, operation, on_event)
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
    /// - `definitions`: 随请求发送的工具定义；独立请求路径下为空
    /// - `operation`: 用量统计里区分回放与独立请求的标记
    /// - `on_event`: 压缩流式事件回调
    ///
    /// 返回:
    /// - 完整摘要正文
    async fn request_compaction_summary(
        &self,
        messages: Vec<ChatMessage>,
        definitions: Vec<crate::llm::ToolDefinition>,
        operation: &'static str,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<String> {
        let _http_debug_session = crate::llm::HttpDebugSessionGuard::new(self.state.session_id());
        let result = self
            .compaction_client
            .chat_stream_events(messages, definitions, |event| match event {
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
            let _ = crate::usage_history::record_model_call(
                &self.paths,
                crate::usage_history::UsageRecordInput {
                    provider_id: self.compaction_client.provider_id(),
                    provider_name: self.compaction_client.provider_name(),
                    model: self.compaction_client.model(),
                    source: "compaction",
                    operation,
                    status: "success",
                    usage: Some(usage),
                    usage_source: "provider_reported",
                    started_at: chrono::Utc::now().timestamp(),
                    duration_ms: 0,
                    session_id: Some(self.state.session_id()),
                    error_kind: None,
                },
            );
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

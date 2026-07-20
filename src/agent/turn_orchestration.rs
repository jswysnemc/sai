use super::message_context::clean_user_visible_text;
use super::recovery::is_context_overflow_error;
use super::{Agent, AgentEvent};
use crate::llm::ChatResult;
use crate::perf_trace::PerfTrace;
use crate::state::PendingTurnGuard;
use crate::tools::memes;
use anyhow::Result;

impl Agent {
    /// 发送一轮带可选图片的流式对话。
    ///
    /// 参数:
    /// - `input`: 用户文本输入
    /// - `image_url`: 当前轮附加图片 data URL
    /// - `on_event`: 流式事件回调
    ///
    /// 返回:
    /// - 聊天结果
    #[allow(dead_code)]
    pub async fn chat_stream_with_image<F>(
        &mut self,
        input: &str,
        image_url: Option<String>,
        on_event: F,
    ) -> Result<ChatResult>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        self.chat_stream_with_images(
            input,
            image_url.into_iter().collect(),
            /*turn_id*/ None,
            on_event,
        )
        .await
    }

    /// 发送一轮带多张图片的流式对话。
    ///
    /// 参数:
    /// - `input`: 用户文本输入
    /// - `image_urls`: 当前轮图片 data URL 列表
    /// - `turn_id`: 调用方提供的可选稳定轮次标识
    /// - `on_event`: 流式事件回调
    ///
    /// 返回:
    /// - 聊天结果
    pub async fn chat_stream_with_images<F>(
        &mut self,
        input: &str,
        image_urls: Vec<String>,
        turn_id: Option<String>,
        on_event: F,
    ) -> Result<ChatResult>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        // HTTP 调试按会话落盘时绑定 session_id
        let _http_debug_session = crate::llm::HttpDebugSessionGuard::new(self.state.session_id());
        let input = clean_user_visible_text(input);
        let mut perf = PerfTrace::new("agent");
        perf.mark("start turn");
        let turn_id = turn_id.unwrap_or_else(new_turn_id);
        self.state.start_turn_with_images(&turn_id, &input, &image_urls)?;
        perf.mark("state start_turn");
        let mut guard = PendingTurnGuard::new(self.state.clone(), turn_id.clone());
        let worktree_undo = crate::state::worktree_undo::WorktreeUndoGuard::begin(
            &self.state,
            &crate::runtime_cwd::current_dir()?,
            &turn_id,
        )?;
        let auto_meme_plan =
            memes::plan_auto_meme_before_reply(&self.config, &self.paths, &self.client, &input)
                .await?;
        perf.mark("auto meme plan");
        let association_prompt = self
            .memory
            .association(&input)?
            .map(|association| self.memory.format_association(&association));
        perf.mark("memory association");
        let auto_meme_reminder = auto_meme_plan.as_ref().map(|plan| plan.reminder.as_str());
        let mut messages = self.chat_messages_for_turn(
            &turn_id,
            &input,
            &image_urls,
            association_prompt.as_deref(),
            auto_meme_reminder,
        )?;
        perf.mark("build initial messages");
        let mut on_event = on_event;
        let mut emit_event = Box::new(|event: AgentEvent| {
            if let AgentEvent::Chunk(chunk) = &event {
                guard.append_chunk(chunk.kind, &chunk.text);
            }
            on_event(event)
        });
        if self
            .compact_conversation_if_needed(&turn_id, &messages, &mut emit_event)
            .await?
        {
            perf.mark("compaction completed");
            messages = self.chat_messages_for_turn(
                &turn_id,
                &input,
                &image_urls,
                association_prompt.as_deref(),
                auto_meme_reminder,
            )?;
            perf.mark("rebuild messages after compaction");
        }
        let mut used_tools = Vec::new();
        let mut persisted_tool_reports = Vec::new();
        let result = match self
            .chat_with_tools(
                &turn_id,
                &mut messages,
                &mut used_tools,
                &mut persisted_tool_reports,
                &input,
                &image_urls,
                association_prompt.as_deref(),
                auto_meme_reminder,
                &mut emit_event,
                &mut perf,
            )
            .await
        {
            Ok(result) => result,
            Err(err) if is_context_overflow_error(&err) => {
                if !self
                    .recover_after_provider_overflow(
                        &turn_id,
                        &messages,
                        &err,
                        &input,
                        &image_urls,
                        association_prompt.as_deref(),
                        auto_meme_reminder,
                        &mut emit_event,
                    )
                    .await?
                {
                    return Err(err);
                }
                messages = self.chat_messages_for_turn(
                    &turn_id,
                    &input,
                    &image_urls,
                    association_prompt.as_deref(),
                    auto_meme_reminder,
                )?;
                if !used_tools.is_empty() {
                    messages.extend(self.state.project_running_turn_tool_messages(&turn_id)?);
                }
                match self
                    .chat_with_tools(
                        &turn_id,
                        &mut messages,
                        &mut used_tools,
                        &mut persisted_tool_reports,
                        &input,
                        &image_urls,
                        association_prompt.as_deref(),
                        auto_meme_reminder,
                        &mut emit_event,
                        &mut perf,
                    )
                    .await
                {
                    Ok(result) => result,
                    Err(retry_err) if is_context_overflow_error(&retry_err) => {
                        self.record_overflow_retry_failed(&turn_id, &messages, &retry_err)?;
                        return Err(retry_err);
                    }
                    Err(retry_err) => return Err(retry_err),
                }
            }
            Err(err) => return Err(err),
        };
        emit_event.as_mut()(AgentEvent::FlushContent)?;
        perf.mark("final content flushed");
        if let Some(plan) = auto_meme_plan {
            emit_event.as_mut()(AgentEvent::ExternalOutput)?;
            memes::render_auto_meme(&self.config, &self.paths, &plan.event).await?;
            memes::record_auto_meme_event(&self.config, &self.paths, &plan.event)?;
        }
        for (tool_name, report) in persisted_tool_reports {
            self.state
                .append_tool_report_context(&turn_id, &tool_name, &report)?;
        }
        perf.mark("persist tool reports");
        drop(emit_event);
        worktree_undo.finish()?;
        guard.complete(&result.content, result.reasoning.as_deref())?;
        perf.mark("complete turn");
        self.spawn_session_memory_extraction();
        perf.mark("session memory extraction spawned");
        self.memory.process_after_turn(&input, &result.content)?;
        perf.mark("memory process after turn");
        if let Some(usage) = &result.usage {
            self.state.add_usage(usage)?;
        }
        perf.mark("usage saved");
        Ok(result)
    }
}

/// 创建当前对话轮次标识。
///
/// 返回:
/// - 当前轮唯一标识
fn new_turn_id() -> String {
    format!(
        "turn_{}_{}",
        chrono::Utc::now().timestamp_millis(),
        rand::random::<u16>()
    )
}

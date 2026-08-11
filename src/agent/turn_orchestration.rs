use super::message_context::clean_user_visible_text;
use super::recovery::is_context_overflow_error;
use super::turn_settlement::settle_step;
use super::{Agent, AgentEvent, InterMessageSource};
use crate::llm::ChatResult;
use crate::perf_trace::PerfTrace;
use crate::state::PendingTurnGuard;
use crate::tools::memes;
use anyhow::Result;
use std::sync::Arc;

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
        self.chat_stream_with_images_and_inter_messages(
            input, image_urls, turn_id, None, false, on_event,
        )
        .await
    }

    /// 发送一轮对话，并在 provider 消息间隙消费排队消息和外部回执。
    ///
    /// 参数:
    /// - `input`: 用户文本输入
    /// - `image_urls`: 当前轮图片
    /// - `turn_id`: 可选稳定轮次标识
    /// - `inter_message_source`: 可选用户排队消息来源
    /// - `wait_for_external`: 最终回复后是否等待后台工作
    /// - `on_event`: 流式事件回调
    ///
    /// 返回:
    /// - 聊天结果
    pub(crate) async fn chat_stream_with_images_and_inter_messages<F>(
        &mut self,
        input: &str,
        image_urls: Vec<String>,
        turn_id: Option<String>,
        inter_message_source: Option<Arc<dyn InterMessageSource>>,
        wait_for_external: bool,
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
        // 停止标志按轮清零：上一轮的停止不能影响本轮的终态归因
        self.cancel_requested
            .store(false, std::sync::atomic::Ordering::SeqCst);
        // 配置了外部内核时整轮交出去执行；下方全部是 sai 自带内核的路径，
        // 两者只在这一处分流，原生行为不受影响
        if self.uses_external_engine() {
            return self
                .run_external_turn(&input, image_urls, turn_id, on_event)
                .await;
        }
        self.state
            .start_turn_with_images(&turn_id, &input, &image_urls)?;
        perf.mark("state start_turn");
        // 流式增量交由共享句柄累积：事件闭包与守卫各持一份，
        // 守卫不被闭包借走，错误路径可直接落终态
        let partial_content_sink = crate::state::PartialTurnSink::new();
        let mut guard = PendingTurnGuard::new(
            self.state.clone(),
            turn_id.clone(),
            partial_content_sink.clone(),
        )
        .with_cancel_flag(self.cancel_requested.clone());
        // 准备阶段的每一步都经由 settle_step：这些步骤原先直接 `?` 返回，
        // 守卫析构写入的占位文案会盖掉真实原因，界面只剩一句无法排查的提示
        let workspace_dir = settle_step(
            &mut guard,
            crate::runtime_cwd::current_dir().map_err(anyhow::Error::from),
        )?;
        let worktree_undo = settle_step(
            &mut guard,
            crate::state::worktree_undo::WorktreeUndoGuard::begin(
                &self.state,
                &workspace_dir,
                &turn_id,
            ),
        )?;
        let auto_meme_plan = settle_step(
            &mut guard,
            memes::plan_auto_meme_before_reply(&self.config, &self.paths, &self.client, &input)
                .await,
        )?;
        perf.mark("auto meme plan");
        // 结构化记忆按当前工作区召回，相关度不足时不注入
        let workspace = crate::runtime_cwd::current_dir()
            .ok()
            .map(|path| path.display().to_string());
        let association_prompt = settle_step(
            &mut guard,
            self.memory.recall_for_turn(&input, workspace.as_deref()),
        )?;
        perf.mark("memory association");
        let auto_meme_reminder = auto_meme_plan.as_ref().map(|plan| plan.reminder.as_str());
        let mut messages = settle_step(
            &mut guard,
            self.chat_messages_for_turn(
                &turn_id,
                &input,
                &image_urls,
                association_prompt.as_deref(),
                auto_meme_reminder,
            ),
        )?;
        perf.mark("build initial messages");
        let mut on_event = on_event;
        let chunk_sink = partial_content_sink.clone();
        let mut emit_event = Box::new(move |event: AgentEvent| {
            if let AgentEvent::Chunk(chunk) = &event {
                chunk_sink.append(chunk.kind, &chunk.text);
            }
            on_event(event)
        });
        let compacted = settle_step(
            &mut guard,
            self.compact_conversation_if_needed(&turn_id, &messages, &mut emit_event)
                .await,
        )?;
        if compacted {
            perf.mark("compaction completed");
            messages = settle_step(
                &mut guard,
                self.chat_messages_for_turn(
                    &turn_id,
                    &input,
                    &image_urls,
                    association_prompt.as_deref(),
                    auto_meme_reminder,
                ),
            )?;
            perf.mark("rebuild messages after compaction");
        }
        let mut used_tools = Vec::new();
        let execution = match self
            .chat_with_tools(
                &turn_id,
                &mut messages,
                &mut used_tools,
                &input,
                &image_urls,
                association_prompt.as_deref(),
                auto_meme_reminder,
                inter_message_source.as_deref(),
                wait_for_external,
                &mut emit_event,
                &mut perf,
            )
            .await
        {
            Ok(execution) => execution,
            Err(err) if is_context_overflow_error(&err) => {
                let recovered = settle_step(
                    &mut guard,
                    self.recover_after_provider_overflow(
                        &turn_id,
                        &messages,
                        &err,
                        &input,
                        &image_urls,
                        association_prompt.as_deref(),
                        auto_meme_reminder,
                        &mut emit_event,
                    )
                    .await,
                )?;
                if !recovered {
                    // 恢复失败时按失败落库，避免 UI 显示为用户中断
                    let _ = guard.fail_in_place(&crate::llm::error_detail_text(&err));
                    return Err(err);
                }
                messages = settle_step(
                    &mut guard,
                    self.chat_messages_for_turn(
                        &turn_id,
                        &input,
                        &image_urls,
                        association_prompt.as_deref(),
                        auto_meme_reminder,
                    ),
                )?;
                if !used_tools.is_empty() {
                    let tool_messages = settle_step(
                        &mut guard,
                        self.state.project_running_turn_tool_messages(&turn_id),
                    )?;
                    messages.extend(tool_messages);
                }
                match self
                    .chat_with_tools(
                        &turn_id,
                        &mut messages,
                        &mut used_tools,
                        &input,
                        &image_urls,
                        association_prompt.as_deref(),
                        auto_meme_reminder,
                        inter_message_source.as_deref(),
                        wait_for_external,
                        &mut emit_event,
                        &mut perf,
                    )
                    .await
                {
                    Ok(execution) => execution,
                    Err(retry_err) if is_context_overflow_error(&retry_err) => {
                        let recorded =
                            self.record_overflow_retry_failed(&turn_id, &messages, &retry_err);
                        let _ = guard
                            .fail_in_place(&crate::llm::error_detail_text(&retry_err));
                        recorded?;
                        return Err(retry_err);
                    }
                    Err(retry_err) => {
                        let _ = guard
                            .fail_in_place(&crate::llm::error_detail_text(&retry_err));
                        return Err(retry_err);
                    }
                }
            }
            Err(err) => {
                // 请求/工具链路失败：落失败状态，保留真实错误给时间线
                let _ = guard.fail_in_place(&crate::llm::error_detail_text(&err));
                return Err(err);
            }
        };
        let result = execution;
        settle_step(&mut guard, emit_event.as_mut()(AgentEvent::FlushContent))?;
        perf.mark("final content flushed");
        if let Some(plan) = auto_meme_plan {
            settle_step(&mut guard, emit_event.as_mut()(AgentEvent::ExternalOutput))?;
            settle_step(
                &mut guard,
                memes::render_auto_meme(&self.config, &self.paths, &plan.event).await,
            )?;
            settle_step(
                &mut guard,
                memes::record_auto_meme_event(&self.config, &self.paths, &plan.event),
            )?;
        }
        drop(emit_event);
        // 模型已产出完整回复，先落终态再做收尾：
        // 收尾环节失败不应让这一轮显示成中断
        guard.complete(&result.content, result.reasoning.as_deref())?;
        perf.mark("complete turn");
        worktree_undo.finish();
        self.spawn_session_memory_extraction();
        perf.mark("session memory extraction spawned");
        // 长期记忆抽取要发模型请求，异步执行不阻塞用户可见的答复
        self.spawn_memory_capture(&input, &result.content);
        self.memory.process_after_turn(&input, &result.content)?;
        perf.mark("memory process after turn");
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

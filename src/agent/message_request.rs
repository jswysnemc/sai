use super::{Agent, AgentEvent};
use crate::llm::{ChatMessage, ChatResult, ChatStreamEvent, ChatStreamKind, ToolDefinition};
use crate::perf_trace::PerfTrace;
use anyhow::Result;

impl Agent {
    /// 执行一次 provider 流式请求并处理传输层重试。
    ///
    /// 参数:
    /// - `turn_id`: 持久化轮次标识
    /// - `messages`: 已完成顺序整理的 provider 消息
    /// - `definitions`: 当前请求可见工具定义
    /// - `round`: 当前模型子轮编号
    /// - `on_event`: Agent 事件接收器
    /// - `perf`: 性能记录器
    ///
    /// 返回:
    /// - provider 聊天结果
    pub(super) async fn request_model_round<F>(
        &mut self,
        turn_id: &str,
        messages: Vec<ChatMessage>,
        definitions: Vec<ToolDefinition>,
        round: usize,
        on_event: &mut F,
        perf: &mut PerfTrace,
    ) -> Result<ChatResult>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        let events = self.tools.plugin_events();
        events
            .model_round(
                serde_json::json!({"turn_id": turn_id, "round": round}),
                self.request_model_round_inner(
                    turn_id,
                    messages,
                    definitions,
                    round,
                    on_event,
                    perf,
                ),
            )
            .await
    }

    /// 【Agent】【模型传输】执行模型请求及有限重试，生命周期由外层统一配对。
    /// @param turn_id、round 为逻辑轮次；messages、definitions 为请求；on_event 为流式接收器；perf 为计时器
    /// @returns 实际 provider 响应或传输错误
    async fn request_model_round_inner<F>(
        &mut self,
        turn_id: &str,
        messages: Vec<ChatMessage>,
        definitions: Vec<ToolDefinition>,
        round: usize,
        on_event: &mut F,
        perf: &mut PerfTrace,
    ) -> Result<ChatResult>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        let mut saw_reasoning = false;
        let mut saw_content = false;
        let mut saw_tool_progress = false;
        perf.mark(&format!("round {round} model request start"));
        let _http_debug_request = crate::llm::HttpDebugRequestContextGuard::new(turn_id, round);
        let cleanup = self.streamed_tools.cleanup.clone();
        self.streamed_tools = Default::default();
        self.streamed_tools.cleanup = cleanup.clone();
        let mut streamed = super::streamed_tools::StreamedTools::default();
        streamed.cleanup = cleanup.clone();
        streamed.cancel = self.cancel_requested.clone();
        streamed.accounting = self.goal_accounting.clone();
        let cancel = self.cancel_requested.clone();
        let tools = self.tools.clone();
        let visibility = self.tool_visibility.clone();
        let accounting = self.goal_accounting.clone();
        let cwd = crate::runtime_cwd::current_dir()?;
        let response =
            self.client
                .chat_stream_events_retry(messages, definitions, &self.config.retry, |event| {
                    match event {
                        ChatStreamEvent::ToolCallReady { index, call } => {
                            if accounting
                                .as_ref()
                                .map(|value| value.current_goal())
                                .transpose()?
                                .flatten()
                                .is_none_or(|goal| goal.status.is_active())
                            {
                                streamed.ready(index, call, &tools, &visibility, &cwd);
                            }
                            Ok(())
                        }
                        ChatStreamEvent::Chunk(chunk) => {
                            match chunk.kind {
                                ChatStreamKind::Reasoning if !saw_reasoning => {
                                    saw_reasoning = true;
                                    perf.mark(&format!("round {round} first reasoning chunk"));
                                }
                                ChatStreamKind::Content if !saw_content => {
                                    saw_content = true;
                                    perf.mark(&format!("round {round} first content chunk"));
                                }
                                _ => {}
                            }
                            on_event(AgentEvent::Chunk(chunk))
                        }
                        ChatStreamEvent::ToolCallProgress(progress) => {
                            if !saw_tool_progress {
                                saw_tool_progress = true;
                                perf.mark(&format!("round {round} first tool args chunk"));
                            }
                            on_event(AgentEvent::ToolCallProgress(progress))
                        }
                        ChatStreamEvent::Reset {
                            content_chars,
                            reasoning_chars,
                        } => {
                            streamed = Default::default();
                            streamed.cleanup = cleanup.clone();
                            streamed.cancel = cancel.clone();
                            streamed.accounting = accounting.clone();
                            on_event(AgentEvent::ResponseReset {
                                content_chars,
                                reasoning_chars,
                                compaction: false,
                            })
                        }
                        ChatStreamEvent::Reconnecting {
                            attempt,
                            max_attempts,
                        } => on_event(AgentEvent::Reconnecting {
                            attempt,
                            max_attempts,
                        }),
                    }
                })
                .await;
        let result = match response {
            Err(error) => {
                if let Some(empty) = error.downcast_ref::<crate::llm::EmptyModelResponse>() {
                    if self
                        .goal_accounting
                        .as_ref()
                        .map(|accounting| accounting.current_goal())
                        .transpose()?
                        .flatten()
                        .is_some_and(|goal| goal.status.is_active())
                    {
                        return Ok(empty.0.clone());
                    }
                }
                return Err(error);
            }
            Ok(result) => result,
        };
        streamed.retain_confirmed(&result.tool_calls);
        self.streamed_tools = streamed;
        perf.mark(&format!("round {round} model request done"));
        Ok(result)
    }
}

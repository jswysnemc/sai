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
        let mut saw_reasoning = false;
        let mut saw_content = false;
        let mut saw_tool_progress = false;
        perf.mark(&format!("round {round} model request start"));
        // 输出开始前允许瞬时断连重试；开始输出后重试会复制正文，因此立即返回错误
        let emitted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        // 重试策略来自配置：次数、首拍间隔与间隔方式都可在设置的运行时页调整
        let retry = &self.config.retry;
        let max_attempts = retry.clamped_max_attempts();
        let initial_delay = retry.clamped_initial_delay();
        let fixed_backoff = retry.is_fixed_backoff();
        let mut attempt = 0u32;
        let result = loop {
            attempt += 1;
            let _http_debug_request = crate::llm::HttpDebugRequestContextGuard::new(turn_id, round);
            let emitted_flag = std::sync::Arc::clone(&emitted);
            let request_result = self
                .client
                .chat_stream_events(messages.clone(), definitions.clone(), |event| match event {
                    ChatStreamEvent::Chunk(chunk) => {
                        emitted_flag.store(true, std::sync::atomic::Ordering::SeqCst);
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
                        emitted_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                        if !saw_tool_progress {
                            saw_tool_progress = true;
                            perf.mark(&format!("round {round} first tool args chunk"));
                        }
                        on_event(AgentEvent::ToolCallProgress(progress))
                    }
                })
                .await;
            match request_result {
                Ok(result) => break result,
                Err(error) => {
                    let can_retry = !emitted.load(std::sync::atomic::Ordering::SeqCst)
                        && crate::llm::is_transient_transport_error(&error)
                        && attempt < max_attempts;
                    if !can_retry {
                        return Err(error);
                    }
                    // 先通知 UI 正在重连，再退避等待；否则界面会像卡住一样没有反馈
                    on_event(AgentEvent::Reconnecting {
                        attempt,
                        max_attempts,
                    })?;
                    let delay_ms = if fixed_backoff {
                        initial_delay.as_millis() as u64
                    } else {
                        // 指数退避：首拍为配置间隔，之后逐次翻倍，封顶 8 倍防溢出
                        let shift = attempt.saturating_sub(1).min(3);
                        (initial_delay.as_millis() as u64).saturating_mul(1u64 << shift)
                    };
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
        };
        perf.mark(&format!("round {round} model request done"));
        Ok(result)
    }
}

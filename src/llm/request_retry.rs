use super::{
    ChatMessage, ChatResult, ChatStreamEvent, ChatStreamKind, OpenAiCompatibleClient,
    ToolDefinition,
};
use crate::config::RetryConfig;
use anyhow::Result;

impl OpenAiCompatibleClient {
    /// 【模型请求】【统一恢复】主代理、子代理和压缩共用重试预算及响应撤销事件
    /// 参数：历史、工具、重试配置、事件接收器；返回完整响应或最终错误
    pub(crate) async fn chat_stream_events_retry<F>(
        &self,
        messages: Vec<ChatMessage>,
        definitions: Vec<ToolDefinition>,
        retry: &RetryConfig,
        mut on_event: F,
    ) -> Result<ChatResult>
    where
        F: FnMut(ChatStreamEvent) -> Result<()>,
    {
        let max_attempts = retry.clamped_max_attempts();
        for attempt in 1..=max_attempts {
            let mut hook = crate::hooks::ModelAttempt::begin(attempt as usize).await;
            let mut content_chars = 0usize;
            let mut reasoning_chars = 0usize;
            let mut callback_failed = false;
            let result = self
                .chat_stream_events(messages.clone(), definitions.clone(), |event| {
                    if let ChatStreamEvent::Chunk(chunk) = &event {
                        match chunk.kind {
                            ChatStreamKind::Content => content_chars += chunk.text.chars().count(),
                            ChatStreamKind::Reasoning => {
                                reasoning_chars += chunk.text.chars().count()
                            }
                        }
                    }
                    let outcome = on_event(event);
                    callback_failed |= outcome.is_err();
                    outcome
                })
                .await;
            hook.finish(&result).await;
            match result {
                Ok(result) => return Ok(result),
                Err(error)
                    if !callback_failed
                        && attempt < max_attempts
                        && super::is_transient_transport_error(&error) =>
                {
                    // 1. 【模型请求】【失败撤销】只移除本次尝试的部分输出，完整历史与已完成工具保持不变
                    on_event(ChatStreamEvent::Reset {
                        content_chars,
                        reasoning_chars,
                    })?;
                    on_event(ChatStreamEvent::Reconnecting {
                        attempt,
                        max_attempts,
                    })?;
                    let multiplier = if retry.is_fixed_backoff() {
                        1
                    } else {
                        1 << attempt.saturating_sub(1).min(3)
                    };
                    let delay = super::http_error::retry_after(&error).unwrap_or_else(|| {
                        retry.clamped_initial_delay().saturating_mul(multiplier)
                    });
                    tokio::time::sleep(delay).await;
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!("retry configuration always allows at least one attempt")
    }
}

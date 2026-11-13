use super::SubagentRunner;
use crate::llm::{ChatMessage, ChatResult, ChatStreamEvent, ChatStreamKind, ToolDefinition};
use anyhow::Result;

impl SubagentRunner {
    /// 【子任务】【模型请求】复用插件生命周期和现有进度通道执行一次逻辑请求。
    /// @param messages 当前对话；definitions 为实际可见工具；round 为请求序号
    /// @returns 模型响应，失败时同样触发结束事件
    pub(super) async fn request_model_round(
        &self,
        messages: Vec<ChatMessage>,
        definitions: Vec<ToolDefinition>,
        round: usize,
    ) -> Result<ChatResult> {
        let retry = self
            .progressive_context
            .as_ref()
            .map(|(config, _)| config.retry.clone())
            .unwrap_or_default();
        let result = crate::hooks::model_scope(
            self.tools.hooks.clone(),
            self.tools.hook_context().with("round", round),
            self.tools.clone(),
            self.tools.plugin_events().model_round(
                serde_json::json!({"round":round,"kind":"subagent"}),
                self.client
                    .chat_stream_events_retry(messages, definitions, &retry, |event| {
                        match event {
                            ChatStreamEvent::Chunk(chunk) => match chunk.kind {
                                ChatStreamKind::Reasoning => self.progress.reasoning(&chunk.text),
                                ChatStreamKind::Content => self.progress.content(&chunk.text),
                            },
                            ChatStreamEvent::Reset {
                                content_chars,
                                reasoning_chars,
                            } => self.progress.reset_response(content_chars, reasoning_chars),
                            ChatStreamEvent::Reconnecting {
                                attempt,
                                max_attempts,
                            } => self
                                .progress
                                .phase(format!("Reconnecting... {attempt}/{max_attempts}")),
                            ChatStreamEvent::ToolCallProgress(_)
                            | ChatStreamEvent::ToolCallReady { .. } => {}
                        }
                        Ok(())
                    }),
            ),
        )
        .await?;
        if let Some(accounting) = &self.goal_accounting {
            accounting.record(result.usage.as_ref())?;
        }
        Ok(result)
    }
}

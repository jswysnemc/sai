use super::SubagentRunner;
use crate::llm::{ChatMessage, ChatResult, ChatStreamChunk, ChatStreamKind, ToolDefinition};
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
        self.tools
            .plugin_events()
            .model_round(
                serde_json::json!({"round":round,"kind":"subagent"}),
                self.client
                    .chat_stream(messages, definitions, |chunk: ChatStreamChunk| {
                        match chunk.kind {
                            ChatStreamKind::Reasoning => self.progress.reasoning(&chunk.text),
                            ChatStreamKind::Content => self.progress.content(&chunk.text),
                        }
                        Ok(())
                    }),
            )
            .await
    }
}

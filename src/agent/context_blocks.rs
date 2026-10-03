use super::Agent;
use crate::llm::ChatMessage;
use anyhow::Result;

impl Agent {
    /// 【上下文】【实验开关】确认当前内置模型可以使用上下文工具
    /// 参数: 无；返回是否启用实验
    pub(super) fn context_blocks_enabled(&self) -> bool {
        self.config.context.experimental_context_blocks
            && self.tools_enabled
            && !self.uses_external_engine()
    }

    /// 【上下文】【请求投影】应用已提交摘要并失效旧请求的占用记录
    /// 参数: messages 为即将发送的消息；返回是否替换正文
    pub(super) fn project_context_blocks(&self, messages: &mut [ChatMessage]) -> Result<bool> {
        if !self.context_blocks_enabled() {
            return Ok(false);
        }
        let changed = self.state.apply_context_blocks(messages)?;
        if changed {
            self.state.clear_last_conversation_usage()?;
        }
        Ok(changed)
    }

    /// 【上下文】【压缩提醒】在半窗口压力下追加一次提醒，保持历史与系统前缀稳定
    /// 参数: messages 为当前消息，reminded 为本轮提醒状态；返回处理结果
    pub(super) fn remind_context_blocks(
        &mut self,
        messages: &mut Vec<ChatMessage>,
        reminded: &mut bool,
    ) -> Result<()> {
        if !self.context_blocks_enabled() || *reminded {
            return Ok(());
        }
        let count = self.context_token_cache.count(messages);
        if count < self.context_char_budget / 2 {
            return Ok(());
        }
        *reminded = true;
        messages.push(ChatMessage::plain("user", "<system-reminder>Context is above half of the model window. If a subtask is finished, call context_status, then compress_context for eligible historical tool outputs you have already read. Preserve active work and exact identifiers. Use search_context and restore_context when exact archived details are needed. Continue the user's task; automatic compaction remains available.</system-reminder>"));
        Ok(())
    }
}

#[cfg(test)]
#[path = "context_blocks_tests.rs"]
mod tests;

use crate::llm::ChatContent;
use crate::state::StateStore;
use anyhow::Result;
use std::collections::HashMap;

impl StateStore {
    /// 【上下文】【候选投影】从正式历史投影中获取当前可见的工具正文
    /// 参数: 无；返回结果引用到文本的映射，包含未被全局摘要覆盖的运行子轮
    pub(super) fn projected_context_results(&self) -> Result<HashMap<String, String>> {
        let mut messages = self.project_history(None)?.messages;
        self.apply_context_blocks(&mut messages)?;
        Ok(messages
            .into_iter()
            .filter_map(|message| {
                if message.role != "tool" {
                    return None;
                }
                match (message.tool_call_id, message.content) {
                    (Some(id), Some(ChatContent::Text(text))) => Some((id, text)),
                    _ => None,
                }
            })
            .collect())
    }
}

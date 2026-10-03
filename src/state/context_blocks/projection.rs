use super::model::replacement;
use crate::llm::{ChatContent, ChatMessage};
use crate::state::StateStore;
use anyhow::Result;
use std::collections::{HashMap, HashSet};

impl StateStore {
    /// 【上下文】【摘要投影】只替换完整工具结果文本，不删除调用、不移动消息
    /// 参数: messages 为即将发送的消息；返回是否发生变化
    pub(crate) fn apply_context_blocks(&self, messages: &mut [ChatMessage]) -> Result<bool> {
        let present = messages
            .iter()
            .filter(|message| message.role == "tool")
            .filter(|message| matches!(message.content, Some(ChatContent::Text(_))))
            .filter_map(|message| message.tool_call_id.as_deref())
            .collect::<HashSet<_>>();
        let mut replacements = HashMap::new();
        for block in self.visible_context_blocks()? {
            // 1. 【上下文】【摘要投影】部分成员已被全局摘要覆盖时，不留下缺少主摘要的引用
            if !block
                .message_ids
                .iter()
                .all(|id| present.contains(id.as_str()))
            {
                continue;
            }
            for id in &block.message_ids {
                replacements.insert(id.clone(), replacement(&block, id));
            }
        }
        let mut changed = false;
        for message in messages.iter_mut().filter(|message| message.role == "tool") {
            let Some(value) = message
                .tool_call_id
                .as_ref()
                .and_then(|id| replacements.get(id))
            else {
                continue;
            };
            // 2. 【上下文】【摘要投影】多模态结果不改写，附件继续由原消息结构携带
            if let Some(ChatContent::Text(text)) = message.content.as_mut() {
                if text != value {
                    *text = value.clone();
                    changed = true;
                }
            }
        }
        Ok(changed)
    }
}

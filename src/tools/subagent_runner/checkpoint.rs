use super::*;
use crate::llm::ProviderAssistantContent;
use crate::tools::model_attachment::ToolModelAttachment;
use serde::{Deserialize, Serialize};

/// 【子任务】【恢复检查点】持久化消息时保留供应商签名，网络请求序列化仍使用 ChatMessage。
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct SavedMessage {
    message: ChatMessage,
    provider_content: Option<ProviderAssistantContent>,
}

impl SavedMessage {
    /// 【子任务】【恢复检查点】保存一条完整消息；参数为消息引用，返回持久化表示。
    pub(crate) fn new(message: &ChatMessage) -> Self {
        Self {
            message: message.clone(),
            provider_content: message.provider_content.clone(),
        }
    }

    /// 【子任务】【恢复检查点】还原消息及原始签名；无参数，返回模型消息。
    fn restore(&self) -> ChatMessage {
        self.message
            .clone()
            .with_provider_content(self.provider_content.clone())
    }
}

/// 【子任务】【恢复检查点】对话与收件箱消费水位共同提交，防止重启丢失用户指导。
#[derive(Clone, Default, Serialize, Deserialize)]
pub(crate) struct SubagentCheckpoint {
    pub(crate) messages: Vec<SavedMessage>,
    pub(crate) stats: SubagentStats,
    pub(crate) loaded_tools: Vec<String>,
    pub(crate) loaded_skills: Vec<String>,
    pub(crate) consumed_messages: usize,
    pub(crate) steps: usize,
    pub(crate) turn_complete: bool,
    pub(crate) pending_attachment: Option<SavedMessage>,
}

impl SubagentCheckpoint {
    /// 【子任务】【已完成恢复】读取已持久化的最终响应，避免重启后重新执行完成的任务段。
    /// 参数：无；返回最终响应，尚未完成时为空。
    pub(crate) fn completed_result(&self) -> Option<ChatResult> {
        if !self.turn_complete {
            return None;
        }
        let message = self.messages.last()?.restore();
        let crate::llm::ChatContent::Text(content) = message.content? else {
            return None;
        };
        Some(ChatResult {
            content,
            reasoning: message.reasoning_content,
            provider_content: message.provider_content,
            usage: None,
            tool_calls: Vec::new(),
            duration_ms: 0,
            ttft_ms: 0,
        })
    }

    /// 【子任务】【恢复检查点】补齐未确认调用，保留已完成调用的真实结果。
    /// 参数：无；返回修复了工具配对关系的对话，绝不重新执行中断调用。
    pub(crate) fn restore_messages(&self) -> Vec<ChatMessage> {
        let mut messages = self
            .messages
            .iter()
            .map(SavedMessage::restore)
            .collect::<Vec<_>>();
        let answered = messages
            .iter()
            .filter_map(|message| message.tool_call_id.as_deref())
            .collect::<std::collections::HashSet<_>>();
        let missing = messages
            .iter()
            .flat_map(|message| message.tool_calls.iter().flatten())
            .filter(|call| !answered.contains(call.id.as_str()))
            .map(|call| call.id.clone())
            .collect::<Vec<_>>();
        for id in missing {
            messages.push(ChatMessage::tool(id, "Execution was interrupted before its result was durably recorded. The operation may have taken effect. Inspect the actual state before deciding whether to retry; do not blindly repeat side effects."));
        }
        if let Some(attachment) = &self.pending_attachment {
            messages.push(attachment.restore());
        }
        messages
    }
}

impl SubagentRunner {
    /// 【子任务】【恢复检查点】绑定持久化记录；参数为子任务标识，返回执行器。
    pub(crate) fn with_checkpoint(mut self, id: &str) -> Self {
        self.checkpoint_id = Some(id.to_string());
        self
    }

    /// 【子任务】【恢复检查点】恢复完整会话或初始化新任务。
    /// 参数：首条任务指令；返回消息、统计和当前允许的工具加载状态。
    pub(crate) fn restore_session(
        &self,
        prompt: &str,
    ) -> Result<(Vec<ChatMessage>, SubagentStats, ToolVisibility)> {
        let mut visibility = self.fresh_tool_visibility();
        if let Some(saved) = self.load_checkpoint()? {
            visibility.restore_loaded_tools(&self.tools, &saved.loaded_tools);
            visibility.restore_loaded_skills(&saved.loaded_skills);
            return Ok((saved.restore_messages(), saved.stats, visibility));
        }
        Ok((
            self.initial_messages(prompt),
            SubagentStats::default(),
            visibility,
        ))
    }

    /// 【子任务】【恢复检查点】读取当前记录；无参数，返回最近一次成功保存的检查点。
    pub(super) fn load_checkpoint(&self) -> Result<Option<SubagentCheckpoint>> {
        self.checkpoint_id
            .as_deref()
            .map(crate::tools::subagent_state::load_checkpoint)
            .transpose()
            .map(Option::flatten)
    }

    /// 【子任务】【恢复检查点】在请求和工具边界保存可恢复状态。
    /// 参数：对话、统计、工具可见性、本段调用数、是否完成、待追加附件；返回持久化结果。
    pub(super) fn save_checkpoint(
        &self,
        messages: &[ChatMessage],
        stats: &SubagentStats,
        visibility: &ToolVisibility,
        steps: usize,
        turn_complete: bool,
        attachments: &[ToolModelAttachment],
    ) -> Result<()> {
        let Some(id) = &self.checkpoint_id else {
            return Ok(());
        };
        let checkpoint = SubagentCheckpoint {
            messages: messages.iter().map(SavedMessage::new).collect(),
            stats: stats.clone(),
            loaded_tools: visibility.loaded_tool_names(),
            loaded_skills: visibility.loaded_skill_names(),
            consumed_messages: 0,
            steps,
            turn_complete,
            pending_attachment: crate::tools::model_attachment::attachment_message(attachments)
                .as_ref()
                .map(SavedMessage::new),
        };
        crate::tools::subagent_state::save_checkpoint(id, checkpoint)
    }
}

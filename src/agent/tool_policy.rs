use super::Agent;
use crate::state::turn_messages::{NewTurnMessage, TurnMessageKind};
use crate::{llm::ChatMessage, tools::PluginToolPolicyStates};
use anyhow::Result;

impl Agent {
    /// 【工具策略】【上下文追加】收集回调文字，整批工具结束后统一持久化
    /// @param states 本循环状态；name 为实际工具；arguments 为参数；ok 为结果；reminders 为当前批次提醒
    /// @returns 无；策略失败只产生诊断
    pub(super) async fn after_tool_policies(
        &self,
        states: &mut PluginToolPolicyStates,
        name: &str,
        arguments: &str,
        ok: bool,
        reminders: &mut Vec<String>,
    ) {
        for reminder in self
            .tools
            .after_plugin_tool(states, name, arguments, ok)
            .await
        {
            reminders.push(reminder);
        }
    }

    /// 【工具策略】【提醒持久化】按工具批次边界保存已发送提醒，重放时保持原位置。
    /// @param turn_id 为轮次；after_tool_seq 为完整工具批次末尾；reminders 为提醒；messages 为模型上下文
    /// @returns 保存结果
    pub(super) fn persist_tool_reminders(
        &self,
        turn_id: &str,
        after_tool_seq: usize,
        reminders: Vec<String>,
        messages: &mut Vec<ChatMessage>,
    ) -> Result<()> {
        for content in reminders {
            self.state.record_turn_message(NewTurnMessage {
                turn_id: turn_id.into(),
                after_tool_seq,
                kind: TurnMessageKind::ContextReminder,
                model_content: content.clone(),
                display_content: String::new(),
                reasoning: None,
                image_urls: Vec::new(),
            })?;
            messages.push(ChatMessage::plain("user", content));
        }
        Ok(())
    }
}

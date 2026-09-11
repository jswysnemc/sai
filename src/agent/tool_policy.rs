use super::Agent;
use crate::{llm::ChatMessage, tools::PluginToolPolicyStates};

impl Agent {
    /// 【工具策略】【上下文追加】把通用回调文字追加到当前工具循环，保留原工具结果
    /// @param states 本循环状态；name 为实际工具；arguments 为参数；ok 为结果；messages 为请求历史
    /// @returns 无；策略失败只产生诊断
    pub(super) async fn after_tool_policies(
        &self,
        states: &mut PluginToolPolicyStates,
        name: &str,
        arguments: &str,
        ok: bool,
        messages: &mut Vec<ChatMessage>,
    ) {
        for reminder in self
            .tools
            .after_plugin_tool(states, name, arguments, ok)
            .await
        {
            messages.push(ChatMessage::system(reminder));
        }
    }
}

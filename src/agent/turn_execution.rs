use crate::llm::{ChatMessage, ChatResult};

/// 【Agent】【工具子轮】构造需要回传给模型的 assistant 工具调用消息。
///
/// 参数:
/// - `result`: 当前模型子轮结果
///
/// 返回:
/// - 包含正文、工具调用和思考内容的 assistant 消息
pub fn assistant_tool_message(result: &ChatResult) -> ChatMessage {
    ChatMessage::assistant(result.content.clone(), Some(result.tool_calls.clone()))
        .with_reasoning(result.reasoning.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【Agent】【工具子轮】验证 assistant 工具消息保留 DeepSeek 思考内容。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn assistant_tool_message_preserves_reasoning() {
        let result = ChatResult {
            content: "准备调用工具".to_string(),
            reasoning: Some("先查询日期".to_string()),
            usage: None,
            tool_calls: Vec::new(),
            duration_ms: 0,
            ttft_ms: 0,
        };

        let message = assistant_tool_message(&result);

        assert_eq!(message.reasoning_content.as_deref(), Some("先查询日期"));
    }
}

#[cfg(test)]
mod stream_error_tests {
    use super::chat_stream_error_message;
    /// 【协议】【流式错误】验证流中途的错误对象被识别并提取说明。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn extracts_chat_stream_error_message() {
        let message = chat_stream_error_message(
            r#"{"error":{"message":"upstream rate limited","type":"rate_limit"}}"#,
        );

        assert_eq!(message.as_deref(), Some("upstream rate limited"));
    }

    /// 【协议】【流式错误】验证缺少 message 时回落到整个错误对象。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn falls_back_to_the_whole_error_object() {
        let message = chat_stream_error_message(r#"{"error":{"code":500}}"#);

        assert!(
            message.as_deref().is_some_and(|text| text.contains("500")),
            "缺少 message 时不应报出空原因"
        );
    }

    /// 【协议】【流式错误】验证正常增量与结束标记不被误判为错误。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn ignores_regular_stream_deltas() {
        assert!(
            chat_stream_error_message(r#"{"choices":[{"delta":{"content":"hello"}}]}"#).is_none()
        );
        assert!(chat_stream_error_message("[DONE]").is_none());
    }

    /// 【协议】【思考回传】验证只有开启的供应商才携带历史思考。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn preserved_thinking_is_stripped_unless_enabled() {
        use super::apply_preserved_thinking;
        use crate::config::ProviderConfig;
        use crate::llm::ChatMessage;

        let history = vec![
            ChatMessage::plain("user".to_string(), "question".to_string()),
            ChatMessage::plain("assistant".to_string(), "answer".to_string())
                .with_reasoning(Some("considered options".to_string())),
        ];

        let provider = ProviderConfig::default_openai();
        let stripped = apply_preserved_thinking(history.clone(), &provider);
        assert!(
            stripped
                .iter()
                .all(|message| message.reasoning_content.is_none()),
            "未开启时不应发送历史思考"
        );

        let mut preserved_provider = provider;
        preserved_provider.preserve_thinking = true;
        let kept = apply_preserved_thinking(history, &preserved_provider);
        assert_eq!(
            kept[1].reasoning_content.as_deref(),
            Some("considered options")
        );
    }

    /// 【协议】【DeepSeek】验证缺少思考内容的旧工具历史不会产生孤立 tool 消息。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn deepseek_omits_incomplete_legacy_tool_history() {
        use super::apply_preserved_thinking;
        use crate::config::ProviderConfig;
        use crate::llm::{ChatMessage, ToolCall, ToolCallFunction};

        let history = vec![
            ChatMessage::plain("user", "旧问题"),
            ChatMessage::assistant(
                "",
                Some(vec![ToolCall {
                    id: "call-1".to_string(),
                    kind: "function".to_string(),
                    function: ToolCallFunction {
                        name: "get_date".to_string(),
                        arguments: "{}".to_string(),
                    },
                }]),
            ),
            ChatMessage::tool("call-1", "2026-07-31"),
            ChatMessage::plain("assistant", "旧回答")
                .with_reasoning(Some("旧轮最终思考".to_string())),
            ChatMessage::plain("user", "新问题"),
        ];
        let mut provider = ProviderConfig::default_openai();
        provider.id = "deepseek".to_string();

        let prepared = apply_preserved_thinking(history, &provider);

        assert_eq!(prepared.len(), 3);
        assert!(prepared.iter().all(|message| message.role != "tool"));
        assert_eq!(
            prepared[1].reasoning_content.as_deref(),
            Some("旧轮最终思考")
        );
    }
}

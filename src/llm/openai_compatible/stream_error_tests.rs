#[cfg(test)]
mod stream_error_tests {
    use super::chat_stream_error_message;

    /// 【协议】【流式收尾】验证上游声明的终止原因被记录。
    ///
    /// 缺少 finish_reason 是判定"连接提前断开"的唯一依据，
    /// 解析不到就会把截断的回复当成完整回复。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn captures_upstream_finish_reason() {
        let mut content = String::new();
        let mut content_emitted = 0usize;
        let mut reasoning = String::new();
        let mut reasoning_emitted = 0usize;
        let mut usage = None;
        let mut tool_calls = super::ToolCallAccumulator::default();
        let mut finish_reason = None;
        let mut on_event = |_| Ok(());

        super::handle_sse_line(
            r#"data: {"choices":[{"delta":{"content":"done"},"finish_reason":"stop"}]}"#,
            &mut content,
            &mut content_emitted,
            &mut reasoning,
            &mut reasoning_emitted,
            &mut usage,
            &mut tool_calls,
            &mut finish_reason,
            &mut on_event,
        )
        .unwrap();

        assert_eq!(finish_reason.as_deref(), Some("stop"));
    }

    /// 【协议】【流式收尾】验证仍在增量输出的分片不会被当成已结束。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn leaves_finish_reason_unset_while_streaming() {
        let mut content = String::new();
        let mut content_emitted = 0usize;
        let mut reasoning = String::new();
        let mut reasoning_emitted = 0usize;
        let mut usage = None;
        let mut tool_calls = super::ToolCallAccumulator::default();
        let mut finish_reason = None;
        let mut on_event = |_| Ok(());

        super::handle_sse_line(
            r#"data: {"choices":[{"delta":{"content":"partial"},"finish_reason":null}]}"#,
            &mut content,
            &mut content_emitted,
            &mut reasoning,
            &mut reasoning_emitted,
            &mut usage,
            &mut tool_calls,
            &mut finish_reason,
            &mut on_event,
        )
        .unwrap();

        assert!(finish_reason.is_none());
    }

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

    /// 【协议】【流式收尾】验证只回报 usage、不写 finish_reason 时仍算正常结束。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn usage_without_finish_reason_completes_the_stream() {
        assert!(super::openai_stream_completed(
            None,
            Some(&crate::llm::Usage {
                prompt_tokens: 12,
                completion_tokens: 3,
                total_tokens: 15,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            })
        ));
        assert!(!super::openai_stream_completed(None, None));
        assert!(super::openai_stream_completed(Some("stop"), None));
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

    /// 【协议】【DeepSeek】验证关闭思考后保留普通工具历史。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn deepseek_disabled_thinking_keeps_tool_history_without_reasoning() {
        use super::apply_preserved_thinking;
        use crate::config::ProviderConfig;
        use crate::llm::{ChatMessage, ToolCall, ToolCallFunction};

        let history = vec![
            ChatMessage::plain("user", "查询日期"),
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
            ChatMessage::tool("call-1", "2026-08-01"),
        ];
        let mut provider = ProviderConfig::default_openai();
        provider.id = "deepseek".to_string();
        provider.thinking_level = "none".to_string();

        let prepared = apply_preserved_thinking(history, &provider);

        assert_eq!(prepared.len(), 3);
        assert!(prepared
            .iter()
            .all(|message| message.reasoning_content.is_none()));
        assert_eq!(prepared[2].role, "tool");
    }
}

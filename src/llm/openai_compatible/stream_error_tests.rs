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
}

    /// 【协议】【DeepSeek 用量】验证缓存命中字段进入统一用量模型。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn deepseek_usage_maps_prompt_cache_fields() {
        let raw = r#"{
            "choices": [],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 20,
                "total_tokens": 120,
                "prompt_cache_hit_tokens": 80,
                "prompt_cache_miss_tokens": 20
            }
        }"#;
        let parsed: ChatStreamResponse = serde_json::from_str(raw).unwrap();
        let usage = parsed.usage.unwrap().into_usage();

        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.cache_read_tokens, 80);
    }

    /// 【协议】【DeepSeek 用量】验证缺少 prompt 总量时按缓存命中与未命中之和补齐。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn deepseek_usage_recovers_prompt_total_from_cache_fields() {
        let raw = r#"{
            "choices": [],
            "usage": {
                "completion_tokens": 20,
                "total_tokens": 120,
                "prompt_cache_hit_tokens": 80,
                "prompt_cache_miss_tokens": 20
            }
        }"#;
        let parsed: ChatStreamResponse = serde_json::from_str(raw).unwrap();
        let usage = parsed.usage.unwrap().into_usage();

        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.total_tokens, 120);
        assert_eq!(usage.cache_read_tokens, 80);
    }

    /// 【协议】【DeepSeek 用量】验证流式请求显式要求结束块返回 usage。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn deepseek_chat_requests_stream_usage() {
        let provider = test_provider("deepseek", "https://api.deepseek.com");

        let options = chat_stream_options(&provider).expect("DeepSeek 应请求流式用量");

        assert!(options.include_usage);
    }

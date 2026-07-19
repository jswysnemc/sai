#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{ChatContent, ChatContentPart, ImageUrlContent};

    #[test]
    fn stream_chunk_accepts_null_tool_calls() {
        let raw = r#"{"choices":[{"delta":{"content":"在","tool_calls":null}}]}"#;
        let parsed: ChatStreamResponse = serde_json::from_str(raw).unwrap();

        assert_eq!(parsed.choices.len(), 1);
        assert_eq!(parsed.choices[0].delta.content.as_deref(), Some("在"));
        assert!(parsed.choices[0].delta.tool_calls.is_empty());
    }

    #[test]
    fn stream_chunk_accepts_taotoken_glm_nulls() {
        let raw = r#"{"created":1782742568,"usage":null,"model":"glm_for_coding","id":"9981f6121a31494387131c61bd2ad7a2","choices":[{"finish_reason":null,"matched_stop":null,"delta":{"role":null,"tool_calls":null,"content":"在","reasoning_content":null},"index":0,"logprobs":null}],"object":"chat.completion.chunk"}"#;
        let parsed: ChatStreamResponse = serde_json::from_str(raw).unwrap();

        assert!(parsed.usage.is_none());
        assert_eq!(parsed.choices.len(), 1);
        assert_eq!(parsed.choices[0].delta.content.as_deref(), Some("在"));
        assert!(parsed.choices[0].delta.reasoning_content.is_none());
        assert!(parsed.choices[0].delta.tool_calls.is_empty());
    }

    #[test]
    fn stream_chunk_emits_glm_reasoning_content() {
        let mut content = String::new();
        let mut content_emitted = 0usize;
        let mut reasoning = String::new();
        let mut reasoning_emitted = 0usize;
        let mut usage = None;
        let mut tool_calls = ToolCallAccumulator::default();
        let mut chunks = Vec::new();
        let mut on_chunk = |event| {
            if let ChatStreamEvent::Chunk(chunk) = event {
                chunks.push(chunk);
            }
            Ok(())
        };

        handle_sse_line(
            r#"data: {"choices":[{"delta":{"reasoning_content":"先想一下","content":null,"tool_calls":null}}]}"#,
            &mut content,
            &mut content_emitted,
            &mut reasoning,
            &mut reasoning_emitted,
            &mut usage,
            &mut tool_calls,
            &mut on_chunk,
        )
        .unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].kind, ChatStreamKind::Reasoning);
        assert_eq!(chunks[0].text, "先想一下");
    }

    #[test]
    fn taotoken_glm_request_enables_thinking() {
        let mut provider = test_provider("taotoken", "https://taotoken.net/api/v1");
        provider.default_model = "glm_for_coding".to_string();

        assert!(taotoken_glm_chat_template_kwargs(&provider)
            .is_some_and(|kwargs| kwargs.enable_thinking));
    }

    #[test]
    fn non_taotoken_glm_request_keeps_default_body() {
        let mut provider = test_provider("local", "http://localhost:11434/v1");
        provider.default_model = "glm-5".to_string();

        assert!(taotoken_glm_chat_template_kwargs(&provider).is_none());
    }

    #[test]
    fn openai_gpt5_uses_responses_api() {
        let mut provider = test_provider("openai", "https://api.openai.com/v1");
        provider.default_model = "gpt-5.5".to_string();
        let client = OpenAiCompatibleClient {
            client: reqwest::Client::new(),
            provider,
            api_key: "test".to_string(),
            http_debug: None,
        };

        assert!(client.uses_openai_responses());
    }

    #[test]
    fn openai_compatible_gpt5_tries_responses_api() {
        let mut provider = test_provider("taotoken", "https://taotoken.net/api/v1");
        provider.default_model = "gpt-5.5".to_string();
        let client = OpenAiCompatibleClient {
            client: reqwest::Client::new(),
            provider,
            api_key: "test".to_string(),
            http_debug: None,
        };

        assert!(client.uses_openai_responses());
    }

    #[test]
    fn responses_unsupported_allows_chat_fallback() {
        assert!(responses_unsupported(404, "not found"));
        assert!(responses_unsupported(400, "unsupported endpoint"));
        assert!(!responses_unsupported(401, "invalid api key"));
    }

    #[test]
    fn openai_tool_schema_flattens_top_level_any_of() {
        let schema = json!({
            "anyOf": [
                {"type":"object","properties":{"path":{"type":"string"}},"required":["path"]},
                {"type":"object","properties":{"resource":{"anyOf":[{"type":"string"},{"type":"null"}]}},"required":["resource"]}
            ]
        });

        let normalized = openai_tool_input_schema(schema);

        assert_eq!(normalized["type"], "object");
        assert_eq!(normalized["additionalProperties"], false);
        assert_eq!(normalized["properties"]["path"]["type"], "string");
        assert_eq!(normalized["properties"]["resource"]["type"], "string");
        assert!(normalized.get("anyOf").is_none());
    }

    #[test]
    fn responses_stream_emits_reasoning_and_content() {
        let mut content = String::new();
        let mut content_emitted = 0usize;
        let mut reasoning = String::new();
        let mut reasoning_emitted = 0usize;
        let mut usage = None;
        let mut content_started = false;
        let mut tool_calls = ResponsesToolAccumulator::default();
        let mut chunks = Vec::new();
        let mut on_chunk = |event| {
            if let ChatStreamEvent::Chunk(chunk) = event {
                chunks.push(chunk);
            }
            Ok(())
        };

        handle_responses_sse_line(
            r#"data: {"type":"response.reasoning_summary_text.delta","item_id":"rs_1","delta":"思考"}"#,
            &mut content,
            &mut content_emitted,
            &mut reasoning,
            &mut reasoning_emitted,
            &mut usage,
            &mut content_started,
            &mut tool_calls,
            &mut on_chunk,
        )
        .unwrap();
        handle_responses_sse_line(
            r#"data: {"type":"response.output_text.delta","item_id":"msg_1","delta":"答案"}"#,
            &mut content,
            &mut content_emitted,
            &mut reasoning,
            &mut reasoning_emitted,
            &mut usage,
            &mut content_started,
            &mut tool_calls,
            &mut on_chunk,
        )
        .unwrap();

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].kind, ChatStreamKind::Reasoning);
        assert_eq!(chunks[0].text, "思考");
        assert_eq!(chunks[1].kind, ChatStreamKind::Content);
        assert_eq!(chunks[1].text, "答案");
    }

    #[test]
    fn responses_reasoning_done_emits_content_boundary() {
        let mut content = String::new();
        let mut content_emitted = 0usize;
        let mut reasoning = String::new();
        let mut reasoning_emitted = 0usize;
        let mut usage = None;
        let mut content_started = false;
        let mut tool_calls = ResponsesToolAccumulator::default();
        let mut chunks = Vec::new();
        let mut on_chunk = |event| {
            if let ChatStreamEvent::Chunk(chunk) = event {
                chunks.push(chunk);
            }
            Ok(())
        };

        for line in [
            r#"data: {"type":"response.reasoning_summary_text.delta","item_id":"rs_1","delta":"思考"}"#,
            r#"data: {"type":"response.reasoning_summary_text.done","item_id":"rs_1"}"#,
            r#"data: {"type":"response.output_text.delta","item_id":"msg_1","delta":"答案"}"#,
            r#"data: {"type":"response.reasoning_summary_text.delta","item_id":"rs_1","delta":"晚到"}"#,
        ] {
            handle_responses_sse_line(
                line,
                &mut content,
                &mut content_emitted,
                &mut reasoning,
                &mut reasoning_emitted,
                &mut usage,
                &mut content_started,
                &mut tool_calls,
                &mut on_chunk,
            )
            .unwrap();
        }

        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0].kind, ChatStreamKind::Reasoning);
        assert_eq!(chunks[0].text, "思考");
        assert_eq!(chunks[1].kind, ChatStreamKind::Content);
        assert!(chunks[1].text.is_empty());
        assert_eq!(chunks[2].kind, ChatStreamKind::Content);
        assert_eq!(chunks[2].text, "答案");
        assert_eq!(chunks[3].kind, ChatStreamKind::Reasoning);
        assert_eq!(chunks[3].text, "晚到");
        assert_eq!(reasoning, "思考晚到");
    }

    #[test]
    fn stream_filter_skips_split_system_reminder() {
        let mut content = String::new();
        let mut emitted = 0usize;
        let mut chunks = Vec::new();
        let mut on_chunk = |event| {
            if let ChatStreamEvent::Chunk(chunk) = event {
                chunks.push(chunk);
            }
            Ok(())
        };

        push_buffered_chunk(
            &mut content,
            &mut emitted,
            ChatStreamKind::Content,
            "hello <system-rem".to_string(),
            &mut on_chunk,
        )
        .unwrap();
        push_buffered_chunk(
            &mut content,
            &mut emitted,
            ChatStreamKind::Content,
            "inder>hidden</system-reminder> world".to_string(),
            &mut on_chunk,
        )
        .unwrap();

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "hello ");
        assert_eq!(chunks[1].text, " world");
    }

    #[test]
    fn stream_filter_skips_underscore_system_reminder() {
        let mut content = String::new();
        let mut emitted = 0usize;
        let mut chunks = Vec::new();
        let mut on_chunk = |event| {
            if let ChatStreamEvent::Chunk(chunk) = event {
                chunks.push(chunk);
            }
            Ok(())
        };

        push_buffered_chunk(
            &mut content,
            &mut emitted,
            ChatStreamKind::Content,
            "a<system_reminder>hidden</system_reminder>b".to_string(),
            &mut on_chunk,
        )
        .unwrap();

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "a");
        assert_eq!(chunks[1].text, "b");
    }

    #[test]
    fn responses_stream_collects_tool_calls() {
        let mut content = String::new();
        let mut content_emitted = 0usize;
        let mut reasoning = String::new();
        let mut reasoning_emitted = 0usize;
        let mut usage = None;
        let mut content_started = false;
        let mut tool_calls = ResponsesToolAccumulator::default();
        let mut on_chunk = |_| Ok(());

        for line in [
            r#"data: {"type":"response.output_item.added","item":{"type":"function_call","id":"item_1","call_id":"call_1","name":"calc","arguments":""}}"#,
            r#"data: {"type":"response.function_call_arguments.delta","item_id":"call_1","delta":"{\"x\":"}"#,
            r#"data: {"type":"response.function_call_arguments.delta","item_id":"call_1","delta":"1}"}"#,
            r#"data: {"type":"response.output_item.done","item":{"type":"function_call","id":"item_1","call_id":"call_1","name":"calc","arguments":"{\"x\":1}"}}"#,
        ] {
            handle_responses_sse_line(
                line,
                &mut content,
                &mut content_emitted,
                &mut reasoning,
                &mut reasoning_emitted,
                &mut usage,
                &mut content_started,
                &mut tool_calls,
                &mut on_chunk,
            )
            .unwrap();
        }

        let calls = tool_calls.finish();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].function.name, "calc");
        assert_eq!(calls[0].function.arguments, r#"{"x":1}"#);
    }

    #[test]
    fn responses_request_shortens_long_call_ids_consistently() {
        let original = format!("call_{}", "x".repeat(78));
        let assistant = ChatMessage::assistant(
            "",
            Some(vec![ToolCall {
                id: original.clone(),
                kind: "function".to_string(),
                function: ToolCallFunction {
                    name: "calc".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
        );
        let input = lower_responses_messages(vec![assistant, ChatMessage::tool(&original, "ok")]);

        let call_id = input[0]["call_id"].as_str().unwrap();
        let result_id = input[1]["call_id"].as_str().unwrap();
        assert_eq!(call_id.chars().count(), RESPONSES_CALL_ID_MAX_CHARS);
        assert_eq!(call_id, result_id);
        assert_ne!(call_id, original);
    }

    #[test]
    fn responses_request_preserves_valid_call_ids() {
        assert_eq!(responses_call_id("call_1"), "call_1");
    }

    #[test]
    fn protocol_config_accepts_explicit_anthropic() {
        let mut provider = test_provider("anthropic", "https://api.anthropic.com/v1");
        provider.protocol = "anthropic".to_string();

        assert_eq!(
            ProviderProtocol::from_provider(&provider).unwrap(),
            ProviderProtocol::Anthropic
        );
    }

    #[test]
    fn protocol_config_accepts_messages_alias() {
        let mut provider = test_provider("claude", "https://api.anthropic.com/v1");
        provider.protocol = "messages".to_string();

        assert_eq!(
            ProviderProtocol::from_provider(&provider).unwrap(),
            ProviderProtocol::Anthropic
        );
    }

    #[test]
    fn protocol_config_is_case_insensitive_for_anthropic_aliases() {
        let mut provider = test_provider("anthropic", "https://api.anthropic.com/v1");

        for protocol in ["Anthropic-Messages", "CLAUDE-MESSAGES", "Claude-Code"] {
            provider.protocol = protocol.to_string();
            assert_eq!(
                ProviderProtocol::from_provider(&provider).unwrap(),
                ProviderProtocol::Anthropic
            );
        }
    }

    #[test]
    fn auto_protocol_detects_only_official_anthropic_provider() {
        let official = test_provider("anthropic", "https://api.anthropic.com/v1");
        let mut proxy = test_provider("openrouter", "https://openrouter.ai/api/v1");
        proxy.default_model = "anthropic/claude-sonnet-4-5".to_string();
        let named_proxy = test_provider("anthropic-proxy", "https://proxy.example.com/v1");

        assert!(provider_looks_official_anthropic(&official));
        assert!(!provider_looks_official_anthropic(&proxy));
        assert!(!provider_looks_official_anthropic(&named_proxy));
    }

    #[test]
    fn anthropic_web_search_strategy_hides_or_renames_local_tool() {
        let mut provider = test_provider("cpap", "https://cpap.example/v1");
        provider.default_model = "grok-4.5".to_string();
        provider.set_model_tags_for("grok-4.5", vec!["web_search".to_string()]);
        let tool = ToolDefinition {
            kind: "function",
            function: crate::llm::FunctionDefinition {
                name: "web_search".to_string(),
                description: "search".to_string(),
                parameters: json!({"type":"object"}),
            },
        };

        assert_eq!(
            prepare_anthropic_tools(&provider, vec![tool.clone()]).len(),
            1
        );
        provider.set_model_web_search_tool_mode(
            "grok-4.5",
            Some(WEB_SEARCH_TOOL_MODE_HIDE.to_string()),
        );
        assert!(prepare_anthropic_tools(&provider, vec![tool.clone()]).is_empty());
        provider.set_model_web_search_tool_mode(
            "grok-4.5",
            Some(WEB_SEARCH_TOOL_MODE_RENAME.to_string()),
        );
        let renamed = prepare_anthropic_tools(&provider, vec![tool]);
        assert_eq!(renamed[0].function.name, "sai_web_search");
    }

    #[test]
    fn anthropic_stream_accepts_thinking_signature_delta() {
        let mut state = AnthropicStreamState::default();
        let mut on_chunk = |_| Ok(());

        handle_anthropic_sse_data(
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"sig_123"}}"#,
            &mut state,
            &mut on_chunk,
        )
        .unwrap();

        assert_eq!(state.thinking_signature.as_deref(), Some("sig_123"));
        assert!(state.reasoning.is_empty());
    }

    #[test]
    fn anthropic_lowering_keeps_remote_image_urls() {
        let content = lower_anthropic_user_content(Some(ChatContent::Parts(vec![
            ChatContentPart::ImageUrl {
                image_url: ImageUrlContent {
                    url: "https://example.com/image.png".to_string(),
                },
            },
            ChatContentPart::Text {
                text: "describe".to_string(),
            },
        ])));
        let json = serde_json::to_value(content).unwrap();

        assert_eq!(json[0]["source"]["type"], "url");
        assert_eq!(json[0]["source"]["url"], "https://example.com/image.png");
        assert_eq!(json[1]["text"], "describe");
    }

    #[test]
    fn anthropic_thinking_errors_are_retryable_only_when_supported() {
        assert!(anthropic_thinking_unsupported(
            400,
            "thinking is not supported by this model"
        ));
        assert!(anthropic_thinking_unsupported(
            422,
            "unknown thinking parameter"
        ));
        assert!(!anthropic_thinking_unsupported(401, "invalid api key"));
        assert!(!anthropic_thinking_unsupported(
            400,
            "max_tokens is too low"
        ));
    }

    #[test]
    fn anthropic_stream_emits_reasoning_content_and_usage() {
        let mut state = AnthropicStreamState::default();
        let mut chunks = Vec::new();
        let mut on_chunk = |event| {
            if let ChatStreamEvent::Chunk(chunk) = event {
                chunks.push(chunk);
            }
            Ok(())
        };

        for data in [
            r#"{"type":"message_start","message":{"usage":{"input_tokens":3,"output_tokens":0}}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"想"}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"答"}}"#,
            r#"{"type":"message_delta","usage":{"input_tokens":3,"output_tokens":2},"delta":{"stop_reason":"end_turn"}}"#,
            r#"{"type":"message_stop"}"#,
        ] {
            let done = handle_anthropic_sse_data(data, &mut state, &mut on_chunk).unwrap();
            if data.contains("message_delta") {
                assert!(!done);
            }
            if data.contains("message_stop") {
                assert!(done);
            }
        }

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].kind, ChatStreamKind::Reasoning);
        assert_eq!(chunks[0].text, "想");
        assert_eq!(chunks[1].kind, ChatStreamKind::Content);
        assert_eq!(chunks[1].text, "答");
        let usage = state.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 3);
        assert_eq!(usage.completion_tokens, 2);
        assert_eq!(usage.total_tokens, 5);
    }

    #[test]
    fn anthropic_stream_merges_partial_usage_and_cache_tokens() {
        let mut state = AnthropicStreamState::default();
        let mut on_chunk = |_| Ok(());

        for data in [
            r#"{"type":"message_start","message":{"usage":{"input_tokens":100,"cache_creation_input_tokens":2000,"cache_read_input_tokens":4000,"output_tokens":0}}}"#,
            r#"{"type":"message_delta","usage":{"output_tokens":13},"delta":{"stop_reason":"end_turn"}}"#,
        ] {
            handle_anthropic_sse_data(data, &mut state, &mut on_chunk).unwrap();
        }

        let usage = state.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 6100);
        assert_eq!(usage.completion_tokens, 13);
        assert_eq!(usage.total_tokens, 6113);
    }

    #[test]
    fn anthropic_stream_accepts_later_explicit_input_usage() {
        let mut state = AnthropicStreamState::default();
        let mut on_chunk = |_| Ok(());

        for data in [
            r#"{"type":"message_start","message":{"usage":{"input_tokens":32,"output_tokens":0}}}"#,
            r#"{"type":"message_delta","usage":{"input_tokens":6548,"output_tokens":13},"delta":{"stop_reason":"end_turn"}}"#,
        ] {
            handle_anthropic_sse_data(data, &mut state, &mut on_chunk).unwrap();
        }

        let usage = state.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 6548);
        assert_eq!(usage.completion_tokens, 13);
        assert_eq!(usage.total_tokens, 6561);
    }

    #[test]
    fn anthropic_stream_collects_tool_calls() {
        let mut state = AnthropicStreamState::default();
        let mut on_chunk = |_| Ok(());

        for data in [
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_1","name":"calc","input":{}}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"x\":"}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"1}"}}"#,
        ] {
            handle_anthropic_sse_data(data, &mut state, &mut on_chunk).unwrap();
        }

        let calls = state.tool_calls.finish();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "toolu_1");
        assert_eq!(calls[0].function.name, "calc");
        assert_eq!(calls[0].function.arguments, r#"{"x":1}"#);
    }

    fn test_provider(id: &str, base_url: &str) -> ProviderConfig {
        ProviderConfig {
            id: id.to_string(),
            display_name: id.to_string(),
            base_url: base_url.to_string(),
            protocol: "auto".to_string(),
            api_key: None,
            models: Vec::new(),
            model_context_chars: std::collections::HashMap::new(),
            model_metadata: std::collections::HashMap::new(),
            default_model: String::new(),
            timeout_seconds: 60,
            temperature: 0.7,
            anthropic_max_tokens: 4096,
            thinking_level: "auto".to_string(),
            thinking_format: "auto".to_string(),
            extra_body: String::new(),
        }
    }

    /// 验证 UTF-8 多字节字符被网络分片切断时，行缓冲会跨 chunk 拼回完整字符。
    #[test]
    fn sse_buffer_preserves_utf8_split_across_byte_chunks() {
        let line = r#"data: {"choices":[{"delta":{"content":"等","tool_calls":null}}]}"#;
        // 在「等」的中间字节处切开（UTF-8 三字节汉字）
        let split = line.find("等").unwrap() + 1;
        let mut buffer = Utf8LineBuffer::default();

        assert!(buffer.push(&line.as_bytes()[..split]).unwrap().is_empty());
        let lines = buffer.push(&line.as_bytes()[split..]).unwrap();

        assert!(lines.is_empty());
        assert_eq!(buffer.finish().unwrap(), vec![line]);
    }

    /// 对照：旧的 lossy 按 chunk 解码会把被切断的汉字变成 U+FFFD。
    #[test]
    fn previous_lossy_chunk_decode_corrupts_split_utf8() {
        let text = "等";
        let mut decoded = String::new();

        decoded.push_str(&String::from_utf8_lossy(&text.as_bytes()[..1]));
        decoded.push_str(&String::from_utf8_lossy(&text.as_bytes()[1..]));

        assert_eq!(decoded, "\u{FFFD}\u{FFFD}\u{FFFD}");
    }

    /// Anthropic 风格 data 聚合也走字节缓冲。
    #[test]
    fn sse_data_buffer_preserves_utf8_across_chunks() {
        // 双换行闭合 SSE 事件
        let payload = "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"等\"}}\n\n";
        let split = payload.find("等").unwrap() + 1;
        let mut buffer = SseDataBuffer::default();
        assert!(buffer
            .push(&payload.as_bytes()[..split])
            .unwrap()
            .is_empty());
        let events = buffer.push(&payload.as_bytes()[split..]).unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].contains("等"));
    }
}

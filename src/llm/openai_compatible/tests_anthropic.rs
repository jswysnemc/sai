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
    fn openai_chat_thinking_validation_errors_are_retryable() {
        assert!(openai_chat_thinking_unsupported(
            400,
            r#"{"error":{"message":"Validation: `thinking` must be a boolean or an object with `type` set to `enabled`, `disabled`, or `adaptive`"}}"#
        ));
        assert!(openai_chat_thinking_unsupported(
            400,
            r#"{"message":"failed to unmarshal request body: json: cannot unmarshal string into Go struct field ReqeustFeature.thinking of type dto.Thinking"}"#
        ));
        assert!(!openai_chat_thinking_unsupported(500, "thinking unavailable"));
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

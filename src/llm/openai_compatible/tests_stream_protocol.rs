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
        StreamBuffers {
            content: &mut content,
            content_emitted: &mut content_emitted,
            reasoning: &mut reasoning,
            reasoning_emitted: &mut reasoning_emitted,
        },
        &mut usage,
        &mut content_started,
        &mut tool_calls,
        &mut on_chunk,
    )
    .unwrap();
    handle_responses_sse_line(
        r#"data: {"type":"response.output_text.delta","item_id":"msg_1","delta":"答案"}"#,
        StreamBuffers {
            content: &mut content,
            content_emitted: &mut content_emitted,
            reasoning: &mut reasoning,
            reasoning_emitted: &mut reasoning_emitted,
        },
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
            StreamBuffers {
                content: &mut content,
                content_emitted: &mut content_emitted,
                reasoning: &mut reasoning,
                reasoning_emitted: &mut reasoning_emitted,
            },
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
            StreamBuffers {
                content: &mut content,
                content_emitted: &mut content_emitted,
                reasoning: &mut reasoning,
                reasoning_emitted: &mut reasoning_emitted,
            },
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
    provider
        .set_model_web_search_tool_mode("grok-4.5", Some(WEB_SEARCH_TOOL_MODE_HIDE.to_string()));
    assert!(prepare_anthropic_tools(&provider, vec![tool.clone()]).is_empty());
    provider
        .set_model_web_search_tool_mode("grok-4.5", Some(WEB_SEARCH_TOOL_MODE_RENAME.to_string()));
    let renamed = prepare_anthropic_tools(&provider, vec![tool]);
    assert_eq!(renamed[0].function.name, "sai_web_search");
}

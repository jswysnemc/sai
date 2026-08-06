use super::*;
use crate::llm::{ChatContent, ChatContentPart, ImageUrlContent};

/// 构造测试用 provider。
///
/// 参数:
/// - `id`: provider id
/// - `base_url`: base_url
///
/// 返回:
/// - 最小可用 ProviderConfig
fn test_provider(id: &str, base_url: &str) -> ProviderConfig {
    ProviderConfig {
        id: id.to_string(),
        display_name: id.to_string(),
        base_url: base_url.to_string(),
        protocol: "auto".to_string(),
        api_key: None,
        api_keys: Vec::new(),
        api_key_selected: None,
        api_key_balance: false,
        models: Vec::new(),
        model_context_chars: std::collections::HashMap::new(),
        model_metadata: std::collections::HashMap::new(),
        default_model: String::new(),
        timeout_seconds: 60,
        temperature: 0.7,
        anthropic_max_tokens: 4096,
        thinking_level: "auto".to_string(),
        thinking_format: "auto".to_string(),
        preserve_thinking: false,
        extra_body: String::new(),
        extra_headers: std::collections::HashMap::new(),
        user_agent: String::new(),
        client_style: "auto".to_string(),
        claude_1m_context: true,
    }
}

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
fn stream_tool_calls_ignore_empty_id_from_bailian_chunks() {
    // 1. 模拟百炼/DashScope：首个 chunk 给有效 id，后续 arguments chunk 带空字符串 id
    let mut content = String::new();
    let mut content_emitted = 0usize;
    let mut reasoning = String::new();
    let mut reasoning_emitted = 0usize;
    let mut usage = None;
    let mut tool_calls = ToolCallAccumulator::default();
    let mut on_chunk = |_event| Ok(());
    let lines = [
        r#"data: {"choices":[{"delta":{"tool_calls":[{"id":"call_47f08af107c249a68db445ae","type":"function","index":0,"function":{"name":"check_os_info","arguments":""}}]}}]}"#,
        r#"data: {"choices":[{"delta":{"tool_calls":[{"id":"","type":"function","index":0,"function":{"arguments":"{}"}}]}}]}"#,
        r#"data: {"choices":[{"delta":{"tool_calls":[{"id":"call_22e15e90d261418e8209e033","type":"function","index":1,"function":{"name":"read_file","arguments":""}}]}}]}"#,
        r#"data: {"choices":[{"delta":{"tool_calls":[{"id":"","type":"function","index":1,"function":{"arguments":"{\"path\":\"/tmp\"}"}}]}}]}"#,
    ];
    for line in lines {
        handle_sse_line(
            line,
            &mut content,
            &mut content_emitted,
            &mut reasoning,
            &mut reasoning_emitted,
            &mut usage,
            &mut tool_calls,
            &mut on_chunk,
        )
        .unwrap();
    }

    // 2. 校验两个 call_id 都保留首个有效值，且 arguments 仍被拼接
    let calls = tool_calls.finish();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].id, "call_47f08af107c249a68db445ae");
    assert_eq!(calls[0].function.name, "check_os_info");
    assert_eq!(calls[0].function.arguments, "{}");
    assert_eq!(calls[1].id, "call_22e15e90d261418e8209e033");
    assert_eq!(calls[1].function.name, "read_file");
    assert_eq!(calls[1].function.arguments, r#"{"path":"/tmp"}"#);
}

#[test]
fn stream_tool_calls_generate_fallback_ids_when_upstream_omits_id() {
    let mut tool_calls = ToolCallAccumulator::default();
    tool_calls.push(ToolCallDelta {
        index: 0,
        id: None,
        kind: Some("function".to_string()),
        function: ToolCallFunctionDelta {
            name: Some("check_os_info".to_string()),
            arguments: Some("{}".to_string()),
        },
    });
    tool_calls.push(ToolCallDelta {
        index: 1,
        id: Some(String::new()),
        kind: Some("function".to_string()),
        function: ToolCallFunctionDelta {
            name: Some("read_file".to_string()),
            arguments: Some(r#"{"path":"/tmp"}"#.to_string()),
        },
    });

    let calls = tool_calls.finish();
    assert_eq!(calls[0].id, "call-fallback-0");
    assert_eq!(calls[1].id, "call-fallback-1");
}

#[test]
fn taotoken_glm_request_enables_thinking() {
    let mut provider = test_provider("taotoken", "https://taotoken.net/api/v1");
    provider.default_model = "glm_for_coding".to_string();

    assert!(
        taotoken_glm_chat_template_kwargs(&provider).is_some_and(|kwargs| kwargs.enable_thinking)
    );
}

#[test]
fn non_taotoken_glm_request_keeps_default_body() {
    let mut provider = test_provider("local", "http://localhost:11434/v1");
    provider.default_model = "glm-5".to_string();

    assert!(taotoken_glm_chat_template_kwargs(&provider).is_none());
}

#[test]
fn lower_responses_messages_wraps_codex_message_type() {
    let input = lower_responses_messages(vec![
        ChatMessage::system("sys"),
        ChatMessage::plain("user", "hi"),
    ]);
    assert_eq!(input[0]["type"], "message");
    assert_eq!(input[0]["role"], "system");
    assert_eq!(input[0]["content"][0]["type"], "input_text");
    assert_eq!(input[1]["type"], "message");
    assert_eq!(input[1]["role"], "user");
    assert_eq!(input[1]["content"][0]["type"], "input_text");
}

#[test]
fn request_cache_key_is_stable_when_history_grows() {
    let initial = vec![
        ChatMessage::system("stable instructions"),
        ChatMessage::plain("user", "first question"),
    ];
    let continued = vec![
        ChatMessage::system("stable instructions"),
        ChatMessage::plain("user", "first question"),
        ChatMessage::plain("assistant", "first answer"),
        ChatMessage::plain("user", "second question"),
    ];

    assert_eq!(
        stable_request_cache_key("model-a", &initial, &[]),
        stable_request_cache_key("model-a", &continued, &[])
    );
}

#[test]
fn request_cache_key_changes_with_the_stable_instructions() {
    let first = vec![ChatMessage::system("instructions one")];
    let second = vec![ChatMessage::system("instructions two")];

    assert_ne!(
        stable_request_cache_key("model-a", &first, &[]),
        stable_request_cache_key("model-a", &second, &[])
    );
}

#[test]
fn request_cache_key_changes_with_the_tool_definitions() {
    let messages = vec![ChatMessage::system("stable instructions")];
    let first = ToolDefinition {
        kind: "function",
        function: crate::llm::FunctionDefinition {
            name: "first_tool".to_string(),
            description: "first".to_string(),
            parameters: json!({"type": "object"}),
        },
    };
    let second = ToolDefinition {
        kind: "function",
        function: crate::llm::FunctionDefinition {
            name: "second_tool".to_string(),
            description: "second".to_string(),
            parameters: json!({"type": "object"}),
        },
    };

    assert_ne!(
        stable_request_cache_key("model-a", &messages, &[first]),
        stable_request_cache_key("model-a", &messages, &[second])
    );
}

#[test]
fn prefers_codex_shape_for_sol_and_codex_models() {
    assert!(prefers_codex_responses_shape(
        "gpt-5.6-sol",
        "https://example.com/v1",
        "auto"
    ));
    assert!(prefers_codex_responses_shape(
        "gpt-5-codex",
        "https://example.com/v1",
        "auto"
    ));
    assert!(prefers_codex_responses_shape(
        "gpt-4o",
        "https://a-ocnfniawgw.cn-shanghai.fcapp.run/v1",
        "auto"
    ));
    assert!(!prefers_codex_responses_shape(
        "gpt-4o",
        "https://api.openai.com/v1",
        "auto"
    ));
    assert!(prefers_codex_responses_shape(
        "gpt-4o",
        "https://api.openai.com/v1",
        "codex"
    ));
    assert!(!prefers_codex_responses_shape(
        "gpt-5.6-sol",
        "https://example.com/v1",
        "default"
    ));
    // Claude 模型即使挂在 fcapp 上也不走 Codex Responses
    assert!(!prefers_codex_responses_shape(
        "claude-sonnet-4-5-20250929",
        "https://a-ocnfniawgw.cn-shanghai.fcapp.run/v1",
        "auto"
    ));
    assert!(!prefers_codex_responses_shape(
        "gpt-4o",
        "https://a-ocnfniawgw.cn-shanghai.fcapp.run/v1",
        "claude"
    ));
}

#[test]
fn prefers_claude_code_shape_for_explicit_and_proxy_auto() {
    assert!(prefers_claude_code_shape(
        "claude-sonnet-4-5",
        "https://api.anthropic.com/v1",
        "claude",
        true
    ));
    assert!(!prefers_claude_code_shape(
        "claude-sonnet-4-5",
        "https://api.anthropic.com/v1",
        "auto",
        true
    ));
    assert!(prefers_claude_code_shape(
        "claude-sonnet-4-5-20250929",
        "https://a-ocnfniawgw.cn-shanghai.fcapp.run/v1",
        "auto",
        false
    ));
    assert!(!prefers_claude_code_shape(
        "gpt-5.6-sol",
        "https://a-ocnfniawgw.cn-shanghai.fcapp.run/v1",
        "auto",
        false
    ));
    assert!(!prefers_claude_code_shape(
        "claude-sonnet-4-5",
        "https://proxy.example.com/v1",
        "default",
        false
    ));
}

#[test]
fn claude_code_messages_url_appends_beta_query() {
    assert_eq!(
        claude_code_messages_url("https://example.com/v1/messages"),
        "https://example.com/v1/messages?beta=true"
    );
    assert_eq!(
        claude_code_messages_url("https://example.com/v1/messages?foo=1"),
        "https://example.com/v1/messages?foo=1&beta=true"
    );
    assert_eq!(
        claude_code_messages_url("https://example.com/v1/messages?beta=true"),
        "https://example.com/v1/messages?beta=true"
    );
}

#[test]
fn apply_claude_code_body_shape_rewrites_system_and_thinking() {
    let mut body = json!({
        "model": "claude-sonnet-4-5",
        "system": "You are Sai.",
        "max_tokens": 4096,
        "thinking": {"type":"enabled","budget_tokens":8000}
    });
    apply_claude_code_body_shape(&mut body, "sess-1", "high");
    let system = body["system"].as_array().expect("system array");
    assert!(system.len() >= 3);
    assert!(system[0]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("x-anthropic-billing-header"));
    assert!(system[1]["text"]
        .as_str()
        .unwrap_or_default()
        .contains("Claude Code"));
    assert_eq!(system[2]["text"], "You are Sai.");
    assert_eq!(body["thinking"], json!({"type":"adaptive"}));
    assert_eq!(body["output_config"]["effort"], "high");
    assert!(body["metadata"]["user_id"]
        .as_str()
        .unwrap()
        .contains("sess-1"));

    let headers = claude_code_request_headers("sk-test", "sess-1", CLAUDE_CLI_USER_AGENT, true);
    let names: Vec<_> = headers
        .iter()
        .map(|(k, _)| k.to_ascii_lowercase())
        .collect();
    assert!(names.iter().any(|n| n == "anthropic-beta"));
    assert!(names.iter().any(|n| n == "x-app"));
    assert!(names.iter().any(|n| n == "x-claude-code-session-id"));
    let beta = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("anthropic-beta"))
        .map(|(_, v)| v.as_str())
        .unwrap_or_default();
    assert!(beta.contains("context-1m-2025-08-07"));
    assert!(beta.contains("claude-code-20250219"));

    let beta_off = claude_code_beta_header(false);
    assert!(!beta_off.contains("context-1m-2025-08-07"));
    assert!(beta_off.contains("claude-code-20250219"));
    let beta_on = claude_code_beta_header(true);
    assert!(beta_on.contains("context-1m-2025-08-07"));
}

#[test]
fn openai_gpt5_uses_responses_api() {
    let mut provider = test_provider("openai", "https://api.openai.com/v1");
    provider.default_model = "gpt-5.5".to_string();
    let client = OpenAiCompatibleClient {
        client: reqwest::Client::new(),
        provider,
        api_key: "test".to_string(),
        key_pool: Vec::new(),
        key_cursor: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
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
        key_pool: Vec::new(),
        key_cursor: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
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
    provider
        .set_model_web_search_tool_mode("grok-4.5", Some(WEB_SEARCH_TOOL_MODE_HIDE.to_string()));
    assert!(prepare_anthropic_tools(&provider, vec![tool.clone()]).is_empty());
    provider
        .set_model_web_search_tool_mode("grok-4.5", Some(WEB_SEARCH_TOOL_MODE_RENAME.to_string()));
    let renamed = prepare_anthropic_tools(&provider, vec![tool]);
    assert_eq!(renamed[0].function.name, "sai_web_search");
}

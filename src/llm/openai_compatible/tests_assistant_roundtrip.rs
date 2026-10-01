/// 【Anthropic】【工具续轮】从真实 SSE 解析到下一请求，思考、签名与删节块逐字保留。
/// @returns 无；不接收参数
#[test]
fn thinking_blocks_survive_stream_result_and_next_request() {
    let mut state = AnthropicStreamState::default();
    let blocks = vec![
        json!({"type":"thinking","thinking":"先读文件","signature":"signed-payload"}),
        json!({"type":"redacted_thinking","data":"opaque-payload"}),
        json!({"type":"text","text":"准备读取"}),
        json!({"type":"tool_use","id":"tool-1","name":"read_file","input":{"path":"a"}}),
    ];
    for (index, original) in blocks.iter().enumerate() {
        let mut block = original.clone();
        if block["type"] == "tool_use" {
            block["input"] = json!({});
        }
        handle_anthropic_sse_data(
            &json!({"type":"content_block_start","index":index,"content_block":block}).to_string(),
            &mut state,
            &mut |_| Ok(()),
        )
        .unwrap();
        if original["type"] == "tool_use" {
            handle_anthropic_sse_data(&json!({"type":"content_block_delta","index":index,"delta":{"type":"input_json_delta","partial_json":original["input"].to_string()}}).to_string(), &mut state, &mut |_| Ok(())).unwrap();
        }
        handle_anthropic_sse_data(
            &json!({"type":"content_block_stop","index":index}).to_string(),
            &mut state,
            &mut |_| Ok(()),
        )
        .unwrap();
    }
    handle_anthropic_sse_data("{\"type\":\"message_stop\"}", &mut state, &mut |_| Ok(())).unwrap();
    let result = finalize_anthropic_result(state).unwrap();
    let message = result.assistant_message();
    let request =
        lower_anthropic_messages(vec![message.clone(), ChatMessage::tool("tool-1", "file")]);
    assert_eq!(
        serde_json::to_value(&request).unwrap()[0]["content"],
        json!(blocks)
    );
    assert!(!serde_json::to_string(&message)
        .unwrap()
        .contains("signed-payload"));
}

/// 【Anthropic】【缓存断点】Claude 模拟保留显式 system 块策略；无参数或返回值。
#[test]
fn claude_style_preserves_explicit_system_cache_breakpoints() {
    let block =
        json!({"type":"text","text":"stable","cache_control":{"type":"ephemeral","ttl":"1h"}});
    let mut body = json!({"system":[block.clone()]});
    apply_claude_code_body_shape(&mut body, "session", "none");
    apply_default_cache_control(&mut body);
    assert_eq!(body["system"][2], block);
    assert!(body.get("cache_control").is_none());
}

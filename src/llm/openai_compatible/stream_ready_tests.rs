use super::*;

/// 【模型流】【Anthropic 就绪】只有块结束后发出完整调用，后续文本仍可继续传输。
/// 参数：无；返回断言结果。
#[test]
fn anthropic_tool_ready_precedes_message_stop() {
    let mut state = AnthropicStreamState::default();
    let mut ready = Vec::new();
    let mut sink = |event| {
        if let ChatStreamEvent::ToolCallReady { index, call } = event {
            ready.push((index, call));
        }
        Ok(())
    };
    for data in [
        r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"r1","name":"read_file","input":{}}}"#,
        r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"a.txt\"}"}}"#,
    ] {
        assert!(!handle_anthropic_sse_data(data, &mut state, &mut sink).unwrap());
    }
    assert!(!handle_anthropic_sse_data(
        r#"{"type":"content_block_stop","index":1}"#,
        &mut state,
        &mut sink
    )
    .unwrap());
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].0, 0);
    assert_eq!(ready[0].1.id, "r1");
    assert_eq!(ready[0].1.function.arguments, r#"{"path":"a.txt"}"#);
}

/// 【模型流】【Responses 就绪】output_item.done 时可读取，尚未 response.completed。
/// 参数：无；返回断言结果。
#[test]
fn responses_tool_ready_precedes_response_completed() {
    let (mut content, mut reasoning) = (String::new(), String::new());
    let (mut content_emitted, mut reasoning_emitted) = (0, 0);
    let (mut usage, mut started) = (None, false);
    let mut calls = ResponsesToolAccumulator::default();
    let mut ready = Vec::new();
    let completed = handle_responses_sse_line(
        r#"data: {"type":"response.output_item.done","item":{"type":"function_call","id":"item-1","call_id":"call-1","name":"read_file","arguments":"{\"path\":\"a.txt\"}"}}"#,
        StreamBuffers { content: &mut content, content_emitted: &mut content_emitted, reasoning: &mut reasoning, reasoning_emitted: &mut reasoning_emitted },
        &mut usage, &mut started, &mut calls, &mut |event| {
            if let ChatStreamEvent::ToolCallReady { index, call } = event { ready.push((index, call)); }
            Ok(())
        }).unwrap();
    assert!(!completed);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].1.id, "call-1");
}

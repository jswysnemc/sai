fn clean_response_content(content: String) -> (String, Option<String>) {
    split_tagged_reasoning(clean_plain_text(content))
}

fn split_tagged_reasoning(content: String) -> (String, Option<String>) {
    match split_tag_pair(content, "think").or_else(|content| split_tag_pair(content, "thinking")) {
        Ok(result) => result,
        Err(content) => (content, None),
    }
}

fn split_tag_pair(
    content: String,
    tag: &str,
) -> std::result::Result<(String, Option<String>), String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let Some(start) = content.find(&open) else {
        return Err(content);
    };
    let reasoning_start = start + open.len();
    let Some(relative_end) = content[reasoning_start..].find(&close) else {
        return Ok((content, None));
    };
    let end = reasoning_start + relative_end;
    let reasoning = content[reasoning_start..end].trim().to_string();
    let mut visible = String::new();
    visible.push_str(content[..start].trim_end());
    visible.push_str(content[end + close.len()..].trim_start());
    Ok((
        visible.trim().to_string(),
        (!reasoning.is_empty()).then_some(reasoning),
    ))
}

fn handle_sse_line<F>(
    line: &str,
    content: &mut String,
    content_emitted: &mut usize,
    reasoning: &mut String,
    reasoning_emitted: &mut usize,
    usage: &mut Option<Usage>,
    tool_calls: &mut ToolCallAccumulator,
    on_event: &mut F,
) -> Result<Option<bool>>
where
    F: FnMut(ChatStreamEvent) -> Result<()>,
{
    let Some(data) = line.strip_prefix("data:").map(str::trim) else {
        return Ok(None);
    };
    if data == "[DONE]" {
        flush_buffer(
            content,
            content_emitted,
            ChatStreamKind::Content,
            on_event,
            true,
        )?;
        flush_buffer(
            reasoning,
            reasoning_emitted,
            ChatStreamKind::Reasoning,
            on_event,
            true,
        )?;
        return Ok(Some(true));
    }
    let response: ChatStreamResponse = serde_json::from_str(data).with_context(|| {
        format!(
            "{}: {}",
            t(
                "invalid chat completions stream response",
                "无效的聊天流式响应",
            ),
            clean_plain_text(data.to_string())
        )
    })?;
    if let Some(next_usage) = response.usage {
        *usage = Some(next_usage);
    }
    for choice in response.choices {
        let delta = choice.delta;
        if let Some(text) = delta_reasoning_text(&delta) {
            push_buffered_chunk(
                reasoning,
                reasoning_emitted,
                ChatStreamKind::Reasoning,
                text,
                on_event,
            )?;
        }
        if let Some(text) = delta.content {
            push_buffered_chunk(
                content,
                content_emitted,
                ChatStreamKind::Content,
                text,
                on_event,
            )?;
        }
        for tool_call in delta.tool_calls {
            if let Some(progress) = tool_calls.push(tool_call) {
                on_event(ChatStreamEvent::ToolCallProgress(progress))?;
            }
        }
    }
    Ok(Some(false))
}

fn handle_responses_sse_line<F>(
    line: &str,
    content: &mut String,
    content_emitted: &mut usize,
    reasoning: &mut String,
    reasoning_emitted: &mut usize,
    usage: &mut Option<Usage>,
    content_started: &mut bool,
    tool_calls: &mut ResponsesToolAccumulator,
    on_event: &mut F,
) -> Result<bool>
where
    F: FnMut(ChatStreamEvent) -> Result<()>,
{
    let Some(data) = line.strip_prefix("data:").map(str::trim) else {
        return Ok(false);
    };
    if data == "[DONE]" {
        flush_buffer(
            content,
            content_emitted,
            ChatStreamKind::Content,
            on_event,
            true,
        )?;
        flush_buffer(
            reasoning,
            reasoning_emitted,
            ChatStreamKind::Reasoning,
            on_event,
            true,
        )?;
        return Ok(true);
    }
    let event: ResponsesStreamEvent = serde_json::from_str(data).with_context(|| {
        format!(
            "{}: {}",
            t(
                "invalid responses stream event",
                "无效的 Responses 流式事件"
            ),
            clean_plain_text(data.to_string())
        )
    })?;
    match event.kind.as_str() {
        "response.output_text.delta" => {
            if let Some(text) = event.delta {
                *content_started = true;
                push_buffered_chunk(
                    content,
                    content_emitted,
                    ChatStreamKind::Content,
                    text,
                    on_event,
                )?;
            }
        }
        "response.reasoning_text.delta"
        | "response.reasoning_summary.delta"
        | "response.reasoning_summary_text.delta" => {
            if let Some(text) = event.delta {
                push_buffered_chunk(
                    reasoning,
                    reasoning_emitted,
                    ChatStreamKind::Reasoning,
                    text,
                    on_event,
                )?;
            }
        }
        "response.reasoning_text.done"
        | "response.reasoning_summary.done"
        | "response.reasoning_summary_text.done" => {
            if !*content_started && !reasoning.trim().is_empty() {
                flush_buffer(
                    reasoning,
                    reasoning_emitted,
                    ChatStreamKind::Reasoning,
                    on_event,
                    true,
                )?;
                *content_started = true;
                on_event(ChatStreamEvent::Chunk(ChatStreamChunk {
                    kind: ChatStreamKind::Content,
                    text: String::new(),
                }))?;
            }
        }
        "response.output_item.added" => {
            if let Some(item) = event.item {
                if let Some(progress) = tool_calls.start(item) {
                    on_event(ChatStreamEvent::ToolCallProgress(progress))?;
                }
            }
        }
        "response.function_call_arguments.delta" => {
            if let Some(delta) = event.delta {
                if let Some(progress) = tool_calls.append_arguments(event.item_id, delta) {
                    on_event(ChatStreamEvent::ToolCallProgress(progress))?;
                }
            }
        }
        "response.output_item.done" => {
            if let Some(item) = event.item {
                if let Some(progress) = tool_calls.finish_item(item) {
                    on_event(ChatStreamEvent::ToolCallProgress(progress))?;
                }
            }
        }
        "response.completed" | "response.incomplete" => {
            if let Some(next_usage) = event.response.and_then(|response| response.usage) {
                *usage = Some(Usage {
                    prompt_tokens: next_usage.input_tokens,
                    completion_tokens: next_usage.output_tokens,
                    total_tokens: next_usage.total_tokens,
                });
            }
            flush_buffer(
                content,
                content_emitted,
                ChatStreamKind::Content,
                on_event,
                true,
            )?;
            flush_buffer(
                reasoning,
                reasoning_emitted,
                ChatStreamKind::Reasoning,
                on_event,
                true,
            )?;
            return Ok(true);
        }
        "error" | "response.failed" => {
            bail!(
                "OpenAI Responses stream failed: {}",
                clean_plain_text(data.to_string())
            );
        }
        _ => {}
    }
    Ok(false)
}

fn handle_anthropic_sse_data<F>(
    data: &str,
    state: &mut AnthropicStreamState,
    on_event: &mut F,
) -> Result<bool>
where
    F: FnMut(ChatStreamEvent) -> Result<()>,
{
    if data == "[DONE]" {
        flush_anthropic_state(state, on_event)?;
        return Ok(true);
    }
    let event: AnthropicStreamEvent = serde_json::from_str(data).with_context(|| {
        format!(
            "{}: {}",
            t(
                "invalid anthropic messages stream event",
                "无效的 Anthropic Messages 流式事件"
            ),
            clean_plain_text(data.to_string())
        )
    })?;
    match event.kind.as_str() {
        "message_start" => {
            if let Some(usage) = event.message.and_then(|message| message.usage) {
                merge_anthropic_usage(state, usage);
            }
        }
        "content_block_start" => {
            if let Some(block) = event.content_block {
                match block.kind.as_str() {
                    "tool_use" | "server_tool_use" => {
                        if let Some(index) = event.index {
                            if let Some(progress) = state.tool_calls.start(index, block) {
                                on_event(ChatStreamEvent::ToolCallProgress(progress))?;
                            }
                        }
                    }
                    "text" => {
                        if let Some(text) = block.text {
                            push_buffered_chunk(
                                &mut state.content,
                                &mut state.content_emitted,
                                ChatStreamKind::Content,
                                text,
                                on_event,
                            )?;
                        }
                    }
                    "thinking" => {
                        if let Some(text) = block.thinking {
                            push_buffered_chunk(
                                &mut state.reasoning,
                                &mut state.reasoning_emitted,
                                ChatStreamKind::Reasoning,
                                text,
                                on_event,
                            )?;
                        }
                    }
                    _ => {}
                }
            }
        }
        "content_block_delta" => {
            if let Some(delta) = event.delta {
                match delta.kind.as_deref() {
                    Some("text_delta") => {
                        if let Some(text) = delta.text {
                            push_buffered_chunk(
                                &mut state.content,
                                &mut state.content_emitted,
                                ChatStreamKind::Content,
                                text,
                                on_event,
                            )?;
                        }
                    }
                    Some("thinking_delta") => {
                        if let Some(text) = delta.thinking {
                            push_buffered_chunk(
                                &mut state.reasoning,
                                &mut state.reasoning_emitted,
                                ChatStreamKind::Reasoning,
                                text,
                                on_event,
                            )?;
                        }
                    }
                    Some("input_json_delta") => {
                        if let (Some(index), Some(text)) = (event.index, delta.partial_json) {
                            if let Some(progress) = state.tool_calls.append_arguments(index, text) {
                                on_event(ChatStreamEvent::ToolCallProgress(progress))?;
                            }
                        }
                    }
                    Some("signature_delta") => {
                        state.thinking_signature = delta.signature;
                    }
                    _ => {}
                }
            }
        }
        "message_delta" => {
            if let Some(usage) = event.usage {
                merge_anthropic_usage(state, usage);
            }
            flush_anthropic_state(state, on_event)?;
        }
        "message_stop" => {
            flush_anthropic_state(state, on_event)?;
            return Ok(true);
        }
        "error" => {
            let message = event
                .error
                .map(|error| match (error.kind, error.message) {
                    (Some(kind), Some(message)) => format!("{kind}: {message}"),
                    (Some(kind), None) => kind,
                    (None, Some(message)) => message,
                    (None, None) => "Anthropic Messages stream error".to_string(),
                })
                .unwrap_or_else(|| "Anthropic Messages stream error".to_string());
            bail!("{message}");
        }
        _ => {}
    }
    Ok(false)
}

fn flush_anthropic_state<F>(state: &mut AnthropicStreamState, on_event: &mut F) -> Result<()>
where
    F: FnMut(ChatStreamEvent) -> Result<()>,
{
    flush_buffer(
        &state.content,
        &mut state.content_emitted,
        ChatStreamKind::Content,
        on_event,
        true,
    )?;
    flush_buffer(
        &state.reasoning,
        &mut state.reasoning_emitted,
        ChatStreamKind::Reasoning,
        on_event,
        true,
    )
}

/// 合并 Anthropic 分阶段返回的令牌统计。
///
/// 参数:
/// - `state`: 当前 Anthropic 流状态
/// - `usage`: 本次事件实际携带的统计字段
///
/// 返回:
/// - 无；合并结果写入流状态
fn merge_anthropic_usage(state: &mut AnthropicStreamState, usage: AnthropicUsage) {
    if let Some(value) = usage.input_tokens {
        state.input_tokens = Some(value);
    }
    if let Some(value) = usage.cache_creation_input_tokens {
        state.cache_creation_input_tokens = Some(value);
    }
    if let Some(value) = usage.cache_read_input_tokens {
        state.cache_read_input_tokens = Some(value);
    }
    if let Some(value) = usage.output_tokens {
        state.output_tokens = Some(value);
    }
    let prompt_tokens = state.input_tokens.unwrap_or_default()
        + state.cache_creation_input_tokens.unwrap_or_default()
        + state.cache_read_input_tokens.unwrap_or_default();
    let completion_tokens = state.output_tokens.unwrap_or_default();
    state.usage = Some(Usage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
    });
}

fn delta_reasoning_text(delta: &ChatChoiceMessage) -> Option<String> {
    delta
        .reasoning_content
        .clone()
        .or_else(|| delta.reasoning.clone())
        .or_else(|| delta.thinking.clone())
        .or_else(|| delta.thinking_content.clone())
        .or_else(|| delta.reasoning_text.clone())
        .or_else(|| reasoning_details_text(delta.reasoning_details.as_ref()))
}

fn reasoning_details_text(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    if let Some(array) = value.as_array() {
        let text = array
            .iter()
            .filter_map(|item| {
                item.get("text")
                    .or_else(|| item.get("content"))
                    .and_then(serde_json::Value::as_str)
            })
            .collect::<Vec<_>>()
            .join("");
        return (!text.is_empty()).then_some(text);
    }
    value
        .get("text")
        .or_else(|| value.get("content"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn push_buffered_chunk<F>(
    target: &mut String,
    emitted: &mut usize,
    kind: ChatStreamKind,
    text: String,
    on_event: &mut F,
) -> Result<()>
where
    F: FnMut(ChatStreamEvent) -> Result<()>,
{
    if text.is_empty() {
        return Ok(());
    }
    target.push_str(&text);
    flush_buffer(target, emitted, kind, on_event, false)
}

fn flush_buffer<F>(
    target: &str,
    emitted: &mut usize,
    kind: ChatStreamKind,
    on_event: &mut F,
    final_flush: bool,
) -> Result<()>
where
    F: FnMut(ChatStreamEvent) -> Result<()>,
{
    while *emitted < target.len() {
        let remaining = &target[*emitted..];
        if starts_hidden_prefix(remaining) {
            if let Some(end) = hidden_end_after(target, *emitted) {
                *emitted = end;
                continue;
            }
            if final_flush {
                *emitted = target.len();
            }
            return Ok(());
        }
        let hidden_start = hidden_start_after(target, *emitted);
        let mut safe_end = hidden_start.unwrap_or(target.len());
        if hidden_start.is_none() && !final_flush {
            safe_end =
                safe_end.saturating_sub(partial_hidden_suffix_len(&target[*emitted..safe_end]));
        }
        if safe_end <= *emitted {
            return Ok(());
        }
        let text = target[*emitted..safe_end].to_string();
        *emitted = safe_end;
        if !text.is_empty() {
            on_event(ChatStreamEvent::Chunk(ChatStreamChunk { kind, text }))?;
        }
    }
    Ok(())
}

fn finalize_stream_result(
    content: String,
    reasoning: String,
    usage: Option<Usage>,
    tool_calls: Vec<ToolCall>,
) -> Result<ChatResult> {
    let content = clean_plain_text(content);
    let (content, mut dsml_tool_calls) = extract_dsml_tool_calls(content);
    let reasoning = clean_plain_text(reasoning);
    let (reasoning, reasoning_dsml_tool_calls) = extract_dsml_tool_calls(reasoning);
    dsml_tool_calls.extend(reasoning_dsml_tool_calls);
    let (content, tag_reasoning) = clean_response_content(content);
    let reasoning = if reasoning.trim().is_empty() {
        tag_reasoning
    } else {
        Some(reasoning)
    };
    let tool_calls = if dsml_tool_calls.is_empty() {
        tool_calls
    } else {
        dsml_tool_calls
    };
    if content.trim().is_empty() && tool_calls.is_empty() {
        bail!(
            "{}",
            t(
                "chat completions stream response was empty",
                "聊天流式响应为空",
            )
        );
    }
    Ok(ChatResult {
        content,
        reasoning: reasoning.filter(|text| !text.trim().is_empty()),
        usage,
        tool_calls,
        duration_ms: 0,
    })
}

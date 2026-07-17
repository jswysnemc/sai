async fn chat_with_tools(
    client: &OpenAiCompatibleClient,
    mut messages: Vec<ChatMessage>,
    tools: ToolRegistry,
    max_steps: usize,
    timeout_seconds: u64,
    progress: &ResearchProgress,
    state: Arc<Mutex<ResearchState>>,
) -> Result<ChatResult> {
    let definitions = tools.definitions_except(&["deep_research"]);
    let mut steps = 0usize;
    loop {
        let result = client
            .chat_stream(
                messages.clone(),
                definitions.clone(),
                |chunk: ChatStreamChunk| {
                    if chunk.kind == ChatStreamKind::Reasoning {
                        progress.reasoning(&chunk.text);
                    }
                    Ok(())
                },
            )
            .await?;
        if result.tool_calls.is_empty() {
            return Ok(result);
        }
        messages.push(ChatMessage::assistant(
            result.content.clone(),
            Some(result.tool_calls.clone()),
        ));
        for call in result.tool_calls {
            if max_steps > 0 && steps >= max_steps {
                progress.tool(format!(
                    "→{} skipped: tool budget reached",
                    call.function.name
                ));
                messages.push(ChatMessage::tool(
                    call.id,
                    "tool budget reached for this deep research round",
                ));
                continue;
            }
            steps += 1;
            {
                let mut state = state.lock().expect("deep research state lock");
                state.stats.tool_calls += 1;
            }
            progress.subtool_text(if is_zh() {
                format!(
                    "工具 #{steps}：{} 运行中",
                    readable_tool_name(&call.function.name)
                )
            } else {
                format!("tool #{steps}: {} running", call.function.name)
            });
            progress.subtool(format!(
                "__subtool_call__{}",
                json!({
                    "name": call.function.name,
                    "args": call.function.arguments,
                })
            ));
            let (output, ok) = match tokio::time::timeout(
                Duration::from_secs(timeout_seconds.max(5)),
                tools.call(&call.function.name, &call.function.arguments),
            )
            .await
            {
                Ok(Ok(output)) => (output, true),
                Ok(Err(err)) => (format!("tool error: {err}"), false),
                Err(_) => (
                    format!(
                        "tool error: {} timed out after {timeout_seconds}s",
                        call.function.name
                    ),
                    false,
                ),
            };
            {
                let mut state = state.lock().expect("deep research state lock");
                if ok {
                    state.stats.tool_ok += 1;
                } else {
                    state.stats.tool_errors += 1;
                }
            }
            progress.subtool_text(if is_zh() {
                format!(
                    "工具 #{steps}：{} ok",
                    readable_tool_name(&call.function.name)
                )
            } else {
                format!("tool #{steps}: {} ok", call.function.name)
            });
            progress.subtool(format!(
                "__subtool_result__{}",
                json!({
                    "name": call.function.name,
                    "ok": ok,
                    "output": output,
                })
            ));
            messages.push(ChatMessage::tool(
                call.id,
                tool_output_for_context(&call.function.name, &output),
            ));
        }
    }
}

fn thinker_prompt(
    topic: &str,
    iteration: usize,
    draft: &str,
    review: &Value,
    state: &Arc<Mutex<ResearchState>>,
) -> Result<String> {
    Ok(format!(
        "请完成第 {iteration} 轮深度研究。\n\n用户命题：\n{topic}\n\n上一轮草稿：\n{}\n\n上一轮审视意见：\n{}\n\n当前参考资料注册表：\n{}\n\n要求：结论先行，必要时调用工具查证；需要引用时先注册参考资料，并在正文中使用 [R1]/[K1]/[W1] 标注。不要输出参考资料章节。",
        if draft.trim().is_empty() { "（无）" } else { draft },
        serde_json::to_string_pretty(review)?,
        reference_registry_json(state)?,
    ))
}

fn reviewer_prompt(
    topic: &str,
    iteration: usize,
    draft: &str,
    state: &Arc<Mutex<ResearchState>>,
) -> Result<String> {
    Ok(format!(
        "请审查第 {iteration} 轮草案。\n\n用户命题：\n{topic}\n\n草案：\n{draft}\n\n参考资料注册表：\n{}\n\n若可以发送，accepted=true；否则列出具体 revision_instructions。",
        reference_registry_json(state)?,
    ))
}

fn reference_registry_json(state: &Arc<Mutex<ResearchState>>) -> Result<String> {
    let state = state.lock().expect("deep research state lock");
    let refs = state.references.iter().map(|item| json!({"ref": item.marker, "type": item.kind, "title": item.title, "url": item.url, "path": item.path, "snippet": item.snippet})).collect::<Vec<_>>();
    Ok(serde_json::to_string_pretty(&refs)?)
}

fn parse_review(content: &str) -> Value {
    parse_json_object(content).unwrap_or_else(|| {
        json!({"accepted": true, "challenge": "reviewer returned non-JSON feedback; accept current draft to avoid repeated research", "revision_instructions": [], "review_text": content.trim()})
    })
}

fn parse_json_object(content: &str) -> Option<Value> {
    let trimmed = content.trim();
    serde_json::from_str(trimmed)
        .ok()
        .or_else(|| extract_json_object(trimmed).and_then(|json| serde_json::from_str(json).ok()))
}

fn extract_json_object(content: &str) -> Option<&str> {
    let start = content.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in content[start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_string {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = start + offset + ch.len_utf8();
                    return Some(&content[start..end]);
                }
            }
            _ => {}
        }
    }
    None
}


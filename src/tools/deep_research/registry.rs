pub fn register(
    registry: &mut ToolRegistry,
    config: AppConfig,
    paths: SaiPaths,
    tools: ToolRegistry,
) {
    let context = DeepResearchContext {
        config,
        paths,
        tools,
    };
    registry.register(ToolSpec::new_with_progress(
        "deep_research",
        "Run a dual-role deep research task and write the final Markdown report to the configured output directory.",
        json!({
            "type": "object",
            "properties": {
                "topic": { "type": "string", "description": "Research question or topic." },
                "thinking_depth": { "type": "string", "enum": ["minimal", "low", "medium", "high", "xhigh"], "description": "Optional depth override." }
            },
            "required": ["topic"],
            "additionalProperties": false
        }),
        move |args, progress| {
            let context = context.clone();
            async move { run_deep_research(args, context, progress).await }
        },
    ));
}

async fn run_deep_research(
    args: Value,
    context: DeepResearchContext,
    progress: ToolProgress,
) -> Result<String> {
    if !context.config.plugins.deep_research.enabled {
        bail!("deep_research plugin is disabled")
    }
    let topic = args
        .get("topic")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if topic.is_empty() {
        bail!("topic is required")
    }
    let plugin = &context.config.plugins.deep_research;
    let progress = ResearchProgress::new(&context.config, progress);
    let depth = args
        .get("thinking_depth")
        .and_then(Value::as_str)
        .unwrap_or(&plugin.thinking_depth)
        .to_string();
    let max_revisions = if plugin.max_review_revisions == 0 {
        depth_default_revisions(&depth)
    } else {
        plugin.max_review_revisions
    };
    let max_tool_steps = if plugin.max_tool_steps_per_round == 0 {
        depth_default_tool_steps(&depth)
    } else {
        plugin.max_tool_steps_per_round
    };
    let client = OpenAiCompatibleClient::from_config(&context.config, &context.paths)?;
    let state = Arc::new(Mutex::new(ResearchState::default()));
    let mut draft = String::new();
    let mut review =
        json!({"accepted": false, "challenge": "首轮暂无审视意见", "revision_instructions": []});
    let mut iterations = 0usize;
    let mut stop_reason = "max_review_revisions_reached".to_string();
    progress.phase(format!(
        "{}=\"{}\"",
        t("topic", "主题"),
        topic_title(&state, &topic)
    ));

    loop {
        let iteration = iterations + 1;
        if max_revisions != usize::MAX && iteration > max_revisions.saturating_add(1) {
            break;
        }
        iterations = iteration;
        progress.phase(format!("round {iteration}: thinker drafting"));
        let tools = research_tool_registry(&context, Arc::clone(&state));
        let prompt = thinker_prompt(&topic, iteration, &draft, &review, &state)?;
        let thinker_system = THINKER_SYSTEM_PROMPT;
        let thinker = chat_with_tools(
            &client,
            vec![
                ChatMessage::system(thinker_system),
                ChatMessage::plain("user", prompt.clone()),
            ],
            tools,
            max_tool_steps,
            plugin.tool_call_timeout_seconds,
            &progress,
            Arc::clone(&state),
        )
        .await?;
        state
            .lock()
            .expect("deep research state lock")
            .stats
            .add_usage_or_estimate(
                thinker.usage.as_ref(),
                &[thinker_system, &prompt, &thinker.content],
            );
        if !thinker.content.trim().is_empty() {
            draft = thinker.content.trim().to_string();
        }
        if draft.is_empty() {
            stop_reason = "thinker_failed".to_string();
            progress.phase("thinker failed to produce a draft");
            break;
        }
        progress.phase(&format!(
            "round {iteration}: draft ready chars={}",
            draft.chars().count()
        ));
        let review_prompt = reviewer_prompt(&topic, iteration, &draft, &state)?;
        progress.phase(format!("round {iteration}: reviewer checking"));
        let reviewer_system = REVIEWER_SYSTEM_PROMPT;
        let review_result = client
            .chat_stream(
                vec![
                    ChatMessage::system(reviewer_system),
                    ChatMessage::plain("user", review_prompt.clone()),
                ],
                Vec::new(),
                |chunk: ChatStreamChunk| {
                    if chunk.kind == ChatStreamKind::Reasoning {
                        progress.reasoning(&chunk.text);
                    }
                    Ok(())
                },
            )
            .await?;
        state
            .lock()
            .expect("deep research state lock")
            .stats
            .add_usage_or_estimate(
                review_result.usage.as_ref(),
                &[reviewer_system, &review_prompt, &review_result.content],
            );
        review = parse_review(&review_result.content);
        if review
            .get("accepted")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            stop_reason = "accepted".to_string();
            progress.phase(format!("round {iteration}: accepted"));
            break;
        }
        progress.phase(&format!(
            "round {iteration}: revision requested - {}",
            clip_inline(
                review
                    .get("challenge")
                    .and_then(Value::as_str)
                    .unwrap_or("reviewer requested changes"),
                100
            )
        ));
    }

    progress.phase("finalizing report");
    let mut final_answer = normalize_final_answer(&draft, &state)?;
    if plugin.max_final_answer_chars > 0
        && final_answer.chars().count() > plugin.max_final_answer_chars
    {
        final_answer = format!(
            "{}\n\n...[truncated to {} chars]",
            final_answer
                .chars()
                .take(plugin.max_final_answer_chars)
                .collect::<String>(),
            plugin.max_final_answer_chars
        );
    }
    let path = write_report(
        plugin,
        &context.paths,
        &topic,
        &final_answer,
        &state,
        &stop_reason,
        iterations,
        &state,
    )?;
    let stats = public_stats(&state);
    progress.phase(format!(
        "{} {} {} {} {}\n{} {}",
        t("tool calls", "工具调用"),
        stats["tool_calls"].as_u64().unwrap_or(0),
        t("times", "次"),
        t("token cost", "消耗 Token"),
        format_token_count(
            stats["token_estimate"].as_u64().unwrap_or(0),
            !stats["token_estimate_is_actual"].as_bool().unwrap_or(false)
        ),
        t("result file", "结果文件"),
        path.display()
    ));
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "kind": "deep_research",
        "topic": topic,
        "topic_title": topic_title(&state, &topic),
        "iterations_used": iterations,
        "stop_reason": stop_reason,
        "archive_path": path.display().to_string(),
        "final_answer": final_answer,
        "stats": stats,
        "sources": public_sources(&state)
    }))?)
}

fn research_tool_registry(
    context: &DeepResearchContext,
    state: Arc<Mutex<ResearchState>>,
) -> ToolRegistry {
    let mut registry = context.tools.clone();
    register_reference_tools(&mut registry, state);
    registry
}

fn register_reference_tools(registry: &mut ToolRegistry, state: Arc<Mutex<ResearchState>>) {
    let title_state = Arc::clone(&state);
    registry.register(ToolSpec::new(
        "register_deep_research_topic_title",
        "Register a concise title for this deep research task.",
        json!({"type":"object","properties":{"topic_title":{"type":"string"},"reason":{"type":"string"}},"required":["topic_title"],"additionalProperties":false}),
        move |args| {
            let title_state = Arc::clone(&title_state);
            async move {
                let title = args.get("topic_title").and_then(Value::as_str).unwrap_or_default();
                let title = sanitize_title(title, 40);
                let mut state = title_state.lock().expect("deep research state lock");
                state.topic_title = title.clone();
                Ok(json!({"ok": true, "topic_title": title}).to_string())
            }
        },
    ));
    let ref_state = Arc::clone(&state);
    registry.register(ToolSpec::new(
        "register_deep_research_reference",
        "Register a source and receive a stable citation marker such as [W1].",
        json!({"type":"object","properties":{"reference_type":{"type":"string","enum":["R","K","W","record","knowledge","web"]},"title":{"type":"string"},"url":{"type":"string"},"path":{"type":"string"},"snippet":{"type":"string"}},"required":["reference_type","title"],"additionalProperties":false}),
        move |args| {
            let ref_state = Arc::clone(&ref_state);
            async move {
                let kind = normalized_reference_kind(args.get("reference_type").and_then(Value::as_str).unwrap_or("W"));
                let title = args.get("title").and_then(Value::as_str).unwrap_or("Untitled").trim().to_string();
                let url = args.get("url").and_then(Value::as_str).unwrap_or_default().trim().to_string();
                let path = args.get("path").and_then(Value::as_str).unwrap_or_default().trim().to_string();
                let snippet = args.get("snippet").and_then(Value::as_str).unwrap_or_default().trim().to_string();
                let mut state = ref_state.lock().expect("deep research state lock");
                let number = match kind.as_str() {
                    "R" => { state.counters.record += 1; state.counters.record }
                    "K" => { state.counters.knowledge += 1; state.counters.knowledge }
                    _ => { state.counters.web += 1; state.counters.web }
                };
                let marker = format!("{kind}{number}");
                state.references.push(Reference { marker: marker.clone(), kind, title, url, path, snippet });
                Ok(json!({"ok": true, "ref": marker, "citation": format!("[{marker}]")}).to_string())
            }
        },
    ));
    registry.register(ToolSpec::new(
        "remove_deep_research_reference",
        "Remove a registered source by marker.",
        json!({"type":"object","properties":{"ref":{"type":"string"},"reason":{"type":"string"}},"required":["ref"],"additionalProperties":false}),
        move |args| {
            let state = Arc::clone(&state);
            async move {
                let marker = args.get("ref").and_then(Value::as_str).unwrap_or_default().trim().trim_matches(&['[', ']'][..]).to_string();
                let mut state = state.lock().expect("deep research state lock");
                let old_len = state.references.len();
                state.references.retain(|item| item.marker != marker);
                Ok(json!({"ok": old_len != state.references.len(), "ref": marker}).to_string())
            }
        },
    ));
}


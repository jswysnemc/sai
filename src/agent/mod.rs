mod compaction;
mod compaction_model;
mod context_projection;
mod conversation;
mod event;
mod external_events;
mod instruction_files;
mod lifecycle;
mod load_request;
mod message_context;
mod mode;
mod model_context;
mod recovery;
mod system_prompt;
mod tool_history;
mod tool_visibility;
mod turn_orchestration;

use crate::config::AppConfig;
use crate::llm::{
    ChatMessage, ChatResult, ChatStreamChunk, ChatStreamEvent, ChatStreamKind,
    OpenAiCompatibleClient,
};
use crate::memory::MemoryStore;
use crate::paths::SaiPaths;
use crate::perf_trace::PerfTrace;
use crate::state::request_projection::{
    project_provider_base_context_projection, project_provider_turn_from_base_projection,
    project_provider_turn_from_messages, DynamicContextSource, ProjectedBaseContext,
};
use crate::state::StateStore;
use crate::tools::{self, memes, ToolPermission, ToolRegistry};
use anyhow::{bail, Result};
use message_context::{runtime_context_message, system_messages_first};
use model_context::selected_model_label;
use system_prompt::build_base_system_prompt;
use tokio::sync::mpsc;
use tool_history::extract_persistable_tool_report;
use tool_visibility::ToolVisibility;

pub(crate) use compaction::CompactionRunOutcome;
pub use event::{AgentEvent, CompactionError};
pub(crate) use external_events::{ExternalEventBatch, ExternalEventWake};
pub use mode::AgentMode;

const MAX_QUESTION_ROUNDS_PER_TURN: usize = 8;
pub struct Agent {
    state: StateStore,
    client: OpenAiCompatibleClient,
    compaction_client: OpenAiCompatibleClient,
    compaction_model_label: String,
    base_system_prompt: String,
    // 上下文窗口 token 数经保守换算得到的字符预算，压缩触发与预算判断均用字符口径
    context_char_budget: usize,
    tools_enabled: bool,
    max_tool_rounds: usize,
    tools: ToolRegistry,
    tool_visibility: ToolVisibility,
    memory: MemoryStore,
    mode: AgentMode,
    config: AppConfig,
    paths: SaiPaths,
    last_dynamic_sources: Vec<DynamicContextSource>,
}

impl Agent {
    async fn chat_with_tools<F>(
        &mut self,
        turn_id: &str,
        messages: &mut Vec<ChatMessage>,
        used_tools: &mut Vec<String>,
        persisted_tool_reports: &mut Vec<(String, String)>,
        input: &str,
        image_urls: &[String],
        association_prompt: Option<&str>,
        auto_meme_reminder: Option<&str>,
        on_event: &mut F,
        perf: &mut PerfTrace,
    ) -> Result<ChatResult>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        let mut tool_round = 0usize;
        let mut tool_event_seq = self.state.tool_call_count_for_turn(turn_id)?;
        let mut todo_reminder = self
            .tools
            .contains("todo")
            .then(|| tools::todo::TodoReminder::new(self.state.todo_file()));
        let mut question_rounds = 0usize;
        let hook_ctx = crate::hooks::HookContext {
            session_id: self.state.session_id().to_string(),
            workdir: crate::runtime_cwd::current_dir()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            tool_name: None,
            extra: Default::default(),
        };
        crate::hooks::dispatch(
            &self.config.hooks,
            crate::hooks::HookEvent::AgentStart,
            &hook_ctx,
        )
        .await;
        loop {
            if self.max_tool_rounds > 0 && tool_round >= self.max_tool_rounds {
                let content = format!(
                    "工具调用已达到上限 {} 轮，已停止继续调用。可将 `tools.max_rounds` 设为 0 以允许无限工具调用。",
                    self.max_tool_rounds
                );
                on_event(AgentEvent::Chunk(ChatStreamChunk {
                    kind: ChatStreamKind::Content,
                    text: content.clone(),
                }))?;
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::AgentEnd,
                    &hook_ctx,
                )
                .await;
                return Ok(ChatResult {
                    content,
                    reasoning: None,
                    usage: None,
                    tool_calls: Vec::new(),
                });
            }
            tool_round += 1;
            crate::hooks::dispatch(
                &self.config.hooks,
                crate::hooks::HookEvent::TurnStart,
                &hook_ctx,
            )
            .await;
            crate::hooks::dispatch(
                &self.config.hooks,
                crate::hooks::HookEvent::MessageStart,
                &hook_ctx,
            )
            .await;
            self.compact_between_tool_rounds(
                tool_round,
                turn_id,
                messages,
                input,
                image_urls,
                association_prompt,
                auto_meme_reminder,
                on_event,
                perf,
            )
            .await?;
            let definitions = if self.tools_enabled {
                self.tool_visibility.definitions(&self.tools)
            } else {
                Vec::new()
            };
            perf.mark(&format!("round {tool_round} tool definitions"));
            let ordered_messages = system_messages_first(messages.clone());
            let projection = project_provider_turn_from_messages(
                &ordered_messages,
                definitions.len(),
                self.context_char_budget,
            );
            self.state
                .enforce_provider_projection(Some(turn_id), &projection)?;
            perf.mark(&format!("round {tool_round} provider projection"));
            let mut saw_reasoning = false;
            let mut saw_content = false;
            let mut saw_tool_progress = false;
            perf.mark(&format!("round {tool_round} model request start"));
            let result = self
                .client
                .chat_stream_events(ordered_messages, definitions.clone(), |event| match event {
                    ChatStreamEvent::Chunk(chunk) => {
                        match chunk.kind {
                            ChatStreamKind::Reasoning if !saw_reasoning => {
                                saw_reasoning = true;
                                perf.mark(&format!("round {tool_round} first reasoning chunk"));
                            }
                            ChatStreamKind::Content if !saw_content => {
                                saw_content = true;
                                perf.mark(&format!("round {tool_round} first content chunk"));
                            }
                            _ => {}
                        }
                        on_event(AgentEvent::Chunk(chunk))
                    }
                    ChatStreamEvent::ToolCallProgress(progress) => {
                        if !saw_tool_progress {
                            saw_tool_progress = true;
                            perf.mark(&format!("round {tool_round} first tool args chunk"));
                        }
                        on_event(AgentEvent::ToolCallProgress(progress))
                    }
                })
                .await?;
            perf.mark(&format!("round {tool_round} model request done"));
            if result.tool_calls.is_empty() || !self.tools_enabled {
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::MessageEnd,
                    &hook_ctx,
                )
                .await;
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::TurnEnd,
                    &hook_ctx,
                )
                .await;
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::AgentEnd,
                    &hook_ctx,
                )
                .await;
                return Ok(result);
            }
            messages.push(ChatMessage::assistant(
                result.content.clone(),
                Some(result.tool_calls.clone()),
            ));
            let ask_question_enabled = self.tools.contains("ask_question");
            let question_call_count = result
                .tool_calls
                .iter()
                .filter(|call| ask_question_enabled && call.function.name == "ask_question")
                .count();
            if question_call_count == 1 {
                question_rounds += 1;
            }
            let question_round_allowed =
                question_call_count == 1 && question_rounds <= MAX_QUESTION_ROUNDS_PER_TURN;
            let defer_sibling_tools = question_call_count == 1 && result.tool_calls.len() > 1;
            for call in result.tool_calls {
                tool_event_seq += 1;
                self.record_tool_call_started(turn_id, tool_event_seq, &call)?;
                used_tools.push(call.function.name.clone());
                perf.mark(&format!("tool {} call recorded", call.function.name));
                on_event(AgentEvent::ToolCall {
                    name: call.function.name.clone(),
                    arguments: call.function.arguments.clone(),
                })?;
                if ask_question_enabled && call.function.name == "ask_question" {
                    if question_call_count > 1 {
                        let output = "tool error: only one ask_question call is allowed per tool batch; combine all questions into one call".to_string();
                        self.record_tool_result_completed(turn_id, &call, false, &output, &output)?;
                        on_event(AgentEvent::ToolResult {
                            name: call.function.name.clone(),
                            ok: false,
                            output: output.clone(),
                        })?;
                        messages.push(ChatMessage::tool(call.id, output));
                        continue;
                    }
                    if !question_round_allowed {
                        let output = format!(
                            "tool error: ask_question exceeded the per-turn limit of {MAX_QUESTION_ROUNDS_PER_TURN}"
                        );
                        self.record_tool_result_completed(turn_id, &call, false, &output, &output)?;
                        on_event(AgentEvent::ToolResult {
                            name: call.function.name.clone(),
                            ok: false,
                            output: output.clone(),
                        })?;
                        messages.push(ChatMessage::tool(call.id, output));
                        continue;
                    }
                    let request =
                        match crate::question::QuestionRequest::parse(&call.function.arguments) {
                            Ok(request) => request,
                            Err(err) => {
                                let output =
                                    format!("tool error: invalid ask_question request: {err}");
                                self.record_tool_result_completed(
                                    turn_id, &call, false, &output, &output,
                                )?;
                                on_event(AgentEvent::ToolResult {
                                    name: call.function.name.clone(),
                                    ok: false,
                                    output: output.clone(),
                                })?;
                                messages.push(ChatMessage::tool(call.id, output));
                                continue;
                            }
                        };
                    let (pending, response_rx) =
                        crate::question::request_question(self.session_id(), request.clone());
                    let request_id = pending.id.clone();
                    on_event(AgentEvent::QuestionRequested(pending))?;
                    let response = response_rx
                        .await
                        .unwrap_or(crate::question::QuestionResponse::Cancelled);
                    on_event(AgentEvent::QuestionResolved {
                        request_id,
                        response: response.clone(),
                    })?;
                    let output = match response {
                        crate::question::QuestionResponse::Answered(answers) => {
                            match crate::question::QuestionExchange::new(request, answers) {
                                Ok(exchange) => crate::question::answered_tool_output(&exchange),
                                Err(err) => {
                                    let output =
                                        format!("tool error: invalid ask_question answers: {err}");
                                    self.record_tool_result_completed(
                                        turn_id, &call, false, &output, &output,
                                    )?;
                                    on_event(AgentEvent::ToolResult {
                                        name: call.function.name.clone(),
                                        ok: false,
                                        output: output.clone(),
                                    })?;
                                    messages.push(ChatMessage::tool(call.id, output));
                                    continue;
                                }
                            }
                        }
                        crate::question::QuestionResponse::Cancelled => {
                            let output = crate::question::unavailable_tool_output(
                                "user cancelled the question",
                            );
                            self.record_tool_result_completed(
                                turn_id, &call, false, &output, &output,
                            )?;
                            on_event(AgentEvent::ToolResult {
                                name: call.function.name.clone(),
                                ok: false,
                                output: output.clone(),
                            })?;
                            messages.push(ChatMessage::tool(call.id, output));
                            continue;
                        }
                        crate::question::QuestionResponse::Unavailable(reason) => {
                            crate::question::unavailable_tool_output(&reason)
                        }
                    };
                    self.record_tool_result_completed(turn_id, &call, true, &output, &output)?;
                    on_event(AgentEvent::ToolResult {
                        name: call.function.name.clone(),
                        ok: true,
                        output: output.clone(),
                    })?;
                    messages.push(ChatMessage::tool(call.id, output));
                    continue;
                }
                if defer_sibling_tools {
                    let output = "tool error: deferred until the user answers ask_question; reissue this tool call after receiving the answer".to_string();
                    self.record_tool_result_completed(turn_id, &call, false, &output, &output)?;
                    on_event(AgentEvent::ToolResult {
                        name: call.function.name.clone(),
                        ok: false,
                        output: output.clone(),
                    })?;
                    messages.push(ChatMessage::tool(call.id, output));
                    continue;
                }
                if self.mode == AgentMode::Plan
                    && self.tools.permission(&call.function.name)? != ToolPermission::ReadOnly
                {
                    let output = format!(
                        "tool error: Plan mode blocked non-read-only tool: {}",
                        call.function.name
                    );
                    self.record_tool_result_completed(turn_id, &call, false, &output, &output)?;
                    bail!(
                        "Plan mode blocked non-read-only tool: {}",
                        call.function.name
                    );
                }
                if self
                    .tools
                    .requires_permission(&call.function.name, &call.function.arguments)?
                {
                    self.tools.record_permission_requested(
                        &call.function.name,
                        &call.function.arguments,
                    )?;
                    // 自动审核：与人工审核并行；必须在 on_event（可能阻塞）之前启动
                    let (auto_task, auto_audit_active) = if self.mode == AgentMode::AutoAudit {
                        let context =
                            crate::permission::build_audit_context(&messages, 2_500);
                        let tool_name = call.function.name.clone();
                        let arguments = call.function.arguments.clone();
                        match crate::permission::resolve_auto_audit_client(
                            &self.config,
                            &self.paths,
                        ) {
                            Ok(audit_client) => {
                                // 先占位 request_id，创建请求后再克隆给任务
                                (Some((audit_client, context, tool_name, arguments)), true)
                            }
                            Err(_) => {
                                // 客户端不可用：静默回退人工审核
                                (None, false)
                            }
                        }
                    } else {
                        (None, false)
                    };
                    let (request, decision_rx) =
                        crate::permission::request_permission_with_auto_audit(
                            self.session_id(),
                            &call.function.name,
                            &call.function.arguments,
                            auto_audit_active,
                        );
                    let request_id = request.id.clone();
                    let auto_task = auto_task.map(|(audit_client, context, tool_name, arguments)| {
                        let audit_request_id = request_id.clone();
                        tokio::spawn(async move {
                            // 超时或失败时静默回退人工审核
                            match crate::permission::run_auto_audit(
                                &audit_client,
                                &audit_request_id,
                                &tool_name,
                                &arguments,
                                &context,
                            )
                            .await
                            {
                                Ok(_) => {}
                                Err(error) => {
                                    let message = format!("{error:#}");
                                    // 超时 / 竞态：完全静默；其它失败仅提示一次后回退人工
                                    if message.contains("timed out")
                                        || message.contains("timeout")
                                        || message.contains("no longer pending")
                                        || message.contains("no longer running")
                                    {
                                        return;
                                    }
                                    eprintln!("[sai] auto-audit fallback to human: {message}");
                                }
                            }
                        })
                    });
                    on_event(AgentEvent::PermissionRequested(request.clone()))?;
                    let decision = match decision_rx.await {
                        Ok(decision) => {
                            if let Some(task) = auto_task {
                                task.abort();
                            }
                            decision
                        }
                        Err(_) => {
                            if let Some(task) = auto_task {
                                let _ = task.await;
                            }
                            crate::permission::PermissionDecision::Deny {
                                reply: Some("权限审核通道已关闭".to_string()),
                            }
                        }
                    };
                    on_event(AgentEvent::PermissionResolved {
                        request_id,
                        decision: decision.clone(),
                    })?;
                    match decision {
                        crate::permission::PermissionDecision::Allow => {
                            self.tools.record_permission_approved(
                                &call.function.name,
                                &call.function.arguments,
                            )?;
                        }
                        crate::permission::PermissionDecision::Deny { reply } => {
                            self.tools.record_permission_denied(
                                &call.function.name,
                                &call.function.arguments,
                                reply.as_deref(),
                            )?;
                            let output = reply
                                .filter(|value| !value.trim().is_empty())
                                .unwrap_or_else(|| "用户拒绝了此工具调用".to_string());
                            self.record_tool_result_completed(
                                turn_id, &call, false, &output, &output,
                            )?;
                            on_event(AgentEvent::ToolResult {
                                name: call.function.name.clone(),
                                ok: false,
                                output: output.clone(),
                            })?;
                            messages.push(ChatMessage::tool(call.id, output));
                            continue;
                        }
                    }
                }
                if self.tool_visibility.is_loader_call(&call.function.name) {
                    let output = match self.tool_visibility.load_from_arguments(
                        &self.tools,
                        &call.function.arguments,
                        &self.config,
                        &self.paths,
                    ) {
                        Ok(output) => {
                            on_event(AgentEvent::ToolResult {
                                name: call.function.name.clone(),
                                ok: true,
                                output: output.clone(),
                            })?;
                            output
                        }
                        Err(err) => {
                            let output = format!("tool error: {err}");
                            on_event(AgentEvent::ToolResult {
                                name: call.function.name.clone(),
                                ok: false,
                                output: output.clone(),
                            })?;
                            output
                        }
                    };
                    let context_output =
                        tools::tool_output_for_context(&call.function.name, &output);
                    self.record_tool_result_completed(
                        turn_id,
                        &call,
                        !context_output.starts_with("tool error:"),
                        &output,
                        &context_output,
                    )?;
                    messages.push(ChatMessage::tool(call.id, context_output));
                    continue;
                }
                if !self.tool_visibility.is_visible(&call.function.name) {
                    let output = format!(
                        "tool error: tool {} is not loaded in the current visible tool set; call load with type=tool and a keywords array first. If this tool was loaded in a previous conversation, the loaded-tool session state was reset or is unavailable.",
                        call.function.name
                    );
                    on_event(AgentEvent::ToolResult {
                        name: call.function.name.clone(),
                        ok: false,
                        output: output.clone(),
                    })?;
                    let context_output =
                        tools::tool_output_for_context(&call.function.name, &output);
                    self.record_tool_result_completed(
                        turn_id,
                        &call,
                        false,
                        &output,
                        &context_output,
                    )?;
                    messages.push(ChatMessage::tool(call.id, context_output));
                    continue;
                }
                if call.function.name == "install_aur_package"
                    && used_tools.iter().any(|name| name == "review_aur_package")
                {
                    let output = "tool error: install_aur_package cannot run in the same turn as review_aur_package. This is a workflow confirmation error, not a tool loading error. Do not call load again; ask the user to confirm installation in a new turn first.".to_string();
                    on_event(AgentEvent::ToolResult {
                        name: call.function.name.clone(),
                        ok: false,
                        output: output.clone(),
                    })?;
                    let context_output =
                        tools::tool_output_for_context(&call.function.name, &output);
                    self.record_tool_result_completed(
                        turn_id,
                        &call,
                        false,
                        &output,
                        &context_output,
                    )?;
                    messages.push(ChatMessage::tool(call.id, context_output));
                    continue;
                }
                let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
                perf.mark(&format!("tool {} start", call.function.name));
                let mut tool_hook_ctx = hook_ctx.clone();
                tool_hook_ctx.tool_name = Some(call.function.name.clone());
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::ToolExecutionStart,
                    &tool_hook_ctx,
                )
                .await;
                let tool_future = self.tools.call_with_progress(
                    &call.function.name,
                    &call.function.arguments,
                    progress_tx,
                );
                tokio::pin!(tool_future);
                let output = loop {
                    tokio::select! {
                        result = &mut tool_future => {
                            break match result {
                                Ok(output) => {
                                    while let Ok(message) = progress_rx.try_recv() {
                                        on_event(AgentEvent::ToolProgress {
                                            name: call.function.name.clone(),
                                            message,
                                        })?;
                                    }
                                    on_event(AgentEvent::ToolResult {
                                        name: call.function.name.clone(),
                                        ok: true,
                                        output: output.clone(),
                                    })?;
                                    perf.mark(&format!("tool {} result event", call.function.name));
                                    if let Some(report) = extract_persistable_tool_report(
                                        &call.function.name,
                                        &output,
                                    ) {
                                        persisted_tool_reports
                                            .push((call.function.name.clone(), report));
                                    }
                                    output
                                }
                                Err(err) => {
                                    while let Ok(message) = progress_rx.try_recv() {
                                        on_event(AgentEvent::ToolProgress {
                                            name: call.function.name.clone(),
                                            message,
                                        })?;
                                    }
                                    on_event(AgentEvent::ToolResult {
                                        name: call.function.name.clone(),
                                        ok: false,
                                        output: format!("tool error: {err}"),
                                    })?;
                                    perf.mark(&format!("tool {} error event", call.function.name));
                                    format!("tool error: {err}")
                                }
                            };
                        }
                        Some(message) = progress_rx.recv() => {
                            on_event(AgentEvent::ToolProgress {
                                name: call.function.name.clone(),
                                message,
                            })?;
                        }
                    }
                };
                let context_output = tools::tool_output_for_context(&call.function.name, &output);
                self.record_tool_result_completed(
                    turn_id,
                    &call,
                    !output.starts_with("tool error:"),
                    &output,
                    &context_output,
                )?;
                perf.mark(&format!("tool {} result persisted", call.function.name));
                messages.push(ChatMessage::tool(call.id, context_output));
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::ToolExecutionEnd,
                    &tool_hook_ctx,
                )
                .await;
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::MessageEnd,
                    &hook_ctx,
                )
                .await;
                crate::hooks::dispatch(
                    &self.config.hooks,
                    crate::hooks::HookEvent::TurnEnd,
                    &hook_ctx,
                )
                .await;
                if let Some(reminder) = todo_reminder.as_mut() {
                    let todo_updated = call.function.name == "todo"
                        && !output.starts_with("tool error:")
                        && tools::todo::is_mutating_call(&call.function.arguments);
                    if let Some(content) = reminder.after_tool_round(todo_updated)? {
                        messages.push(ChatMessage::system(content));
                    }
                }
            }
        }
    }
}

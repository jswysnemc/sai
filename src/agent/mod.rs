mod agent_state;
mod compaction;
mod compaction_model;
mod compaction_replay;
pub(crate) mod compaction_schema;
mod context_projection;
mod context_resources;
mod conversation;
mod deepseek_anchor;
mod edit_guard;
mod event;
mod external_events;
mod external_tool_history;
mod external_turn;
mod instruction_files;
mod inter_message;
mod lifecycle;
mod load_request;
mod message_context;
mod message_gap;
mod message_request;
mod message_usage;
mod mode;
mod model_context;
mod recovery;
pub(crate) mod repeat_guard;
mod runtime_context;
mod skill_load;
pub(crate) mod system_prompt;
mod tool_attachments;
mod tool_batch_execution;
mod tool_execution;
mod tool_gate;
mod tool_history;
mod tool_invocation;
mod tool_visibility;
mod turn_execution;
mod turn_orchestration;
mod turn_settlement;

use crate::config::AppConfig;
use crate::llm::{
    ChatMessage, ChatResult, OpenAiCompatibleClient,
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
use anyhow::Result;
use message_context::system_messages_first;
use model_context::selected_model_label;
pub(crate) use runtime_context::{context_state_update, RuntimeContextSnapshot};
pub(crate) use tool_gate::{evaluate_tool_gate, ToolGate};
use tool_gate::{is_tool_error_output, tool_error_output};
pub(crate) use tool_visibility::ToolVisibility;
use turn_execution::assistant_tool_message;

pub use agent_state::Agent;
pub(crate) use compaction::CompactionRunOutcome;
pub(crate) use context_resources::{
    combine_context_updates, context_resource_update, context_resource_update_against_baseline,
};
pub use event::{AgentEvent, CompactionError, MessageContextUpdate};
pub(crate) use external_events::{ExternalEventBatch, ExternalEventWake};
pub(crate) use instruction_files::{extract_instruction_files, load_instruction_prompt};
pub(crate) use inter_message::{
    InterMessage, InterMessageEvent, InterMessageKind, InterMessageSource,
};
pub use mode::AgentMode;
pub(crate) use system_prompt::{build_base_system_prompt, build_base_system_prompt_for_phase};
pub(crate) use tool_invocation::resolve_execution_call;

impl Agent {
    async fn chat_with_tools<F>(
        &mut self,
        turn_id: &str,
        messages: &mut Vec<ChatMessage>,
        used_tools: &mut Vec<String>,
        input: &str,
        image_urls: &[String],
        memory_index_prompt: Option<&str>,
        auto_meme_reminder: Option<&str>,
        inter_message_source: Option<&dyn InterMessageSource>,
        wait_for_external: bool,
        on_event: &mut F,
        perf: &mut PerfTrace,
    ) -> Result<ChatResult>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        let mut tool_round = 0usize;
        // 跨轮累计同一工具调用的重复次数，防止模型对同一参数无限重复
        let mut repeat_guard = repeat_guard::RepeatGuard::default();
        let mut tool_event_seq = self.state.tool_call_count_for_turn(turn_id)?;
        let mut todo_reminder = self
            .tools
            .contains("todo")
            .then(|| tools::todo::TodoReminder::new(self.state.todo_file()));
        let mut question_rounds = 0usize;
        let mut pending_gap_delivery = None;
        let mut turn_usage = None;
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
            // 1. 模型请求开始前只消费一条就绪消息，避免合并多个完成回执
            if pending_gap_delivery.is_none() {
                if let Some(candidate) = self
                    .next_gap_message(inter_message_source, false, false, on_event)
                    .await?
                {
                    pending_gap_delivery = Some(self.inject_gap_message(
                        turn_id,
                        tool_event_seq,
                        candidate,
                        messages,
                        on_event,
                    )?);
                }
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
                memory_index_prompt,
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
            let was_anchor_bootstrap = self.tool_visibility.is_anchor_bootstrap();
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
            let mut result = match self
                .request_model_round(
                    turn_id,
                    ordered_messages,
                    definitions,
                    tool_round,
                    on_event,
                    perf,
                )
                .await
            {
                Ok(result) => result,
                Err(error) => {
                    if let Some(delivery) = pending_gap_delivery.take() {
                        self.rollback_gap_delivery(&delivery)?;
                    }
                    return Err(error);
                }
            };
            // 2. provider 成功接收请求后再确认来源，失败时保留队列项待重投
            if let Some(delivery) = pending_gap_delivery.take() {
                self.acknowledge_gap_delivery(delivery, inter_message_source)
                    .await?;
            }
            self.record_message_usage(&result)?;
            message_usage::accumulate_turn_usage(&mut turn_usage, result.usage.as_ref());
            on_event(AgentEvent::ContextUpdated(MessageContextUpdate {
                usage: result.usage.clone(),
                context_window_tokens: self.context_char_budget,
            }))?;
            tool_attachments::remove_pending_model_attachments(messages);
            if result.tool_calls.is_empty() || !self.tools_enabled {
                if let Some(candidate) = self
                    .next_gap_message(inter_message_source, wait_for_external, true, on_event)
                    .await?
                {
                    // 3. 已输出的助手消息属于同一持久化轮次，下一条消息在其后继续请求
                    self.persist_intermediate_assistant(
                        turn_id,
                        tool_event_seq,
                        &result,
                        messages,
                    )?;
                    if was_anchor_bootstrap {
                        self.promote_anchor_and_refresh_prompt(messages);
                    }
                    crate::hooks::dispatch(
                        &self.config.hooks,
                        crate::hooks::HookEvent::MessageEnd,
                        &hook_ctx,
                    )
                    .await;
                    pending_gap_delivery = Some(self.inject_gap_message(
                        turn_id,
                        tool_event_seq,
                        candidate,
                        messages,
                        on_event,
                    )?);
                    continue;
                }
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
                if was_anchor_bootstrap {
                    self.promote_anchor_and_refresh_prompt(messages);
                }
                result.usage = turn_usage;
                return Ok(result);
            }
            messages.push(assistant_tool_message(&result));
            let prepared_calls = tool_invocation::prepare_tool_calls(
                &self.tool_visibility,
                &self.tools,
                &result.tool_calls,
            );
            let (question_call_count, question_round_allowed, defer_sibling_tools) =
                prepared_calls.question_policy(&mut question_rounds);
            let mut round_model_attachments = Vec::new();
            let assistant_round = tool_event_seq.saturating_add(1);
            let groups = prepared_calls.into_execution_groups(
                &self.tools,
                &self.tool_visibility,
                assistant_round,
            );
            tool_event_seq = tool_event_seq.saturating_add(result.tool_calls.len());
            for group in groups {
                let serial_calls = match group {
                    tool_invocation::ToolExecutionGroup::ConcurrentReadOnly(group) => {
                        round_model_attachments.extend(
                            self.execute_concurrent_read_only_group(
                                turn_id,
                                assistant_round,
                                result.reasoning.as_deref(),
                                group,
                                used_tools,
                                messages,
                                &mut repeat_guard,
                                on_event,
                                perf,
                                &hook_ctx,
                            )
                            .await?,
                        );
                        continue;
                    }
                    tool_invocation::ToolExecutionGroup::Serial(call) => vec![call],
                };
                for item in serial_calls {
                    let provider_call = item.provider_call;
                    let recorded_call = &provider_call;
                    let execution_call = item.execution_call;
                    let Some(call) = self.begin_tool_invocation(
                        turn_id,
                        item.sequence,
                        assistant_round,
                        result.reasoning.as_deref(),
                        recorded_call,
                        execution_call,
                        messages,
                        &mut repeat_guard,
                        on_event,
                    )?
                    else {
                        continue;
                    };
                    used_tools.push(call.function.name.clone());
                    perf.mark(&format!("tool {} call recorded", call.function.name));
                    on_event(AgentEvent::ToolCall {
                        name: call.function.name.clone(),
                        arguments: call.function.arguments.clone(),
                    })?;
                    // 重复统计覆盖全部分支：被门禁或权限挡下的调用同样占用轮次，
                    // 漏掉它们会让反复发起的幻觉工具名无限空转
                    let repeat_verdict =
                        repeat_guard.observe(&call.function.name, &call.function.arguments);
                    if let repeat_guard::RepeatVerdict::Stop { seen } = repeat_verdict {
                        let output = repeat_guard::stop_notice(&call.function.name, seen);
                        on_event(AgentEvent::ToolResult {
                            name: call.function.name.clone(),
                            ok: false,
                            output: output.clone(),
                        })?;
                        self.record_simple_tool_result(turn_id, recorded_call, false, &output)?;
                        messages.push(ChatMessage::tool(call.id, output));
                        continue;
                    }
                    // 未知工具名、畸形参数、未加载工具等可恢复问题在专用流程前统一拦下
                    if let ToolGate::Reject(output) =
                        evaluate_tool_gate(&self.tools, &self.tool_visibility, &call, &used_tools)
                    {
                        // 门禁拒绝的调用重发多少次结果都一样，计入重复统计以便及时停止
                        repeat_guard
                            .observe_rejected(&call.function.name, &call.function.arguments);
                        self.record_simple_tool_result(turn_id, recorded_call, false, &output)?;
                        on_event(AgentEvent::ToolResult {
                            name: call.function.name.clone(),
                            ok: false,
                            output: output.clone(),
                        })?;
                        messages.push(ChatMessage::tool(call.id, output));
                        continue;
                    }
                    if matches!(call.function.name.as_str(), "str_replace" | "write_file") {
                        if let Err(error) = edit_guard::ensure_edit_target_was_read(self, &call) {
                            let output = format!("tool error: {error}");
                            repeat_guard
                                .observe_rejected(&call.function.name, &call.function.arguments);
                            self.record_simple_tool_result(turn_id, recorded_call, false, &output)?;
                            on_event(AgentEvent::ToolResult {
                                name: call.function.name.clone(),
                                ok: false,
                                output: output.clone(),
                            })?;
                            messages.push(ChatMessage::tool(call.id, output));
                            continue;
                        }
                    }
                    if call.function.name == "ask_question" {
                        if question_call_count > 1 {
                            let output = "tool error: only one ask_question call is allowed per tool batch; combine all questions into one call".to_string();
                            repeat_guard
                                .observe_rejected(&call.function.name, &call.function.arguments);
                            self.record_simple_tool_result(turn_id, recorded_call, false, &output)?;
                            on_event(AgentEvent::ToolResult {
                                name: call.function.name.clone(),
                                ok: false,
                                output: output.clone(),
                            })?;
                            messages.push(ChatMessage::tool(call.id, output));
                            continue;
                        }
                        if !question_round_allowed {
                            let output = tool_invocation::question_limit_notice();
                            repeat_guard
                                .observe_rejected(&call.function.name, &call.function.arguments);
                            self.record_simple_tool_result(turn_id, recorded_call, false, &output)?;
                            on_event(AgentEvent::ToolResult {
                                name: call.function.name.clone(),
                                ok: false,
                                output: output.clone(),
                            })?;
                            messages.push(ChatMessage::tool(call.id, output));
                            continue;
                        }
                        let request =
                            match crate::question::QuestionRequest::parse(&call.function.arguments)
                            {
                                Ok(request) => request,
                                Err(err) => {
                                    let output =
                                        format!("tool error: invalid ask_question request: {err}");
                                    repeat_guard.observe_rejected(
                                        &call.function.name,
                                        &call.function.arguments,
                                    );
                                    self.record_simple_tool_result(
                                        turn_id,
                                        recorded_call,
                                        false,
                                        &output,
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
                                    Ok(exchange) => {
                                        crate::question::answered_tool_output(&exchange)
                                    }
                                    Err(err) => {
                                        let output = format!(
                                            "tool error: invalid ask_question answers: {err}"
                                        );
                                        self.record_simple_tool_result(
                                            turn_id,
                                            recorded_call,
                                            false,
                                            &output,
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
                                self.record_simple_tool_result(
                                    turn_id,
                                    recorded_call,
                                    false,
                                    &output,
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
                        self.record_simple_tool_result(turn_id, recorded_call, true, &output)?;
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
                        self.record_simple_tool_result(turn_id, recorded_call, false, &output)?;
                        on_event(AgentEvent::ToolResult {
                            name: call.function.name.clone(),
                            ok: false,
                            output: output.clone(),
                        })?;
                        messages.push(ChatMessage::tool(call.id, output));
                        continue;
                    }
                    if self.mode() == AgentMode::Plan
                        && self.tools.permission(&call.function.name)? != ToolPermission::ReadOnly
                    {
                        let output = format!(
                            "tool error: Plan mode blocked non-read-only tool: {}",
                            call.function.name
                        );
                        repeat_guard
                            .observe_rejected(&call.function.name, &call.function.arguments);
                        self.record_simple_tool_result(turn_id, recorded_call, false, &output)?;
                        on_event(AgentEvent::ToolResult {
                            name: call.function.name.clone(),
                            ok: false,
                            output: output.clone(),
                        })?;
                        messages.push(ChatMessage::tool(call.id, output));
                        continue;
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
                        let (auto_task, auto_audit_active) = if self.mode() == AgentMode::AutoAudit
                        {
                            let context = crate::permission::build_audit_context(&messages, 2_500);
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
                        let auto_task =
                            auto_task.map(|(audit_client, context, tool_name, arguments)| {
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
                                            eprintln!(
                                                "[sai] auto-audit fallback to human: {message}"
                                            );
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
                        // 自动审核的放行理由写入审计日志，人工批准没有说明
                        let approval_detail = decision.detail().map(str::to_string);
                        match decision {
                            crate::permission::PermissionDecision::Allow { .. } => {
                                self.tools.record_permission_approved(
                                    &call.function.name,
                                    &call.function.arguments,
                                    approval_detail.as_deref(),
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
                                self.record_simple_tool_result(
                                    turn_id,
                                    recorded_call,
                                    false,
                                    &output,
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
                                let output = tool_error_output(&err);
                                repeat_guard.observe_rejected(
                                    &call.function.name,
                                    &call.function.arguments,
                                );
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
                            recorded_call,
                            !is_tool_error_output(&context_output),
                            &output,
                            &context_output,
                        )?;
                        messages.push(ChatMessage::tool(call.id, context_output));
                        continue;
                    }
                    perf.mark(&format!("tool {} start", call.function.name));
                    let mut tool_hook_ctx = hook_ctx.clone();
                    tool_hook_ctx.tool_name = Some(call.function.name.clone());
                    crate::hooks::dispatch(
                        &self.config.hooks,
                        crate::hooks::HookEvent::ToolExecutionStart,
                        &tool_hook_ctx,
                    )
                    .await;
                    let execution = self
                        .execute_real_tool(turn_id, &call, on_event, perf)
                        .await?;
                    round_model_attachments.extend(execution.model_attachments);
                    let output = execution.output;
                    if execution.failed || is_tool_error_output(&output) {
                        repeat_guard
                            .observe_rejected(&call.function.name, &call.function.arguments);
                    }
                    let context_output =
                        tools::tool_output_for_context(&call.function.name, &output);
                    // 重复调用尚未到拒绝阈值时照常返回结果，但提醒模型结果没有变化
                    let context_output = match repeat_verdict {
                        repeat_guard::RepeatVerdict::Warn { seen } => format!(
                            "{context_output}{}",
                            repeat_guard::warn_notice(&call.function.name, seen)
                        ),
                        _ => context_output,
                    };
                    self.record_tool_result_completed(
                        turn_id,
                        recorded_call,
                        !output.starts_with("tool error:"),
                        &output,
                        &context_output,
                    )?;
                    edit_guard::record_successful_reads(self, &call, &output)?;
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
            tool_attachments::append_model_attachments(messages, round_model_attachments);
            if was_anchor_bootstrap {
                self.promote_anchor_and_refresh_prompt(messages);
            }
        }
    }

    /// 首个持久 assistant/tool 信号后切换到完整系统提示。
    fn promote_anchor_and_refresh_prompt(&mut self, messages: &mut Vec<ChatMessage>) {
        if !self.tool_visibility.promote_anchor() {
            return;
        }
        if let Some(system) = messages.iter_mut().find(|message| message.role == "system") {
            system.content = Some(crate::llm::ChatContent::Text(
                self.base_system_prompt.clone(),
            ));
        }
    }
}

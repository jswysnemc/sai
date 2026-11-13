use super::progress_guard::{tool_succeeded, ProgressGuard};
use super::*;
use crate::agent::repeat_guard::{self, RepeatVerdict};
use crate::agent::{evaluate_tool_gate, resolve_execution_call, ToolGate};
use crate::tools::tool_output_for_context;
use crate::tools::ToolOutput;
use std::time::Duration;

impl SubagentRunner {
    /// 运行子代理的工具调用循环。
    ///
    /// 参数:
    /// - `messages`: 初始消息
    /// - `stats`: 可变统计信息
    ///
    /// 返回:
    /// - 子代理最终聊天结果
    pub(super) async fn chat_with_tools(
        &self,
        messages: &mut Vec<ChatMessage>,
        stats: &mut SubagentStats,
        tool_visibility: &mut ToolVisibility,
    ) -> Result<ChatResult> {
        let excluded = self
            .excluded_tools
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let mut steps = self
            .load_checkpoint()?
            .filter(|saved| !saved.turn_complete)
            .map_or(0, |saved| saved.steps);
        let mut repeat_guard = ProgressGuard::default();
        // 运行时限只提醒一次，避免每轮刷屏挤占上下文
        let started = tokio::time::Instant::now();
        let mut deadline_reminded = false;
        let mut round = 0;
        let mut stop_hook_active = false;
        loop {
            round += 1;
            if let Some(goal) = self
                .goal_accounting
                .as_ref()
                .map(|accounting| accounting.current_goal())
                .transpose()?
                .flatten()
                .filter(|goal| !goal.status.is_active())
            {
                messages.push(ChatMessage::plain("user", crate::goal::stop_prompt(&goal)));
                return self.finalize_without_tools(messages, round, stats).await;
            }
            // 0. 步间注入排队的追加消息（不打断进行中的工具调用）
            self.inject_queued_messages(messages);
            self.save_checkpoint(messages, stats, tool_visibility, steps, false, &[])?;
            // 1. 时限临近时注入软提醒；工具不受限，由子代理自行收敛
            if !deadline_reminded && self.session_deadline_seconds > 0 {
                let elapsed = started.elapsed().as_secs();
                let remaining = self.session_deadline_seconds.saturating_sub(elapsed);
                let threshold =
                    (self.session_deadline_seconds as f64 * DEADLINE_REMINDER_RATIO) as u64;
                if remaining <= threshold {
                    messages.push(ChatMessage::plain("user", deadline_reminder(remaining)));
                    deadline_reminded = true;
                }
            }
            if self.max_steps > 0 && steps >= self.max_steps {
                stats.budget_reached = true;
                messages.push(ChatMessage::plain("user", finalization_prompt()));
                return self.finalize_without_tools(messages, round, stats).await;
            }
            let result = self
                .request_with_context(messages, stats, tool_visibility, &excluded, round, steps)
                .await?;
            stats.record_response(messages, &result);
            // 每轮结束把累计用量写回快照：底部面板据此实时显示 token，
            // 否则长任务期间数字要等到任务结束才出现
            self.progress.stats(stats.public());
            if result.tool_calls.is_empty() {
                let stopped = crate::hooks::dispatch_with_tools(
                    &self.tools.hooks,
                    crate::hooks::HookEvent::SubagentStop,
                    &self
                        .tools
                        .hook_context()
                        .with("agent_type", &self.hook_agent_type)
                        .with("stop_hook_active", stop_hook_active)
                        .with("last_assistant_message", &result.content),
                    Some(&self.tools),
                )
                .await;
                if !stopped.stop && !stop_hook_active {
                    if let Some(reason) = stopped.deny {
                        messages.push(result.assistant_message());
                        messages.push(ChatMessage::plain("user", reason));
                        stop_hook_active = true;
                        continue;
                    }
                }
                return Ok(result);
            }
            messages.push(result.assistant_message());
            self.save_checkpoint(messages, stats, tool_visibility, steps, false, &[])?;
            let mut attachments = Vec::new();
            repeat_guard.begin_round();
            for provider_call in result.tool_calls {
                if self.max_steps > 0 && steps >= self.max_steps {
                    messages.push(ChatMessage::tool(
                        provider_call.id,
                        "tool budget reached for this subagent session",
                    ));
                    continue;
                }
                steps += 1;
                stats.tool_calls += 1;
                let call = match resolve_execution_call(tool_visibility, &provider_call) {
                    Ok(call) => call,
                    Err(error) => {
                        let verdict = repeat_guard.observe(
                            &provider_call.function.name,
                            &provider_call.function.arguments,
                        );
                        let output = match verdict {
                            RepeatVerdict::Stop { seen } => {
                                repeat_guard::stop_notice(&provider_call.function.name, seen)
                            }
                            _ => format!("tool error: {error:#}"),
                        };
                        if !matches!(verdict, RepeatVerdict::Stop { .. }) {
                            repeat_guard.record(
                                &provider_call.function.name,
                                &provider_call.function.arguments,
                                false,
                                &output,
                                Duration::ZERO,
                            );
                        }
                        self.progress
                            .tool_start(steps, &provider_call.function.name);
                        self.progress.tool_call_detail(
                            &provider_call.function.name,
                            &provider_call.function.arguments,
                        );
                        self.progress
                            .tool_end(steps, &provider_call.function.name, false, &output);
                        stats.tool_errors += 1;
                        messages.push(ChatMessage::tool(provider_call.id, output));
                        continue;
                    }
                };
                self.progress.tool_start(steps, &call.function.name);
                self.progress
                    .tool_call_detail(&call.function.name, &call.function.arguments);
                let repeat_verdict =
                    repeat_guard.observe(&call.function.name, &call.function.arguments);
                self.save_checkpoint(messages, stats, tool_visibility, steps, false, &attachments)?;
                let started = tokio::time::Instant::now();
                let (output, ok, _) = match repeat_verdict {
                    RepeatVerdict::Stop { seen } => (
                        ToolOutput::text(repeat_guard::stop_notice(&call.function.name, seen)),
                        false,
                        true,
                    ),
                    _ => {
                        self.execute_tool_call(&call, tool_visibility, &excluded)
                            .await
                    }
                };
                attachments.extend(output.model_attachments);
                let output = output.content;
                if !matches!(repeat_verdict, RepeatVerdict::Stop { .. }) {
                    repeat_guard.record(
                        &call.function.name,
                        &call.function.arguments,
                        ok,
                        &output,
                        started.elapsed(),
                    );
                }
                if ok {
                    stats.tool_ok += 1;
                } else {
                    stats.tool_errors += 1;
                }
                self.progress
                    .tool_end(steps, &call.function.name, ok, &output);
                let context_output = tool_output_for_context(&call.function.name, &output);
                let context_output = match repeat_verdict {
                    RepeatVerdict::Warn { seen } => format!(
                        "{context_output}{}",
                        repeat_guard::warn_notice(&call.function.name, seen)
                    ),
                    _ => context_output,
                };
                messages.push(ChatMessage::tool(provider_call.id, context_output));
                self.save_checkpoint(messages, stats, tool_visibility, steps, false, &attachments)?;
            }
            if let Some(message) = crate::tools::model_attachment::attachment_message(&attachments)
            {
                messages.push(message);
            }
            self.save_checkpoint(messages, stats, tool_visibility, steps, false, &[])?;
            // 【子任务】【循环防护】2. 连续无进展时撤下工具，仅请求一次已有结果汇总
            if repeat_guard.finish_round() {
                stats.loop_detected = true;
                self.progress.phase(if is_zh() {
                    "检测到重复调用且结果未变化，正在汇总已有结果"
                } else {
                    "Repeated calls produced no new results; summarizing available findings"
                });
                messages.push(ChatMessage::plain("user", "<subagent_loop_detected>连续多轮工具调用没有产生新结果，已停止重复执行。不要再请求工具，不要把未完成的工作说成已完成。请基于已有结果输出已完成部分、未完成部分及阻碍，交回主代理处理。</subagent_loop_detected>"));
                return self
                    .finalize_without_tools(messages, round + 1, stats)
                    .await;
            }
        }
    }

    /// 【子任务】【任务收尾】仅请求一次文本汇总，模型继续索要工具时明确失败。
    /// 参数：完整对话与请求序号；返回最终文本回复或无法收尾的错误。
    async fn finalize_without_tools(
        &self,
        messages: &[ChatMessage],
        round: usize,
        stats: &mut SubagentStats,
    ) -> Result<ChatResult> {
        let result = self
            .request_model_round(messages.to_vec(), Vec::new(), round)
            .await?;
        stats.record_response(messages, &result);
        anyhow::ensure!(
            result.tool_calls.is_empty(),
            "subagent finalization requested tools after execution was stopped"
        );
        anyhow::ensure!(
            !result.content.trim().is_empty(),
            "subagent finalization returned no summary"
        );
        Ok(result)
    }

    /// 校验并执行一条已经解包的真实工具调用。
    ///
    /// 参数:
    /// - `call`: 解包后的真实工具调用
    /// - `tool_visibility`: 当前渐进加载状态
    /// - `excluded`: 当前子 Agent 禁用的工具名称
    ///
    /// 返回:
    /// - 工具输出、成功状态与是否属于可重复判定的拒绝
    async fn execute_tool_call(
        &self,
        call: &crate::llm::ToolCall,
        tool_visibility: &mut ToolVisibility,
        excluded: &[&str],
    ) -> (ToolOutput, bool, bool) {
        if let Some(accounting) = &self.goal_accounting {
            match accounting.current_goal() {
                Ok(Some(goal)) if !goal.status.is_active() => {
                    return (
                        ToolOutput::text("tool error: parent goal execution stopped"),
                        false,
                        true,
                    )
                }
                Err(error) => {
                    return (
                        ToolOutput::text(format!("tool error: {error:#}")),
                        false,
                        true,
                    )
                }
                _ => {}
            }
        }
        if excluded.iter().any(|name| *name == call.function.name) {
            return (
                ToolOutput::text(format!(
                    "tool error: tool {} is disabled for this subagent",
                    call.function.name
                )),
                false,
                true,
            );
        }
        if let ToolGate::Reject(output) = evaluate_tool_gate(&self.tools, tool_visibility, call) {
            return (ToolOutput::text(output), false, true);
        }
        let prepared = match self.tools.prepare_hooked_call(call).await {
            Ok(prepared) => prepared,
            Err(error) => {
                return (
                    ToolOutput::text(format!("tool error: {error}")),
                    false,
                    true,
                )
            }
        };
        let call = &prepared.call;
        if tool_visibility.is_loader_call(&call.function.name) {
            let Some((config, paths)) = self.progressive_context.as_ref() else {
                return (
                    ToolOutput::text("tool error: progressive loading context is unavailable"),
                    false,
                    true,
                );
            };
            let result = tool_visibility
                .load_from_arguments(&self.tools, &call.function.arguments, config, paths)
                .map(ToolOutput::text);
            return match self.tools.finish_hooked_output(&prepared, result).await {
                Ok(output) => (output, true, false),
                Err(error) => (
                    ToolOutput::text(format!("tool error: {error}")),
                    false,
                    true,
                ),
            };
        }
        match tokio::time::timeout(
            Duration::from_secs(self.timeout_seconds.max(5)),
            self.call_tool_with_progress(&prepared),
        )
        .await
        {
            Ok(Ok(output)) => {
                let ok = tool_succeeded(&call.function.name, &output.content);
                (output, ok, false)
            }
            // 同一参数连续执行失败时计入拒绝统计，避免业务校验错误导致无限重试
            Ok(Err(error)) => (
                ToolOutput::text(format!("tool error: {error:#}")),
                false,
                true,
            ),
            Err(_) => (
                ToolOutput::text(format!(
                    "tool error: {} timed out after {}s",
                    call.function.name, self.timeout_seconds
                )),
                false,
                true,
            ),
        }
    }

    /// 【子任务】【工具进度】执行期间转发进度并保留全部模型附件
    /// 参数：`call` 为已授权调用；返回：文本与图片输出或执行错误
    async fn call_tool_with_progress(
        &self,
        prepared: &crate::tools::PreparedToolCall,
    ) -> Result<ToolOutput> {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let future = self.tools.execute_prepared(prepared, sender);
        tokio::pin!(future);
        loop {
            tokio::select! {
                result = &mut future => {
                    while let Ok(message) = receiver.try_recv() {
                        self.progress.phase(message);
                    }
                    return result;
                }
                Some(message) = receiver.recv() => self.progress.phase(message),
            }
        }
    }
}

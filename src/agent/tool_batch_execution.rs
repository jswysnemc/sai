use super::repeat_guard::{RepeatGuard, RepeatVerdict};
use super::tool_gate::{evaluate_tool_gate, is_tool_error_output, ToolGate};
use super::tool_invocation::SequencedToolCall;
use super::{Agent, AgentEvent};
use crate::hooks::{HookContext, HookEvent};
use crate::llm::{ChatMessage, ToolCall};
use crate::perf_trace::PerfTrace;
use crate::tools::{self, ToolModelAttachment};
use anyhow::Result;
use futures_util::{future::BoxFuture, stream::FuturesUnordered, FutureExt, StreamExt};

const MAX_CONCURRENT_READS: usize = 8;

/// 并发组中等待执行或已经完成门禁处理的槽位。
enum BatchSlot {
    Completed {
        call_id: String,
        context_output: String,
    },
    Ready {
        provider_call: ToolCall,
        call: ToolCall,
        repeat_verdict: RepeatVerdict,
        hook_context: Box<HookContext>,
    },
}

impl Agent {
    /// 【Agent】【只读并发】并发执行同一模型子轮中的连续只读工具。
    ///
    /// 门禁、重复调用防护与事件序号仍按供应商顺序处理；只有已经通过全部
    /// 同步检查的工具处理函数并发运行。结果完成后立即持久化和展示，模型消息最终按原始顺序组装。
    ///
    /// 参数:
    /// - `turn_id`: 当前持久化轮次标识
    /// - `assistant_round`: 产生本批调用的模型子轮编号
    /// - `assistant_reasoning`: 模型子轮思考内容
    /// - `group`: 已预分配事件序号的连续只读调用
    /// - `used_tools`: 本轮已经调用的工具名称
    /// - `messages`: 当前模型消息列表
    /// - `repeat_guard`: 跨子轮重复调用防护
    /// - `on_event`: Agent 事件回调
    /// - `perf`: 当前轮次性能标记器
    /// - `hook_context`: 当前轮 Hook 上下文
    ///
    /// 返回:
    /// - 下一次模型请求需要附带的工具附件
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn execute_concurrent_read_only_group<F>(
        &self,
        turn_id: &str,
        assistant_round: usize,
        assistant_reasoning: Option<&str>,
        group: Vec<SequencedToolCall>,
        used_tools: &mut Vec<String>,
        messages: &mut Vec<ChatMessage>,
        repeat_guard: &mut RepeatGuard,
        on_event: &mut F,
        perf: &mut PerfTrace,
        hook_context: &HookContext,
    ) -> Result<Vec<ToolModelAttachment>>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        let mut slots = Vec::with_capacity(group.len());

        // 1. 按调用顺序完成记录、事件、重复防护和门禁检查
        for item in group {
            let provider_call = item.provider_call;
            let Some(call) = self.begin_tool_invocation(
                turn_id,
                item.sequence,
                assistant_round,
                assistant_reasoning,
                &provider_call,
                item.execution_call,
                messages,
                repeat_guard,
                on_event,
            )?
            else {
                continue;
            };
            used_tools.push(call.function.name.clone());
            perf.mark(&format!("tool {} call recorded", call.function.name));
            on_event(AgentEvent::ToolCallIdentified {
                id: call.id.clone(),
                name: call.function.name.clone(),
                arguments: call.function.arguments.clone(),
            })?;

            let repeat_verdict =
                repeat_guard.observe(&call.function.name, &call.function.arguments);
            if let RepeatVerdict::Stop { seen } = repeat_verdict {
                let output = super::repeat_guard::stop_notice(&call.function.name, seen);
                self.record_simple_tool_result(turn_id, &provider_call, false, &output)?;
                on_event(AgentEvent::ToolResultIdentified {
                    id: call.id.clone(),
                    name: call.function.name.clone(),
                    ok: false,
                    output: output.clone(),
                })?;
                slots.push(BatchSlot::Completed {
                    call_id: call.id,
                    context_output: output,
                });
                continue;
            }

            if let ToolGate::Reject(output) =
                evaluate_tool_gate(&self.tools, &self.tool_visibility, &call)
            {
                repeat_guard.observe_rejected(&call.function.name, &call.function.arguments);
                self.record_simple_tool_result(turn_id, &provider_call, false, &output)?;
                on_event(AgentEvent::ToolResultIdentified {
                    id: call.id.clone(),
                    name: call.function.name.clone(),
                    ok: false,
                    output: output.clone(),
                })?;
                slots.push(BatchSlot::Completed {
                    call_id: call.id,
                    context_output: output,
                });
                continue;
            }

            let mut tool_hook_context = hook_context.clone();
            tool_hook_context.tool_name = Some(call.function.name.clone());
            crate::hooks::dispatch(
                &self.config.hooks,
                HookEvent::ToolExecutionStart,
                &tool_hook_context,
            )
            .await;
            perf.mark(&format!("tool {} start", call.function.name));
            slots.push(BatchSlot::Ready {
                provider_call,
                call,
                repeat_verdict,
                hook_context: Box::new(tool_hook_context),
            });
        }

        // 2. 【Agent】【并发调度】有界并发运行，进度和完成事件持续交给同一个事件接收器
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel(128);
        let mut results = (0..slots.len()).map(|_| None).collect::<Vec<_>>();
        let ready = slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| match slot {
                BatchSlot::Ready { call, .. } => Some((index, call.clone())),
                BatchSlot::Completed {
                    call_id,
                    context_output,
                } => {
                    results[index] = Some((
                        ChatMessage::tool(call_id.clone(), context_output.clone()),
                        Vec::new(),
                    ));
                    None
                }
            })
            .collect::<Vec<_>>();
        let futures = ready
            .into_iter()
            .map(|(index, call)| {
                let progress = progress_tx.clone();
                async move {
                    (
                        index,
                        self.execute_concurrent_tool(&call, index, progress).await,
                    )
                }
                .boxed()
            })
            .collect::<Vec<_>>();
        let pending = futures_util::stream::iter(futures).buffer_unordered(MAX_CONCURRENT_READS);
        tokio::pin!(pending);
        let mut hooks: FuturesUnordered<BoxFuture<'_, ()>> = FuturesUnordered::new();
        let mut tools_finished = false;
        loop {
            if tools_finished && hooks.is_empty() && progress_rx.is_empty() {
                break;
            }
            tokio::select! {
                Some((index, message)) = progress_rx.recv() => {
                    if let BatchSlot::Ready { call, .. } = &slots[index] {
                        on_event(AgentEvent::ToolProgressIdentified { id: call.id.clone(), name: call.function.name.clone(), message })?;
                    }
                }
                result = pending.next(), if !tools_finished => {
                    let Some((index, execution)) = result else { tools_finished = true; continue; };
                    while let Ok((slot, message)) = progress_rx.try_recv() {
                        if let BatchSlot::Ready { call, .. } = &slots[slot] {
                            on_event(AgentEvent::ToolProgressIdentified { id: call.id.clone(), name: call.function.name.clone(), message })?;
                        }
                    }
                    let BatchSlot::Ready { provider_call, call, repeat_verdict, hook_context } = &slots[index] else { unreachable!() };
                    let output = execution.output.clone();
                    if execution.failed || is_tool_error_output(&output) {
                        repeat_guard.observe_rejected(&call.function.name, &call.function.arguments);
                    }
                    let context_output = tools::tool_output_for_context(&call.function.name, &output);
                    let context_output = match repeat_verdict {
                        RepeatVerdict::Warn { seen } => format!("{context_output}{}", super::repeat_guard::warn_notice(&call.function.name, *seen)),
                        _ => context_output,
                    };
                    // 3. 【Agent】【逐项提交】先保存结果，再通知界面；慢工具不阻塞已完成工具落盘
                    self.record_tool_result_completed(turn_id, provider_call,
                        !execution.failed && !is_tool_error_output(&context_output), &output, &context_output)?;
                    let execution = self.finish_real_tool_execution(turn_id, call, execution,
                        &mut |event| on_event(match event {
                            AgentEvent::ToolResult { name, ok, output } => AgentEvent::ToolResultIdentified { id: call.id.clone(), name, ok, output },
                            other => other,
                        }), perf)?;
                    super::edit_guard::record_successful_reads(self, call, &output)?;
                    results[index] = Some((ChatMessage::tool(call.id.clone(), context_output), execution.model_attachments));
                    let hooks_config = self.config.hooks.clone();
                    let context = hook_context.clone();
                    hooks.push(async move {
                        for event in [HookEvent::ToolExecutionEnd, HookEvent::MessageEnd, HookEvent::TurnEnd] {
                            crate::hooks::dispatch(&hooks_config, event, &context).await;
                        }
                    }.boxed());
                }
                Some(()) = hooks.next(), if !hooks.is_empty() => {}
                else => break,
            }
            if tools_finished && hooks.is_empty() && progress_rx.is_empty() {
                break;
            }
        }
        // 4. 【Agent】【协议顺序】界面按完成时刻更新，模型仍收到供应商顺序的完整结果与附件
        let mut attachments = Vec::new();
        for (message, images) in results.into_iter().flatten() {
            messages.push(message);
            attachments.extend(images);
        }
        Ok(attachments)
    }
}

#[cfg(test)]
#[path = "tool_batch_execution_tests.rs"]
mod tests;

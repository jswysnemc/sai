use super::repeat_guard::{RepeatGuard, RepeatVerdict};
use super::tool_gate::{evaluate_tool_gate, is_tool_error_output, ToolGate};
use super::tool_invocation::SequencedToolCall;
use super::{Agent, AgentEvent};
use crate::hooks::{HookContext, HookEvent};
use crate::llm::{ChatMessage, ToolCall};
use crate::perf_trace::PerfTrace;
use crate::tools::{self, ToolModelAttachment};
use anyhow::Result;

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
        hook_context: HookContext,
    },
}

impl Agent {
    /// 【Agent】【只读并发】并发执行同一模型子轮中的连续只读工具。
    ///
    /// 门禁、重复调用防护与事件序号仍按供应商顺序处理；只有已经通过全部
    /// 同步检查的工具处理函数并发运行。结果按原始顺序写回消息和持久化状态。
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
            on_event(AgentEvent::ToolCall {
                name: call.function.name.clone(),
                arguments: call.function.arguments.clone(),
            })?;

            let repeat_verdict =
                repeat_guard.observe(&call.function.name, &call.function.arguments);
            if let RepeatVerdict::Stop { seen } = repeat_verdict {
                let output = super::repeat_guard::stop_notice(&call.function.name, seen);
                self.record_simple_tool_result(turn_id, &provider_call, false, &output)?;
                on_event(AgentEvent::ToolResult {
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
                evaluate_tool_gate(&self.tools, &self.tool_visibility, &call, used_tools)
            {
                repeat_guard.observe_rejected(&call.function.name, &call.function.arguments);
                self.record_simple_tool_result(turn_id, &provider_call, false, &output)?;
                on_event(AgentEvent::ToolResult {
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
                hook_context: tool_hook_context,
            });
        }

        // 2. 只并发运行工具处理函数，事件与持久化仍在下一步顺序提交
        let ready_calls = slots
            .iter()
            .filter_map(|slot| match slot {
                BatchSlot::Ready { call, .. } => Some(call),
                BatchSlot::Completed { .. } => None,
            })
            .collect::<Vec<_>>();
        let executions = futures_util::future::join_all(
            ready_calls
                .iter()
                .map(|call| self.execute_real_tool_buffered(call)),
        )
        .await;
        let mut executions = executions.into_iter();
        let mut attachments = Vec::new();

        // 3. 结果按供应商调用顺序回填，保证 tool_call_id 与消息顺序稳定
        for slot in slots {
            match slot {
                BatchSlot::Completed {
                    call_id,
                    context_output,
                } => messages.push(ChatMessage::tool(call_id, context_output)),
                BatchSlot::Ready {
                    provider_call,
                    call,
                    repeat_verdict,
                    hook_context,
                } => {
                    let buffered = executions.next().expect("missing buffered tool result");
                    let execution =
                        self.finish_buffered_real_tool(turn_id, &call, buffered, on_event, perf)?;
                    attachments.extend(execution.model_attachments);
                    let output = execution.output;
                    if execution.failed || is_tool_error_output(&output) {
                        repeat_guard
                            .observe_rejected(&call.function.name, &call.function.arguments);
                    }
                    let context_output =
                        tools::tool_output_for_context(&call.function.name, &output);
                    let context_output = match repeat_verdict {
                        RepeatVerdict::Warn { seen } => format!(
                            "{context_output}{}",
                            super::repeat_guard::warn_notice(&call.function.name, seen)
                        ),
                        _ => context_output,
                    };
                    self.record_tool_result_completed(
                        turn_id,
                        &provider_call,
                        !is_tool_error_output(&context_output),
                        &output,
                        &context_output,
                    )?;
                    super::edit_guard::record_successful_reads(self, &call, &output)?;
                    perf.mark(&format!("tool {} result persisted", call.function.name));
                    messages.push(ChatMessage::tool(call.id, context_output));
                    crate::hooks::dispatch(
                        &self.config.hooks,
                        HookEvent::ToolExecutionEnd,
                        &hook_context,
                    )
                    .await;
                    crate::hooks::dispatch(
                        &self.config.hooks,
                        HookEvent::MessageEnd,
                        &hook_context,
                    )
                    .await;
                    crate::hooks::dispatch(&self.config.hooks, HookEvent::TurnEnd, &hook_context)
                        .await;
                }
            }
        }
        Ok(attachments)
    }
}

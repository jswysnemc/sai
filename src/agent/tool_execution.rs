use super::deepseek_anchor;
use super::{tool_history::extract_persistable_tool_report, Agent, AgentEvent};
use crate::llm::ToolCall;
use crate::perf_trace::PerfTrace;
use crate::tools::ToolModelAttachment;
use anyhow::Result;
use tokio::sync::mpsc;

/// 单次真实工具执行完成后的内部结果。
pub(super) struct RealToolExecution {
    /// 返回给模型和界面的文本
    pub(super) output: String,
    /// 下一次模型请求需要携带的图片
    pub(super) model_attachments: Vec<ToolModelAttachment>,
    /// 工具处理函数是否返回错误
    pub(super) failed: bool,
}

/// 并发工具执行期间暂存的结果与进度。
pub(super) struct BufferedRealToolExecution {
    execution: RealToolExecution,
    progress: Vec<String>,
}

impl Agent {
    /// 【Agent】【工具执行】执行已经通过协议、门禁和权限检查的真实工具。
    ///
    /// 执行期间持续转发进度；完成后立即发出结果事件，并把需要跨消息保留的工具
    /// 报告写入当前轮次。协议解包、权限判断和重复防护不属于本方法职责。
    ///
    /// 参数:
    /// - `turn_id`: 当前持久化轮次标识
    /// - `call`: 已解析为真实工具名称和参数的调用
    /// - `on_event`: TUI、CLI 与 Web 共用的事件回调
    /// - `perf`: 当前轮次性能标记器
    ///
    /// 返回:
    /// - 工具文本、模型附件与失败状态
    pub(super) async fn execute_real_tool<F>(
        &self,
        turn_id: &str,
        call: &ToolCall,
        on_event: &mut F,
        perf: &mut PerfTrace,
    ) -> Result<RealToolExecution>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
        let requested_name = if call.kind == deepseek_anchor::BASH_EXECUTION_KIND {
            crate::tools::DSH_BASH_EXECUTION_ALIAS
        } else {
            &call.function.name
        };
        let tool_future =
            self.tools
                .call_with_progress(requested_name, &call.function.arguments, progress_tx);
        tokio::pin!(tool_future);

        loop {
            tokio::select! {
                result = &mut tool_future => {
                    // 1. 工具结束前排空进度队列，保证最后一次进度不会丢失
                    while let Ok(message) = progress_rx.try_recv() {
                        on_event(AgentEvent::ToolProgress {
                            name: call.function.name.clone(),
                            message,
                        })?;
                    }
                    // 2. 成功结果立即发出并持久化可复用报告
                    let execution = tool_result(result);
                    return self.finish_real_tool_execution(
                        turn_id,
                        call,
                        execution,
                        on_event,
                        perf,
                    );
                }
                Some(message) = progress_rx.recv() => {
                    on_event(AgentEvent::ToolProgress {
                        name: call.function.name.clone(),
                        message,
                    })?;
                }
            }
        }
    }

    /// 【Agent】【并发工具】执行只读工具并暂存进度与结果。
    ///
    /// 参数:
    /// - `call`: 已通过门禁与权限检查的只读调用
    ///
    /// 返回:
    /// - 等待调用方按原始顺序提交的执行结果
    pub(super) async fn execute_real_tool_buffered(
        &self,
        call: &ToolCall,
    ) -> BufferedRealToolExecution {
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
        let requested_name = if call.kind == deepseek_anchor::BASH_EXECUTION_KIND {
            crate::tools::DSH_BASH_EXECUTION_ALIAS
        } else {
            &call.function.name
        };
        let result = self
            .tools
            .call_with_progress(requested_name, &call.function.arguments, progress_tx)
            .await;
        let mut progress = Vec::new();
        while let Ok(message) = progress_rx.try_recv() {
            progress.push(message);
        }
        BufferedRealToolExecution {
            execution: tool_result(result),
            progress,
        }
    }

    /// 【Agent】【并发工具】按调用顺序提交暂存结果。
    ///
    /// 参数:
    /// - `turn_id`: 当前持久化轮次标识
    /// - `call`: 工具调用
    /// - `buffered`: 暂存的进度与结果
    /// - `on_event`: Agent 事件回调
    /// - `perf`: 当前轮次性能标记器
    ///
    /// 返回:
    /// - 已发出事件并持久化报告的执行结果
    pub(super) fn finish_buffered_real_tool<F>(
        &self,
        turn_id: &str,
        call: &ToolCall,
        buffered: BufferedRealToolExecution,
        on_event: &mut F,
        perf: &mut PerfTrace,
    ) -> Result<RealToolExecution>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        for message in buffered.progress {
            on_event(AgentEvent::ToolProgress {
                name: call.function.name.clone(),
                message,
            })?;
        }
        self.finish_real_tool_execution(turn_id, call, buffered.execution, on_event, perf)
    }

    /// 发出工具结果事件并保存可复用报告。
    fn finish_real_tool_execution<F>(
        &self,
        turn_id: &str,
        call: &ToolCall,
        execution: RealToolExecution,
        on_event: &mut F,
        perf: &mut PerfTrace,
    ) -> Result<RealToolExecution>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        on_event(AgentEvent::ToolResult {
            name: call.function.name.clone(),
            ok: !execution.failed,
            output: execution.output.clone(),
        })?;
        perf.mark(&format!(
            "tool {} {} event",
            call.function.name,
            if execution.failed { "error" } else { "result" }
        ));
        if !execution.failed {
            if let Some(report) =
                extract_persistable_tool_report(&call.function.name, &execution.output)
            {
                self.state
                    .append_tool_report_context(turn_id, &call.function.name, &report)?;
            }
        }
        Ok(execution)
    }
}

/// 把注册表执行结果转换为统一内部结果。
fn tool_result(result: anyhow::Result<crate::tools::ToolOutput>) -> RealToolExecution {
    match result {
        Ok(mut tool_output) => RealToolExecution {
            output: std::mem::take(&mut tool_output.content),
            model_attachments: tool_output.model_attachments,
            failed: false,
        },
        Err(error) => RealToolExecution {
            output: format!("tool error: {error}"),
            model_attachments: Vec::new(),
            failed: true,
        },
    }
}

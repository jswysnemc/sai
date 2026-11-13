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
    /// 需要随工具结果交给模型查看的图片
    pub(super) model_attachments: Vec<ToolModelAttachment>,
    /// 工具处理函数是否返回错误
    pub(super) failed: bool,
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
        &mut self,
        turn_id: &str,
        prepared: &crate::tools::PreparedToolCall,
        messages: &[crate::llm::ChatMessage],
        on_event: &mut F,
        perf: &mut PerfTrace,
    ) -> Result<RealToolExecution>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        let call = &prepared.call;
        if call.function.name == crate::tools::codemode::NAME {
            let result = self
                .execute_codemode(turn_id, call, messages, on_event)
                .await;
            let execution = self.finish_special_hook(prepared, result).await;
            return self.finish_real_tool_execution(turn_id, call, execution, on_event, perf);
        }
        if let Some(execution) = self.streamed_tools.take(call).await {
            return self.finish_real_tool_execution(turn_id, call, execution, on_event, perf);
        }
        let (progress_tx, mut progress_rx) = mpsc::channel(128);
        let tool_future = self.tools.execute_prepared(prepared, progress_tx);
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

    /// 【Agent】【并发工具】执行只读工具并持续向有界队列转发进度。
    /// 参数：调用、槽位序号和进度通道；返回工具最终结果，取消时释放工具 future。
    pub(super) async fn execute_concurrent_tool(
        &self,
        call: &ToolCall,
        slot: usize,
        progress: mpsc::Sender<(usize, String)>,
    ) -> RealToolExecution {
        match self.stopped_goal() {
            Ok(Some(goal)) => {
                return tool_result(Err(anyhow::anyhow!(crate::goal::stop_prompt(&goal))))
            }
            Err(error) => return tool_result(Err(error)),
            Ok(None) => {}
        }
        if let Some(execution) = self.streamed_tools.take(call).await {
            return execution;
        }
        let (sender, mut receiver) = mpsc::channel(128);
        let prepared = match self.tools.prepare_hooked_call(call).await {
            Ok(prepared) => prepared,
            Err(error) => return tool_result(Err(error)),
        };
        let future = self.tools.execute_prepared(&prepared, sender);
        tokio::pin!(future);
        loop {
            tokio::select! {
                result = &mut future => {
                    while let Ok(message) = receiver.try_recv() {
                        if progress.send((slot, message)).await.is_err() { break; }
                    }
                    return tool_result(result);
                }
                Some(message) = receiver.recv() => {
                    if progress.send((slot, message)).await.is_err() {
                        return tool_result(Err(anyhow::anyhow!("concurrent tool receiver closed")));
                    }
                }
            }
        }
    }

    /// 发出工具结果事件并保存可复用报告。
    pub(super) fn finish_real_tool_execution<F>(
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
            // 工具图片与结果一起持久化，历史投影和轮次内压缩据此重建附件消息
            self.state.record_tool_result_images(
                turn_id,
                &call.id,
                &execution.model_attachments,
            )?;
        }
        Ok(execution)
    }
}

/// 把注册表执行结果转换为统一内部结果。
pub(super) fn tool_result(result: anyhow::Result<crate::tools::ToolOutput>) -> RealToolExecution {
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

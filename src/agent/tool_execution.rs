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
        let tool_future = self.tools.call_with_progress(
            &call.function.name,
            &call.function.arguments,
            progress_tx,
        );
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
                    return match result {
                        Ok(mut tool_output) => {
                            let output = std::mem::take(&mut tool_output.content);
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
                                self.state.append_tool_report_context(
                                    turn_id,
                                    &call.function.name,
                                    &report,
                                )?;
                            }
                            Ok(RealToolExecution {
                                output,
                                model_attachments: tool_output.model_attachments,
                                failed: false,
                            })
                        }
                        Err(error) => {
                            let output = format!("tool error: {error}");
                            on_event(AgentEvent::ToolResult {
                                name: call.function.name.clone(),
                                ok: false,
                                output: output.clone(),
                            })?;
                            perf.mark(&format!("tool {} error event", call.function.name));
                            Ok(RealToolExecution {
                                output,
                                model_attachments: Vec::new(),
                                failed: true,
                            })
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
        }
    }
}

use super::{ReplRuntime, LIVE_REFRESH_INTERVAL};
use crate::agent::AgentEvent;
use crate::render::work_status::WorkStatus;
use crate::runner::RunnerEvent;
use anyhow::Result;
use std::time::Instant;

impl ReplRuntime {
    /// 记录一条 RunnerEvent 并将可显示部分插入历史区。
    ///
    /// 参数:
    /// - `event`: Runner 输出事件
    ///
    /// 返回:
    /// - 操作是否成功
    pub(in crate::cli) fn record_runner_event(&mut self, event: &RunnerEvent) -> Result<()> {
        let agent_event = match event {
            RunnerEvent::Started => {
                self.transcript.set_work_status(WorkStatus::WaitingResponse);
                self.arm_live_ticker();
                return self.sync_transcript(true);
            }
            RunnerEvent::AutomaticInput(input) => {
                self.next_live_refresh = None;
                self.live_sync_pending = false;
                self.transcript.finalize_live_tail();
                self.transcript.push_automatic_echo(input.content.clone());
                self.transcript.set_work_status(WorkStatus::WaitingResponse);
                self.arm_live_ticker();
                return self.sync_transcript(true);
            }
            RunnerEvent::WaitingExternal => {
                self.transcript.set_work_status(WorkStatus::WaitingExternal);
                self.arm_live_ticker();
                return self.sync_transcript(true);
            }
            RunnerEvent::Agent(agent_event) => agent_event,
            RunnerEvent::Interrupted | RunnerEvent::Completed(_) | RunnerEvent::Failed(_) => {
                self.next_live_refresh = None;
                self.live_sync_pending = false;
                self.transcript.finalize_live_tail();
                self.transcript.clear_work_status();
                return self.sync_transcript(false);
            }
            RunnerEvent::LoadedToolsChanged(_) => return Ok(()),
            RunnerEvent::FinalSummary(snapshot) => {
                // 本轮结束后追加上下文与耗时摘要；空行把总览和正文隔开，
                // 否则数据行紧贴最后一段回答，读起来像正文的一部分
                let summary = crate::render::session_summary::render_session_summary(snapshot);
                if !summary.trim().is_empty() {
                    self.transcript.finalize_live_tail();
                    self.transcript.clear_work_status();
                    self.transcript.push_meta(format!("\n{summary}"));
                    return self.sync_transcript(false);
                }
                return Ok(());
            }
        };
        // 【TUI】【增量同步】事件驱动的工作状态与动效节拍统一在此维护
        if let Some(status) = WorkStatus::from_agent_event(agent_event) {
            self.transcript.set_work_status(status);
            self.arm_live_ticker();
        }
        match agent_event {
            AgentEvent::Chunk(chunk) => {
                self.transcript.push_chunk(chunk);
                // 流式文本按固定节拍冲刷，避免长回复每个分片全量重排 markdown
                self.throttled_live_sync()
            }
            AgentEvent::InterMessage(message) => {
                self.transcript.finalize_live_tail();
                self.transcript.push_automatic_echo(message.content.clone());
                self.transcript.set_work_status(WorkStatus::WaitingResponse);
                self.arm_live_ticker();
                self.sync_transcript(true)
            }
            AgentEvent::WaitingExternal => {
                self.transcript.set_work_status(WorkStatus::WaitingExternal);
                self.arm_live_ticker();
                self.sync_transcript(true)
            }
            AgentEvent::ContextUpdated(_) => Ok(()),
            AgentEvent::ToolCall { name, arguments }
            | AgentEvent::ToolCallIdentified {
                name, arguments, ..
            } => {
                self.transcript
                    .push_tool_call(name.clone(), arguments.clone());
                self.sync_transcript(true)?;
                // 新子 agent 出现时底部 agent 面板需要重组
                if name == "subagent" {
                    self.redraw_stream_composer()?;
                }
                Ok(())
            }
            // 参数预览是临时 source；完整 ToolCall 到达后会替换为定稿工具块
            AgentEvent::ToolCallProgress(progress) => {
                self.transcript.push_tool_call_progress(progress);
                self.throttled_live_sync()
            }
            AgentEvent::ToolResult { name, ok, output }
            | AgentEvent::ToolResultIdentified {
                name, ok, output, ..
            } => {
                self.transcript
                    .push_tool_result(name.clone(), *ok, output.clone());
                self.sync_transcript(true)?;
                // todo 快照 / 子 agent 状态更新会改变沉底面板内容
                if name == "todo" || name == "subagent" {
                    self.redraw_stream_composer()?;
                }
                Ok(())
            }
            AgentEvent::ToolProgress { name, message }
            | AgentEvent::ToolProgressIdentified { name, message, .. } => {
                // 工具声明将直接写终端时，下一次同步前从光标处重启受管区域
                if message == "__external_output__" {
                    self.transcript.finalize_live_tail();
                    self.transcript.clear_work_status();
                    self.sync_transcript(true)?;
                    self.mark_desynced();
                    return Ok(());
                }
                if name == "run_command" {
                    if let Some(chunk) = crate::tools::command::decode_command_output(message) {
                        if self.transcript.push_command_output(name, &chunk) {
                            return self.throttled_live_sync();
                        }
                        return Ok(());
                    }
                }
                self.transcript
                    .push_tool_progress(name.clone(), message.clone());
                self.sync_transcript(true)
            }
            AgentEvent::PermissionRequested(request) => {
                // 权限选择期间不显示工作动效；审计 UI 由 sink 单独插入
                self.next_live_refresh = None;
                self.live_sync_pending = false;
                self.transcript.clear_work_status();
                let _ = request;
                // 立刻同步，避免上一状态的 working 行与审核控件同屏
                self.sync_transcript(false)
            }
            AgentEvent::PermissionResolved {
                request_id,
                decision,
            } => self.resolve_permission(request_id, decision.clone()),
            AgentEvent::QuestionRequested(_) => {
                self.next_live_refresh = None;
                self.live_sync_pending = false;
                self.transcript.clear_work_status();
                self.transcript.finalize_live_tail();
                self.sync_transcript(false)
            }
            // match 前的统一处理已恢复 Working 状态与节拍，这里立即上屏
            AgentEvent::QuestionResolved { .. } => self.sync_transcript(true),
            AgentEvent::CompactionStarted { turn_count, model } => {
                self.transcript
                    .push_compaction_started(*turn_count, model.clone());
                self.sync_transcript(true)
            }
            AgentEvent::CompactionDelta { text } => {
                // 与正文 Chunk 一致：流式期间保留 Compacting 状态行，不熄灭动效
                self.transcript.push_chunk(&crate::llm::ChatStreamChunk {
                    kind: crate::llm::ChatStreamKind::Content,
                    text: text.clone(),
                });
                self.throttled_live_sync()
            }
            AgentEvent::CompactionFinished {
                applied,
                summary,
                error,
            } => {
                // 压缩结束后模型将继续请求：回到等待态而不是熄灭动效
                self.transcript.set_work_status(WorkStatus::WaitingResponse);
                self.arm_live_ticker();
                self.transcript.push_compaction_finished(
                    *applied,
                    error.as_ref().map(|item| item.message.clone()),
                    error.as_ref().map(|item| item.detail.clone()),
                    summary
                        .as_ref()
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty()),
                );
                self.sync_transcript(true)
            }
            AgentEvent::EngineReady { engine, version } => {
                self.transcript.push_meta(format!(
                    "{} {engine} {version}",
                    crate::render::terminal_text("connected to", "已连接")
                ));
                self.sync_transcript(true)
            }
            AgentEvent::FlushContent => {
                self.transcript.finalize_live_tail();
                self.sync_transcript(true)
            }
            AgentEvent::ExternalOutput => {
                // 先冲刷既有内容，再标记失步等待外部程序写完
                self.transcript.finalize_live_tail();
                self.sync_transcript(true)?;
                self.mark_desynced();
                Ok(())
            }
        }
    }

    /// 启动 live 刷新节拍（已在运行时保持原节奏）。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    fn arm_live_ticker(&mut self) {
        if self.next_live_refresh.is_none() {
            self.next_live_refresh = Some(Instant::now() + LIVE_REFRESH_INTERVAL);
        }
    }

    /// 按固定节拍同步流式内容：未到期时仅标记待冲刷。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 操作是否成功
    fn throttled_live_sync(&mut self) -> Result<()> {
        let now = Instant::now();
        if let Some(next) = self.next_live_refresh {
            if now < next {
                self.live_sync_pending = true;
                return Ok(());
            }
        }
        self.next_live_refresh = Some(now + LIVE_REFRESH_INTERVAL);
        self.live_sync_pending = false;
        self.sync_transcript(true)
    }
}

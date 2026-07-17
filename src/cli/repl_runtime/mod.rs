mod composer_frame;
mod event_loop;
mod history_insert;
mod reflow;
mod reflow_state;
mod slash_panel;
mod stream;
mod viewport;

#[cfg(test)]
mod tests;

use crate::agent::{AgentEvent, AgentMode};
use crate::cli::repl_chrome::ReplChrome;
use crate::render::transcript::{
    TranscriptMode, TranscriptRenderOptions, TranscriptStore, WelcomeCell,
};
use crate::render::work_status::WorkStatus;
use crate::runner::RunnerEvent;
use anyhow::Result;
use std::io;
use std::time::{Duration, Instant};

use composer_frame::ComposerFrame;
use reflow_state::ReflowState;
use stream::{StreamState, StreamSync};
use viewport::{InlineViewport, TerminalSize};

const LIVE_REASONING_REFRESH_INTERVAL: Duration = Duration::from_millis(100);
const SUBAGENT_REFRESH_INTERVAL: Duration = Duration::from_millis(150);

/// REPL 的 source-backed transcript、inline viewport 与 resize reflow 运行期。
pub(super) struct ReplRuntime {
    transcript: TranscriptStore,
    options: TranscriptRenderOptions,
    viewport: InlineViewport,
    reflow: ReflowState,
    stream: StreamState,
    composer: Option<ComposerFrame>,
    next_live_reasoning_refresh: Option<Instant>,
    subagent_signature: Vec<(String, String, u64, u64)>,
}

impl ReplRuntime {
    /// 创建 REPL 终端运行期。
    ///
    /// 参数:
    /// - `row_cap`: transcript 尾部最大视觉行数
    /// - `options`: 初始 transcript 渲染选项
    ///
    /// 返回:
    /// - 新的 REPL 终端运行期
    pub(super) fn new(row_cap: usize, options: TranscriptRenderOptions) -> Self {
        let viewport = InlineViewport::new();
        let mut reflow = ReflowState::new();
        reflow.observe(viewport.size(), false);
        Self {
            transcript: TranscriptStore::new(row_cap),
            options,
            viewport,
            reflow,
            stream: StreamState::default(),
            composer: None,
            next_live_reasoning_refresh: None,
            subagent_signature: Vec::new(),
        }
    }

    /// 更新配置重载后的 transcript 渲染选项与 row cap。
    ///
    /// 参数:
    /// - `row_cap`: transcript 尾部最大视觉行数
    /// - `options`: 当前 transcript 渲染选项
    ///
    /// 返回:
    /// - 无
    pub(super) fn update_options(&mut self, row_cap: usize, options: TranscriptRenderOptions) {
        self.transcript.set_row_cap(row_cap);
        self.options = options;
    }

    /// 更新 composer source，并在边界变化时从 source 重放历史。
    ///
    /// 参数:
    /// - `chrome`: 当前输入框 chrome 状态
    /// - `input`: 原始输入文本
    /// - `cursor`: 光标字符偏移
    /// - `is_pasted`: 是否为粘贴内容
    ///
    /// 返回:
    /// - composer 顶部行号与视觉行数
    pub(super) fn update_composer(
        &mut self,
        chrome: &ReplChrome,
        input: &str,
        cursor: usize,
        is_pasted: bool,
        slash_selection: usize,
    ) -> Result<(u16, u16)> {
        let size = TerminalSize::current();
        let frame = ComposerFrame::new(
            chrome.clone(),
            input.to_string(),
            cursor,
            is_pasted,
            slash_selection,
        );
        self.composer = Some(frame);
        let lines = self
            .transcript
            .display_tail(usize::from(size.cols), &self.options);
        let composer_height = self.composer_height_for(size);
        let changed = self.viewport.update(size, composer_height, lines.len());
        if changed {
            self.reflow.schedule_immediate();
            self.maybe_reflow_due(false)?;
        }
        Ok((self.viewport.composer_top(), composer_height))
    }

    /// 按已保存的 source 重绘固定在底部的 composer。
    ///
    /// 参数:
    /// - `stdout`: 终端输出句柄
    ///
    /// 返回:
    /// - 绘制是否成功
    pub(super) fn draw_composer(&self, stdout: &mut io::Stdout) -> Result<()> {
        let Some(composer) = &self.composer else {
            return Ok(());
        };
        composer.draw(stdout, &self.viewport)
    }

    /// 结束 composer 绘制并释放底部 viewport 给历史输出。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 操作是否成功
    pub(super) fn end_composer(&mut self) -> Result<()> {
        self.composer = None;
        let size = TerminalSize::current();
        let lines = self
            .transcript
            .display_tail(usize::from(size.cols), &self.options);
        if self.viewport.update(size, 0, lines.len()) {
            self.reflow.schedule_immediate();
            self.maybe_reflow_due(false)?;
        }
        Ok(())
    }

    /// 处理输入阶段的 Resize 事件。
    ///
    /// 参数:
    /// - `cols`: 新终端列数
    /// - `rows`: 新终端行数
    ///
    /// 返回:
    /// - 无
    pub(super) fn observe_input_resize(&mut self, cols: u16, rows: u16) {
        self.observe_size(
            TerminalSize {
                cols: cols.max(1),
                rows: rows.max(1),
            },
            false,
        );
    }

    /// 在流式阶段采样终端尺寸。
    ///
    /// 参数:
    /// - `streaming`: 是否处于流式输出阶段
    ///
    /// 返回:
    /// - 操作是否成功
    pub(super) fn observe_terminal_size(&mut self, streaming: bool) -> Result<()> {
        self.observe_size(TerminalSize::current(), streaming);
        Ok(())
    }

    /// 返回下一次 pending resize reflow 的等待时长。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 尚未到期时的等待时长
    pub(super) fn pending_wait(&self) -> Option<std::time::Duration> {
        let reflow_wait = self
            .reflow
            .pending_until()
            .map(|deadline| deadline.saturating_duration_since(Instant::now()));
        let current_signature = self.transcript.subagent_signature();
        let subagent_wait = if current_signature != self.subagent_signature {
            Some(Duration::ZERO)
        } else {
            self.transcript
                .has_running_subagents()
                .then_some(SUBAGENT_REFRESH_INTERVAL)
        };
        match (reflow_wait, subagent_wait) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (Some(wait), None) | (None, Some(wait)) => Some(wait),
            (None, None) => None,
        }
    }

    /// 重放已经到期的 resize 请求。
    ///
    /// 参数:
    /// - `streaming`: 是否处于流式输出阶段
    ///
    /// 返回:
    /// - 是否完成重放
    pub(super) fn maybe_reflow_due(&mut self, streaming: bool) -> Result<bool> {
        if !self.reflow.is_due(Instant::now()) {
            return Ok(false);
        }
        self.reflow.clear_pending();
        self.replay(streaming)?;
        Ok(true)
    }

    /// 记录用户输入并立即插入 source-backed 历史。
    ///
    /// 参数:
    /// - `mode`: 用户提交时的 REPL 模式
    /// - `text`: 原始输入文本
    ///
    /// 返回:
    /// - 操作是否成功
    pub(super) fn record_user(&mut self, mode: AgentMode, text: String) -> Result<()> {
        self.transcript.push_user_echo(transcript_mode(mode), text);
        self.sync_transcript(false)
    }

    /// 记录控制命令、系统提示或错误信息。
    ///
    /// 参数:
    /// - `text`: 原始消息文本
    ///
    /// 返回:
    /// - 操作是否成功
    pub(super) fn record_meta(&mut self, text: String) -> Result<()> {
        self.transcript.push_meta(text);
        self.sync_transcript(false)
    }

    /// 记录等待用户处理的权限事件。
    ///
    /// 参数:
    /// - `request`: 权限请求
    ///
    /// 返回:
    /// - transcript 同步结果
    pub(super) fn record_permission_request(
        &mut self,
        request: crate::permission::PermissionRequest,
    ) -> Result<()> {
        self.transcript.push_permission_request(request);
        self.sync_transcript(false)
    }

    /// 更新 transcript 中权限事件的最终决定。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `decision`: 用户决定
    ///
    /// 返回:
    /// - transcript 同步结果
    pub(super) fn resolve_permission(
        &mut self,
        request_id: &str,
        decision: crate::permission::PermissionDecision,
    ) -> Result<()> {
        self.transcript.resolve_permission(request_id, decision);
        self.sync_transcript(false)
    }

    /// 更新权限事件中的内联拒绝回复草稿。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `draft`: 回复草稿；空值表示返回权限选择
    ///
    /// 返回:
    /// - transcript 同步结果
    pub(super) fn update_permission_reply(
        &mut self,
        request_id: &str,
        draft: Option<String>,
    ) -> Result<()> {
        self.transcript
            .set_permission_reply_draft(request_id, draft);
        self.sync_transcript(false)
    }

    /// 更新权限事件中的当前高亮选项。
    ///
    /// 参数:
    /// - `request_id`: 权限请求标识
    /// - `selected`: 高亮选项
    ///
    /// 返回:
    /// - transcript 同步结果
    pub(super) fn update_permission_choice(
        &mut self,
        request_id: &str,
        selected: crate::render::PermissionChoice,
    ) -> Result<()> {
        self.transcript.set_permission_choice(request_id, selected);
        self.sync_transcript(false)
    }

    /// 权限交互开始时暂停工作动效，避免遮挡审计选择。
    ///
    /// 返回:
    /// - transcript 同步结果
    pub(super) fn pause_for_permission_prompt(&mut self) -> Result<()> {
        self.next_live_reasoning_refresh = None;
        self.transcript.clear_work_status();
        self.transcript.finalize_live_tail();
        self.sync_transcript(false)
    }

    /// 记录本地 Shell 命令与输出。
    ///
    /// 参数:
    /// - `command`: Shell 命令
    /// - `output`: 命令输出
    /// - `exit_code`: 可选退出码
    ///
    /// 返回:
    /// - 同步 transcript 是否成功
    pub(super) fn record_shell(
        &mut self,
        command: String,
        output: String,
        exit_code: Option<i32>,
    ) -> Result<()> {
        self.transcript.push_shell(command, output, exit_code);
        self.sync_transcript(false)
    }

    /// 记录 REPL 启动欢迎面板。
    ///
    /// 参数:
    /// - `version`: 当前程序版本
    /// - `model`: 当前模型名称
    /// - `directory`: 当前工作目录
    /// - `permissions`: 当前权限模式
    ///
    /// 返回:
    /// - 操作是否成功
    pub(super) fn record_welcome(
        &mut self,
        version: String,
        model: String,
        directory: String,
        permissions: String,
    ) -> Result<()> {
        self.transcript.push_welcome(WelcomeCell {
            version,
            model,
            directory,
            permissions,
        });
        self.sync_transcript(false)
    }

    /// 记录一条 RunnerEvent 并将可显示部分插入历史区。
    ///
    /// 参数:
    /// - `event`: Runner 输出事件
    ///
    /// 返回:
    /// - 操作是否成功
    pub(super) fn record_runner_event(&mut self, event: &RunnerEvent) -> Result<()> {
        let agent_event = match event {
            RunnerEvent::Started => {
                self.transcript.set_work_status(WorkStatus::WaitingResponse);
                self.next_live_reasoning_refresh =
                    Some(Instant::now() + LIVE_REASONING_REFRESH_INTERVAL);
                return self.sync_transcript(true);
            }
            RunnerEvent::Agent(agent_event) => agent_event,
            RunnerEvent::Interrupted | RunnerEvent::Completed(_) | RunnerEvent::Failed(_) => {
                self.next_live_reasoning_refresh = None;
                self.transcript.finalize_live_tail();
                self.transcript.clear_work_status();
                return self.sync_transcript(false);
            }
            RunnerEvent::LoadedToolsChanged(_) | RunnerEvent::FinalSummary(_) => return Ok(()),
        };
        if let Some(status) = WorkStatus::from_agent_event(agent_event) {
            if self.transcript.set_work_status(status) {
                self.next_live_reasoning_refresh =
                    Some(Instant::now() + LIVE_REASONING_REFRESH_INTERVAL);
            } else if self.next_live_reasoning_refresh.is_none() {
                self.next_live_reasoning_refresh =
                    Some(Instant::now() + LIVE_REASONING_REFRESH_INTERVAL);
            }
        }
        match agent_event {
            AgentEvent::Chunk(chunk) => {
                let is_reasoning = chunk.kind == crate::llm::ChatStreamKind::Reasoning;
                let show_reasoning_live =
                    is_reasoning && self.next_live_reasoning_refresh.is_none();
                self.transcript.push_chunk(chunk);
                if is_reasoning {
                    if show_reasoning_live {
                        self.next_live_reasoning_refresh =
                            Some(Instant::now() + LIVE_REASONING_REFRESH_INTERVAL);
                        self.sync_transcript(true)
                    } else {
                        Ok(())
                    }
                } else {
                    self.next_live_reasoning_refresh = None;
                    self.sync_transcript(true)
                }
            }
            AgentEvent::ToolCall { name, arguments } => {
                self.next_live_reasoning_refresh = None;
                self.transcript
                    .push_tool_call(name.clone(), arguments.clone());
                self.sync_transcript(true)
            }
            // 参数预览是临时 source；完整 ToolCall 到达后会替换为定稿工具块
            AgentEvent::ToolCallProgress(progress) => {
                self.next_live_reasoning_refresh = None;
                self.transcript.push_tool_call_progress(progress);
                self.sync_transcript(true)
            }
            AgentEvent::ToolResult { name, ok, output } => {
                self.next_live_reasoning_refresh = None;
                self.transcript
                    .push_tool_result(name.clone(), *ok, output.clone());
                self.sync_transcript(true)
            }
            AgentEvent::ToolProgress { name, message } => {
                self.next_live_reasoning_refresh = None;
                self.transcript
                    .push_tool_progress(name.clone(), message.clone());
                self.sync_transcript(true)
            }
            AgentEvent::PermissionRequested(request) => {
                // 权限选择期间不显示工作动效
                self.next_live_reasoning_refresh = None;
                self.transcript.clear_work_status();
                let _ = request;
                Ok(())
            }
            AgentEvent::PermissionResolved {
                request_id,
                decision,
            } => {
                self.next_live_reasoning_refresh = None;
                self.resolve_permission(request_id, decision.clone())
            }
            AgentEvent::QuestionRequested(_) => {
                self.next_live_reasoning_refresh = None;
                self.transcript.clear_work_status();
                self.transcript.finalize_live_tail();
                self.sync_transcript(false)
            }
            AgentEvent::QuestionResolved { .. } => {
                self.next_live_reasoning_refresh = None;
                Ok(())
            }
            AgentEvent::CompactionStarted { turn_count, model } => {
                self.next_live_reasoning_refresh = None;
                self.transcript
                    .push_compaction_started(*turn_count, model.clone());
                self.sync_transcript(true)
            }
            AgentEvent::CompactionDelta { text } => {
                self.next_live_reasoning_refresh = None;
                self.transcript.clear_work_status();
                self.transcript.push_chunk(&crate::llm::ChatStreamChunk {
                    kind: crate::llm::ChatStreamKind::Content,
                    text: text.clone(),
                });
                self.sync_transcript(true)
            }
            AgentEvent::CompactionFinished {
                applied,
                error,
                ..
            } => {
                self.next_live_reasoning_refresh = None;
                self.transcript.clear_work_status();
                self.transcript.push_compaction_finished(
                    *applied,
                    error.as_ref().map(|item| item.message.clone()),
                    error.as_ref().map(|item| item.detail.clone()),
                );
                self.sync_transcript(true)
            }
            AgentEvent::FlushContent | AgentEvent::ExternalOutput => {
                self.next_live_reasoning_refresh = None;
                self.transcript.finalize_live_tail();
                self.sync_transcript(true)
            }
        }
    }

    /// 在流结束后收敛 source，并修复所有 stream-time reflow。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 操作是否成功
    pub(super) fn finish_stream(&mut self) -> Result<()> {
        self.next_live_reasoning_refresh = None;
        let consolidated = self.transcript.finalize_live_tail();
        if self.reflow.take_stream_finish_reflow_needed() {
            self.reflow.schedule_immediate();
            self.maybe_reflow_due(false)?;
        } else if consolidated {
            self.sync_transcript(false)?;
        }
        Ok(())
    }

    /// 清空 transcript 与终端的 Sai 输出区域。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 操作是否成功
    pub(super) fn clear(&mut self) -> Result<()> {
        self.transcript.clear();
        self.reflow.clear();
        self.stream.reset(Vec::new());
        self.next_live_reasoning_refresh = None;
        self.replay(false)
    }

    /// 立即按当前 viewport 从 source 重绘 REPL 终端。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 重绘是否成功
    pub(super) fn redraw(&mut self) -> Result<()> {
        self.replay(false)
    }

    /// 在固定节流周期内刷新 reasoning 字符计数与跳动状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否执行了 live 刷新
    pub(super) fn tick_live_reasoning(&mut self) -> Result<bool> {
        let Some(next_refresh) = self.next_live_reasoning_refresh else {
            return Ok(false);
        };
        let now = Instant::now();
        if now < next_refresh {
            return Ok(false);
        }
        if !self.transcript.advance_live_animation() {
            self.next_live_reasoning_refresh = None;
            return Ok(false);
        }
        // 工作状态或 reasoning 仍在进行时持续刷新动效与耗时
        self.next_live_reasoning_refresh = Some(now + LIVE_REASONING_REFRESH_INTERVAL);
        self.sync_transcript(true)?;
        Ok(true)
    }

    /// 刷新后台子智能体的持久化时间线。
    ///
    /// 返回:
    /// - 是否执行了 transcript 同步
    pub(super) fn tick_subagents(&mut self) -> Result<bool> {
        let signature = self.transcript.subagent_signature();
        if signature == self.subagent_signature {
            return Ok(false);
        }
        self.subagent_signature = signature;
        self.sync_transcript(true)?;
        Ok(true)
    }

    /// 处理输入阶段的定时重绘。
    ///
    /// 返回:
    /// - 是否执行了任何刷新
    pub(super) fn process_idle_tick(&mut self) -> Result<bool> {
        let reflowed = self.maybe_reflow_due(false)?;
        let subagents = self.tick_subagents()?;
        Ok(reflowed || subagents)
    }

    /// 记录终端尺寸变化并安排 resize reflow。
    fn observe_size(&mut self, size: TerminalSize, streaming: bool) {
        let lines = self
            .transcript
            .display_tail(usize::from(size.cols), &self.options);
        self.viewport
            .update(size, self.composer_height_for(size), lines.len());
        self.reflow.observe(size, streaming);
    }

    /// 将 transcript 与前一次已写入快照比较，必要时增量插入或全量重放。
    fn sync_transcript(&mut self, streaming: bool) -> Result<()> {
        let size = self.viewport.size();
        let width = usize::from(size.cols);
        let all_lines = self.transcript.display_tail(width, &self.options);
        let previous_viewport = self.viewport;
        if self
            .viewport
            .update(size, self.composer_height_for(size), all_lines.len())
            && size != previous_viewport.size()
        {
            return self.replay(streaming);
        }
        match self.stream.sync(all_lines) {
            StreamSync::Unchanged => Ok(()),
            StreamSync::Append(lines) => {
                let mut stdout = io::stdout();
                let outcome = history_insert::append_lines(
                    &mut stdout,
                    &previous_viewport,
                    &self.viewport,
                    &lines,
                )?;
                self.viewport.apply_terminal_scroll(outcome.scrolled_rows);
                self.draw_composer(&mut stdout)
            }
            StreamSync::Rebuild => self.replay(streaming),
        }
    }

    /// 清屏并从所有 source cell 重新插入当前宽度的历史行。
    fn replay(&mut self, streaming: bool) -> Result<()> {
        let size = TerminalSize::current();
        let width = usize::from(size.cols);
        let lines = self.transcript.display_tail(width, &self.options);
        self.viewport
            .update(size, self.composer_height_for(size), lines.len());
        let mut stdout = io::stdout();
        reflow::replay(&mut stdout, &self.viewport, &lines)?;
        self.draw_composer(&mut stdout)?;
        self.stream.reset(lines);
        self.reflow.mark_reflowed(size, streaming);
        Ok(())
    }

    /// 计算当前终端尺寸下 composer 需要保留的行数。
    ///
    /// 参数:
    /// - `size`: 当前终端尺寸
    ///
    /// 返回:
    /// - 不超过终端高度的 composer 行数
    fn composer_height_for(&self, size: TerminalSize) -> u16 {
        self.composer
            .as_ref()
            .map(|composer| composer.height(usize::from(size.cols)))
            .unwrap_or(0)
            .min(size.rows)
    }
}

/// 将 AgentMode 映射为 transcript 输入模式。
fn transcript_mode(mode: AgentMode) -> TranscriptMode {
    match mode {
        AgentMode::Plan => TranscriptMode::Plan,
        AgentMode::Audited => TranscriptMode::Yolo,
        AgentMode::Yolo => TranscriptMode::Yolo,
    }
}

pub(super) use event_loop::process_stream_tick;

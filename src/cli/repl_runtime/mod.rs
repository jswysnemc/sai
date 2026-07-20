mod composer_frame;
mod event_loop;
mod history;
mod history_insert;
mod reflow;
mod reflow_state;
mod runner_events;
mod slash_panel;
mod stream;
mod viewport;

#[cfg(test)]
mod tests;

use crate::agent::AgentMode;
use crate::cli::repl_chrome::ReplChrome;
use crate::cli::repl_clipboard::ReplClipboardBlockSpan;
use crate::render::transcript::{
    TranscriptMode, TranscriptRenderOptions, TranscriptStore, WelcomeCell,
};
use crate::state::{SessionTimelineCompaction, SessionTimelineTurn};
use anyhow::Result;
use crossterm::event::Event;
use std::collections::VecDeque;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use composer_frame::ComposerFrame;
use reflow_state::ReflowState;
use stream::{StreamState, SyncPlan};
use viewport::{InlineViewport, TerminalSize};

/// live 动效与流式文本的统一刷新周期。
const LIVE_REFRESH_INTERVAL: Duration = Duration::from_millis(100);
const SUBAGENT_REFRESH_INTERVAL: Duration = Duration::from_millis(150);

/// REPL 的 source-backed transcript、inline viewport 与增量协调运行期。
pub(super) struct ReplRuntime {
    transcript: TranscriptStore,
    options: TranscriptRenderOptions,
    viewport: InlineViewport,
    reflow: ReflowState,
    stream: StreamState,
    composer: Option<ComposerFrame>,
    next_live_refresh: Option<Instant>,
    live_sync_pending: bool,
    desynced: bool,
    subagent_signature: Vec<(String, String, u64, u64)>,
    pending_input_events: VecDeque<Event>,
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
            next_live_refresh: None,
            live_sync_pending: false,
            desynced: false,
            subagent_signature: Vec::new(),
            pending_input_events: VecDeque::new(),
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

    /// 更新 composer source，并在可视历史增高时从 source 重放。
    ///
    /// 参数:
    /// - `chrome`: 当前输入框 chrome 状态
    /// - `input`: 原始输入文本
    /// - `cursor`: 光标字符偏移
    /// - `is_pasted`: 是否为粘贴内容
    /// - `clipboard_blocks`: 剪贴板原子块区间
    /// - `slash_selection`: slash 面板当前选中项
    ///
    /// 返回:
    /// - composer 顶部行号与视觉行数
    pub(super) fn update_composer(
        &mut self,
        chrome: &ReplChrome,
        input: &str,
        cursor: usize,
        is_pasted: bool,
        clipboard_blocks: Vec<ReplClipboardBlockSpan>,
        slash_selection: usize,
    ) -> Result<(u16, u16)> {
        let size = TerminalSize::current();
        let frame = ComposerFrame::new(
            chrome.clone(),
            input.to_string(),
            cursor,
            is_pasted,
            clipboard_blocks,
            slash_selection,
        );
        self.composer = Some(frame);
        let previous_size = self.viewport.size();
        let previous_history = self.viewport.history_height();
        let composer_height = self.composer_height_for(size);
        // composer 需要的行数超过内容下方空余时，先滚动终端腾出空间，
        // 避免 composer 直接覆盖历史尾部（如启动于屏幕底部时的欢迎面板）
        self.reserve_composer_rows(size, composer_height)?;
        self.viewport
            .update(size, composer_height, self.stream.on_screen());
        if self.needs_replay_after_layout(previous_size, previous_history) {
            self.reflow.schedule_immediate();
            self.maybe_reflow_due(false)?;
        }
        Ok((self.viewport.composer_top(), composer_height))
    }

    /// 在内容尾部与屏幕底部之间为 composer 腾出足够行数。
    ///
    /// 不足时在屏幕底行输出换行触发真实滚动，上方内容进入原生
    /// scrollback；被 origin 上移吸收的部分不计入滚出行。
    ///
    /// 参数:
    /// - `size`: 当前终端尺寸
    /// - `composer_height`: composer 需要的行数
    ///
    /// 返回:
    /// - 操作是否成功
    fn reserve_composer_rows(&mut self, size: TerminalSize, composer_height: u16) -> Result<()> {
        let on_screen = self
            .stream
            .on_screen()
            .min(usize::from(size.rows))
            .min(usize::from(u16::MAX)) as u16;
        let content_bottom = self
            .viewport
            .origin_row()
            .saturating_add(on_screen)
            .min(size.rows);
        let free_rows = size.rows.saturating_sub(content_bottom);
        let deficit = composer_height.saturating_sub(free_rows);
        if deficit == 0 {
            return Ok(());
        }
        let mut stdout = io::stdout();
        crossterm::queue!(
            stdout,
            crossterm::cursor::MoveTo(0, size.rows.saturating_sub(1))
        )?;
        for _ in 0..deficit {
            crossterm::queue!(stdout, crossterm::style::Print("\r\n"))?;
        }
        stdout.flush()?;
        let absorbed = deficit.min(self.viewport.origin_row());
        self.viewport.apply_terminal_scroll(deficit);
        self.stream.note_scrolled(deficit.saturating_sub(absorbed));
        Ok(())
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
        let previous_size = self.viewport.size();
        let previous_history = self.viewport.history_height();
        self.viewport.update(size, 0, self.stream.on_screen());
        if self.needs_replay_after_layout(previous_size, previous_history) {
            self.reflow.schedule_immediate();
            self.maybe_reflow_due(false)?;
        }
        Ok(())
    }

    /// 判断布局变化后是否需要重放可视历史。
    ///
    /// 参数:
    /// - `previous_size`: 变化前的终端尺寸
    /// - `previous_history`: 变化前的可视历史行数
    ///
    /// 返回:
    /// - 尺寸变化或历史区域增高（露出被 composer 覆盖的行）时返回 true
    fn needs_replay_after_layout(
        &self,
        previous_size: TerminalSize,
        previous_history: u16,
    ) -> bool {
        self.viewport.size() != previous_size || self.viewport.history_height() > previous_history
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

    /// 将已保存的会话历史渲染到当前 TUI transcript。
    ///
    /// 参数:
    /// - `turns`: 按时间顺序排列的历史轮次
    ///
    /// 返回:
    /// - transcript 同步结果
    /// 将已保存的会话历史与压缩摘要渲染到当前 TUI transcript。
    ///
    /// 参数:
    /// - `turns`: 按时间顺序排列的历史轮次
    /// - `compaction`: 最新压缩摘要
    ///
    /// 返回:
    /// - transcript 同步结果
    pub(super) fn record_history_with_compaction(
        &mut self,
        turns: &[SessionTimelineTurn],
        compaction: Option<&SessionTimelineCompaction>,
    ) -> Result<()> {
        history::append_timeline_with_compaction(&mut self.transcript, turns, compaction);
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
        self.next_live_refresh = None;
        self.live_sync_pending = false;
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

    /// 在流结束后收敛 source，并修复所有 stream-time reflow。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 操作是否成功
    pub(super) fn finish_stream(&mut self) -> Result<()> {
        self.next_live_refresh = None;
        self.live_sync_pending = false;
        self.transcript.finalize_live_tail();
        self.transcript.clear_work_status();
        if self.reflow.take_stream_finish_reflow_needed() {
            self.reflow.schedule_immediate();
            self.maybe_reflow_due(false)?;
            return Ok(());
        }
        self.sync_transcript(false)
    }

    /// 标记终端已被外部程序写入，下一次同步前重启受管区域。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(super) fn mark_desynced(&mut self) {
        self.desynced = true;
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
        self.stream = StreamState::default();
        self.next_live_refresh = None;
        self.live_sync_pending = false;
        self.desynced = false;
        self.pending_input_events.clear();
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

    /// 在固定节流周期内刷新动效帧并冲刷待同步的流式内容。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否执行了 live 刷新
    pub(super) fn tick_live(&mut self) -> Result<bool> {
        let Some(next_refresh) = self.next_live_refresh else {
            return Ok(false);
        };
        let now = Instant::now();
        if now < next_refresh {
            return Ok(false);
        }
        let animated = self.transcript.advance_live_animation();
        let pending = std::mem::take(&mut self.live_sync_pending);
        if !animated && !pending {
            self.next_live_refresh = None;
            return Ok(false);
        }
        // 工作状态、reasoning 或未冲刷的正文仍在进行时保持节奏刷新
        self.next_live_refresh = Some(now + LIVE_REFRESH_INTERVAL);
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
        self.transcript.mark_subagents_dirty();
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
        self.viewport.update(
            size,
            self.composer_height_for(size),
            self.stream.on_screen(),
        );
        self.reflow.observe(size, streaming);
    }

    /// 将 transcript 与终端已写内容做增量协调。
    ///
    /// 稳定前缀不触碰；变化行按行修补；新增行走真实滚动进入原生
    /// scrollback；行数收缩时清理尾部。只有终端尺寸变化才整区重放。
    fn sync_transcript(&mut self, streaming: bool) -> Result<()> {
        if self.desynced {
            self.restart_after_external()?;
        }
        let size = self.viewport.size();
        let width = usize::from(size.cols);
        let min_rows = usize::from(size.rows).saturating_mul(2).max(64);
        let window =
            self.transcript
                .display_window(width, &self.options, min_rows, self.stream.offscreen());
        self.transcript.clear_dirty();
        let previous_viewport = self.viewport;
        self.viewport.update(
            size,
            self.composer_height_for(size),
            window.total.saturating_sub(self.stream.offscreen()),
        );
        if size != previous_viewport.size() {
            return self.replay(streaming);
        }
        match self.stream.sync(&window) {
            SyncPlan::Unchanged => Ok(()),
            SyncPlan::Delta {
                patches,
                append,
                old_total,
                new_total,
            } => {
                let mut stdout = io::stdout();
                let outcome = history_insert::apply_delta(
                    &mut stdout,
                    &previous_viewport,
                    &self.viewport,
                    &patches,
                    &append,
                    old_total,
                    new_total,
                    self.stream.offscreen(),
                )?;
                // 被 origin 上移吸收的滚动只是把屏幕整体上移，transcript 行并未滚出
                let absorbed = outcome.scrolled_rows.min(self.viewport.origin_row());
                self.viewport.apply_terminal_scroll(outcome.scrolled_rows);
                self.stream
                    .note_scrolled(outcome.scrolled_rows.saturating_sub(absorbed));
                self.draw_composer(&mut stdout)
            }
            SyncPlan::Repaint => self.replay(streaming),
        }
    }

    /// 清屏范围内从 source 重新铺设当前宽度的可视历史。
    fn replay(&mut self, streaming: bool) -> Result<()> {
        let size = TerminalSize::current();
        let width = usize::from(size.cols);
        // 重放窗口至少覆盖屏幕，同时尊重配置的 row cap 上限
        let min_rows = usize::from(size.rows)
            .saturating_mul(2)
            .max(64)
            .min(self.transcript.row_cap())
            .max(usize::from(size.rows));
        let window = self
            .transcript
            .display_window(width, &self.options, min_rows, usize::MAX);
        self.transcript.clear_dirty();
        self.viewport
            .update(size, self.composer_height_for(size), window.total);
        let mut stdout = io::stdout();
        let painted = reflow::replay(&mut stdout, &self.viewport, &window.lines)?;
        self.draw_composer(&mut stdout)?;
        self.stream.reset(&window, painted);
        self.reflow.mark_reflowed(size, streaming);
        Ok(())
    }

    /// 外部程序写过终端后，从当前光标行重启受管区域。
    ///
    /// 已有输出全部视作 scrollback 保留原样，后续内容从光标处追加。
    fn restart_after_external(&mut self) -> Result<()> {
        self.desynced = false;
        let mut stdout = io::stdout();
        let position = crossterm::cursor::position().unwrap_or((0, 0));
        if position.0 != 0 {
            write!(stdout, "\r\n")?;
            stdout.flush()?;
        }
        let size = TerminalSize::current();
        let origin = crossterm::cursor::position()
            .map(|(_, row)| row)
            .unwrap_or(position.1);
        self.viewport.restart_at(size, origin);
        self.stream.mark_all_offscreen();
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

pub(super) use event_loop::{process_stream_input, process_stream_tick};

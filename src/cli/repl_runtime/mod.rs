mod agent_panel;
mod bottom_panel;
mod composer;
mod composer_frame;
mod event_loop;
mod history;
mod history_insert;
mod history_replay;
mod layout;
mod live_usage;
mod mention_panel;
mod placeholder_tips;
mod queue_panel;
mod reflow;
mod reflow_state;
mod runner_events;
mod shell_hint_panel;
mod slash_panel;
mod stream;
mod viewport;

pub(in crate::cli) use queue_panel::QueuePanelIdleResult;

#[cfg(test)]
mod tests;

use crate::agent::AgentMode;
use crate::cli::repl_chrome::ReplChrome;
use crate::cli::repl_clipboard::ReplClipboardState;
use crate::cli::repl_windows_paste::WindowsPasteState;
use crate::render::activity_animation::ACTIVITY_FRAME_INTERVAL;
use crate::render::terminal_frame::TerminalFrame;
use crate::render::terminal_paint::paint_lock;
use crate::render::transcript::{TranscriptRenderOptions, TranscriptStore, WelcomeCell};
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
const LIVE_REFRESH_INTERVAL: Duration = ACTIVITY_FRAME_INTERVAL;
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
    /// 智能体运行期间编辑的草稿输入
    stream_draft: StreamComposerDraft,
    /// Tab 入队、等待当前轮结束后执行的提交
    submission_queue: VecDeque<QueuedSubmission>,
    /// 运行期间输入的斜杠命令，本轮结束后按序交主循环执行
    control_queue: VecDeque<String>,
    /// 运行中权限模式热切换句柄（与 Agent 共享）
    live_mode_handle: Option<std::sync::Arc<std::sync::atomic::AtomicU8>>,
    live_session_id: Option<String>,
    /// 最近一次 composer chrome，供流式阶段重建输入框
    last_chrome: Option<ReplChrome>,
    /// 底部主/子 agent 切换面板状态
    agent_panel: agent_panel::AgentPanelState,
    /// 用户消息队列管理面板状态
    queue_panel: queue_panel::QueuePanelState,
    /// 最近一次 composer 绘制后的光标屏幕行（高度变化重锚探测用）
    last_cursor_row: Option<u16>,
    /// 上次 composer 绘制的内容签名，用于跳过无变化的重绘
    last_composer_signature: Option<composer_frame::ComposerSignature>,
    /// `#` 引用可用的 skill 名称与描述
    mention_skills: Vec<(String, String)>,
    /// 沉底 todo 是否单行模式（Ctrl+T 切换）
    todo_panel_compact: bool,
    /// 当前轮已完成请求的实时用量，轮次结束后清空
    live_usage: live_usage::LiveTurnUsage,
    /// 当前帧的终端输出缓冲：整帧攒齐后一次提交
    frame: TerminalFrame,
}

/// 运行期间底部输入框草稿。
#[derive(Clone, Debug, Default)]
pub(super) struct StreamComposerDraft {
    pub(super) text: String,
    pub(super) cursor: usize,
    pub(super) is_pasted: bool,
    pub(super) clipboard: ReplClipboardState,
    pub(super) windows_paste: WindowsPasteState,
    pub(super) slash_selection: usize,
    pub(super) mode: Option<AgentMode>,
}

/// 队列中的下一条用户提交。
#[derive(Clone, Debug)]
pub(in crate::cli) struct QueuedSubmission {
    pub(in crate::cli) mode: AgentMode,
    pub(in crate::cli) text: String,
    /// 草稿携带的剪贴板附件；缺失时占位符会以字面文本发给模型
    pub(in crate::cli) clipboard: ReplClipboardState,
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
            stream_draft: StreamComposerDraft::default(),
            submission_queue: VecDeque::new(),
            control_queue: VecDeque::new(),
            live_mode_handle: None,
            live_session_id: None,
            last_chrome: None,
            agent_panel: agent_panel::AgentPanelState::default(),
            queue_panel: queue_panel::QueuePanelState::default(),
            last_cursor_row: None,
            last_composer_signature: None,
            mention_skills: Vec::new(),
            todo_panel_compact: true,
            live_usage: live_usage::LiveTurnUsage::default(),
            frame: TerminalFrame::new(),
        }
    }

    /// 更新 `#` 引用使用的 skill 目录。
    ///
    /// 参数:
    /// - `skills`: 名称与描述
    ///
    /// 返回:
    /// - 无
    pub(in crate::cli) fn set_mention_skills(&mut self, skills: Vec<(String, String)>) {
        self.mention_skills = skills;
    }

    /// 返回当前缓存的 skill 引用目录。
    ///
    /// 返回:
    /// - 名称与描述
    pub(in crate::cli) fn mention_skills(&self) -> &[(String, String)] {
        &self.mention_skills
    }

    /// 切换沉底 todo 单行 / 多行模式。
    ///
    /// 返回:
    /// - 有 todo 可切换并已翻转时为 true（调用方应重绘 composer）
    pub(super) fn toggle_todo_panel_compact(&mut self) -> bool {
        if self.transcript.latest_todo_items().is_empty() {
            return false;
        }
        self.todo_panel_compact = !self.todo_panel_compact;
        true
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

    /// 绑定当前轮 Agent 的热切换模式句柄。
    pub(in crate::cli) fn bind_live_mode(
        &mut self,
        handle: std::sync::Arc<std::sync::atomic::AtomicU8>,
        session_id: impl Into<String>,
    ) {
        self.live_mode_handle = Some(handle);
        self.live_session_id = Some(session_id.into());
    }

    pub(in crate::cli) fn clear_live_mode(&mut self) {
        self.live_mode_handle = None;
        self.live_session_id = None;
    }

    pub(in crate::cli) fn apply_stream_mode_live(
        &mut self,
        fallback: crate::agent::AgentMode,
    ) -> crate::agent::AgentMode {
        use crate::agent::AgentMode;
        use std::sync::atomic::Ordering;
        let mode = self.stream_mode(fallback);
        if let Some(handle) = self.live_mode_handle.as_ref() {
            handle.store(mode.as_u8(), Ordering::SeqCst);
        }
        if mode == AgentMode::Yolo {
            if let Some(session_id) = self.live_session_id.as_deref() {
                let _ = crate::permission::allow_all_pending_for_session(session_id);
            }
        }
        mode
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
        // 子智能体视图的扫光按 live 节拍唤醒，与主 agent 动效保持同一帧率
        let animation_wait = self
            .transcript
            .viewing_running_subagent()
            .then_some(LIVE_REFRESH_INTERVAL);
        [reflow_wait, subagent_wait, animation_wait]
            .into_iter()
            .flatten()
            .min()
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
    /// - `text`: 回显正文（粘贴长文本应已展开）
    /// - `fold`: 仅粘贴长文本为 true，启用思考式折叠
    ///
    /// 返回:
    /// - 操作是否成功
    pub(super) fn record_user(&mut self, mode: AgentMode, text: String, fold: bool) -> Result<()> {
        self.transcript
            .push_user_echo_with_fold(layout::transcript_mode(mode), text, fold);
        self.sync_transcript(false)
    }

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

    /// 记录轮次失败或中断提示，带失败专属样式。
    ///
    /// 参数:
    /// - `text`: 失败说明
    ///
    /// 返回:
    /// - transcript 同步结果
    pub(super) fn record_failure(&mut self, text: String) -> Result<()> {
        self.transcript.push_failure(text);
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
        self.stream_draft = StreamComposerDraft::default();
        self.submission_queue.clear();
        self.agent_panel.deactivate();
        self.queue_panel.deactivate();
        self.replay(false)
    }

    /// 从备用屏/外部 UI 返回后恢复 TUI 输入与显示。
    ///
    /// 备用屏会完整还原主缓冲内容，因此默认只重绘输入框并校准尺寸；
    /// 尺寸变化时再全量重放，避免原点漂移导致输入框错位。
    ///
    /// 返回:
    /// - 是否成功
    pub(super) fn resync_after_overlay(&mut self) -> Result<()> {
        self.desynced = false;
        // 1. 确保 raw mode（pager 可能曾临时关闭）
        if !crossterm::terminal::is_raw_mode_enabled().unwrap_or(false) {
            crossterm::terminal::enable_raw_mode()?;
        }
        let previous = self.viewport.size();
        let size = TerminalSize::current();
        // 2. 尺寸变了：全量重放（replay 检测到尺寸差异后自动重锚）；
        //    否则仅重绘 composer 恢复光标
        if size != previous {
            self.replay(false)?;
        }
        self.redraw_stream_composer()?;
        Ok(())
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
    /// 下一次到期时间由上一次的计划时刻累加，而不是从本次实际唤醒时刻起算：
    /// 主循环 25ms 一跳、刷新间隔 32ms，两者不整除，按唤醒时刻累加会让每轮
    /// 都多等一个 tick，实际间隔被拉到 50ms。按计划时刻推进则只是对齐到
    /// 最近的 tick，长期平均仍是 32ms。
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
        // 工作状态、reasoning 或未冲刷的正文仍在进行时保持节奏刷新；
        // 落后超过一个周期时（如终端卡顿）重新对齐到当前时刻，不追补欠帧
        let planned = next_refresh + LIVE_REFRESH_INTERVAL;
        self.next_live_refresh = Some(if planned > now {
            planned
        } else {
            now + LIVE_REFRESH_INTERVAL
        });
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
        // 停留在运行中的子智能体视图，或后台仍有子智能体运行（底部面板
        // 的流光与实时统计）时，空闲期也要驱动 live 刷新
        if self.next_live_refresh.is_none()
            && (self.transcript.viewing_running_subagent()
                || self.transcript.has_running_subagents())
        {
            self.next_live_refresh = Some(Instant::now());
        }
        let animated = self.tick_live()?;
        Ok(reflowed || subagents || animated)
    }

    /// 记录终端尺寸变化并安排 resize reflow。
    ///
    /// 只登记观察，不动 viewport 记账：resize 后旧 origin 不可信，
    /// 统一交给 debounce 到期后的 replay 重新锚定。
    fn observe_size(&mut self, size: TerminalSize, streaming: bool) {
        self.reflow.observe(size, streaming);
    }

    /// 将 transcript 与终端已写内容做增量协调。
    ///
    /// 稳定前缀不触碰；变化行按行修补；新增行走真实滚动进入原生
    /// scrollback；行数收缩时清理尾部。resize 未收敛期间冻结增量。
    fn sync_transcript(&mut self, streaming: bool) -> Result<()> {
        if self.desynced {
            self.restart_after_external()?;
        }
        // 1. resize 未收敛：冻结增量修补，等 debounce 到期后整区重锚。
        //    新宽度渲染 + 旧宽度定位会交叉折行画花屏幕
        let current = TerminalSize::current();
        if current != self.viewport.size() {
            self.reflow.observe(current, streaming);
            if self.reflow.pending_until().is_none() {
                self.reflow.schedule_immediate();
            }
            self.live_sync_pending = true;
            return Ok(());
        }
        if self.reflow.pending_until().is_some() {
            self.live_sync_pending = true;
            return Ok(());
        }
        let size = current;
        // 沉底面板承载子智能体状态/动效帧，先于布局重算，不等输入变化；
        // 行数变化引发的腾行滚动发生在窗口计算前，增量记账才不会错位
        self.refresh_bottom_panel()?;
        let width = usize::from(size.cols);
        let min_rows = usize::from(size.rows).saturating_mul(2).max(64);
        let window = layout::display_window(
            &mut self.transcript,
            width,
            &self.options,
            min_rows,
            self.stream.offscreen(),
            layout::live_preview_cap(size),
        );
        self.transcript.clear_dirty();
        let previous_viewport = self.viewport;
        self.viewport.update(
            size,
            self.composer_height_for(size),
            window.total.saturating_sub(self.stream.offscreen()),
        );
        match self.stream.sync(&window) {
            // transcript 行未变时 composer 仍需刷新：底部面板的动效帧与
            // 实时统计在变，且要把被外部绘制移走的光标收回输入位置
            SyncPlan::Unchanged => {
                self.queue_composer()?;
                self.commit_frame()
            }
            SyncPlan::Delta {
                patches,
                append,
                old_total,
                new_total,
            } => {
                let outcome = history_insert::apply_delta(
                    &mut self.frame,
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
                // 历史插入可能滚过 composer 区域，缓存的签名不再代表屏幕现状
                if outcome.scrolled_rows > 0 {
                    self.last_composer_signature = None;
                }
                self.queue_composer()?;
                self.commit_frame()
            }
            SyncPlan::Repaint => self.replay(streaming),
        }
    }

    /// 清屏范围内从 source 重新铺设当前宽度的可视历史。
    fn replay(&mut self, streaming: bool) -> Result<()> {
        // 重锚要读终端的真实光标位置，腾行等已入帧的绘制必须先送达
        self.commit_frame()?;
        let size = TerminalSize::current();
        let previous = self.viewport.size();
        // 1. 分层重锚：仅高度变化时折行不变，scrollback 内容依旧有效，
        //    用光标位移只修正记账即可保留回滚历史与用户滚动进度；
        //    宽度变化或探测失败才清空 scrollback 全量重建
        let full_reanchor = size != previous
            && !(size.cols == previous.cols && self.reanchor_for_height_change(size));
        if full_reanchor {
            self.viewport.restart_at(size, 0);
        }
        let width = usize::from(size.cols);
        // 2. 重放窗口至少覆盖屏幕，同时尊重配置的 row cap 上限；
        //    全量重锚会重建 scrollback，因此取完整 row cap 保留回滚历史
        let min_rows = if full_reanchor {
            self.transcript.row_cap().max(usize::from(size.rows))
        } else {
            usize::from(size.rows)
                .saturating_mul(2)
                .max(64)
                .min(self.transcript.row_cap())
                .max(usize::from(size.rows))
        };
        let window = layout::display_window(
            &mut self.transcript,
            width,
            &self.options,
            min_rows,
            usize::MAX,
            layout::live_preview_cap(size),
        );
        self.transcript.clear_dirty();
        self.viewport
            .update(size, self.composer_height_for(size), window.total);
        let painted = if full_reanchor {
            reflow::replay_full(&mut self.frame, &self.viewport, &window.lines)?
        } else {
            reflow::replay(&mut self.frame, &self.viewport, &window.lines)?
        };
        // 整区重放后屏幕内容已被覆盖，必须强制重绘 composer
        self.last_composer_signature = None;
        self.queue_composer()?;
        self.commit_frame()?;
        self.stream.reset(&window, painted);
        self.reflow.clear_pending();
        self.reflow.mark_reflowed(size, streaming);
        Ok(())
    }

    /// 仅高度变化时按光标位移修正锚点记账。
    ///
    /// 终端缩放会保持光标可见：变矮把内容上滚（顶部行进入 scrollback），
    /// 变高可能把 scrollback 行拉回屏幕。比较上次绘制的光标行与当前实际
    /// 光标行即可得到内容位移量，同步 origin 与 offscreen 记账。
    ///
    /// 参数:
    /// - `size`: 新终端尺寸
    ///
    /// 返回:
    /// - 是否成功重锚（失败时调用方退回全量重建）
    fn reanchor_for_height_change(&mut self, size: TerminalSize) -> bool {
        // 测试环境无真实终端，光标查询会阻塞
        if cfg!(test) {
            return false;
        }
        let Some(expected) = self.last_cursor_row else {
            return false;
        };
        let Ok((_, actual)) = crossterm::cursor::position() else {
            return false;
        };
        let delta = i32::from(expected) - i32::from(actual);
        if delta > 0 {
            // 1. 变矮：内容上移 delta 行；越过 origin 的部分已滚入 scrollback
            let delta = delta.min(i32::from(u16::MAX)) as u16;
            let absorbed = delta.min(self.viewport.origin_row());
            self.viewport.apply_terminal_scroll(delta);
            self.stream.note_scrolled(delta.saturating_sub(absorbed));
        } else if delta < 0 {
            // 2. 变高：scrollback 行被拉回，受管区起点下移且拉回行重新可修补
            let rise = (-delta).min(i32::from(u16::MAX)) as u16;
            self.viewport.shift_origin_down(rise, size);
            self.stream.note_unscrolled(usize::from(rise));
        }
        true
    }

    /// 外部程序写过终端后，从当前光标行重启受管区域。
    ///
    /// 已有输出全部视作 scrollback 保留原样，后续内容从光标处追加。
    fn restart_after_external(&mut self) -> Result<()> {
        self.desynced = false;
        // 光标查询要读终端的真实位置，未提交的绘制必须先送达
        self.commit_frame()?;
        let _paint = paint_lock();
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
}

pub(super) use event_loop::{process_stream_input, process_stream_tick};

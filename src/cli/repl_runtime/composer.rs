use super::composer_frame::ComposerFrame;
use super::viewport::TerminalSize;
use super::{QueuedSubmission, ReplRuntime, StreamComposerDraft};
use crate::agent::AgentMode;
use crate::cli::repl_chrome::ReplChrome;
use crate::cli::repl_clipboard::ReplClipboardBlockSpan;
use anyhow::Result;
use std::io::{self, Write};

impl ReplRuntime {
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
    pub(in crate::cli) fn update_composer(
        &mut self,
        chrome: &ReplChrome,
        input: &str,
        cursor: usize,
        is_pasted: bool,
        clipboard_blocks: Vec<ReplClipboardBlockSpan>,
        slash_selection: usize,
    ) -> Result<(u16, u16)> {
        let size = TerminalSize::current();
        let mut frame = ComposerFrame::new(
            chrome.clone(),
            input.to_string(),
            cursor,
            is_pasted,
            clipboard_blocks,
            slash_selection,
        );
        frame.set_panel_lines(self.bottom_panel_lines(usize::from(size.cols)));
        self.last_chrome = Some(chrome.clone());
        self.composer = Some(frame);
        // 终端尺寸已变化：旧 origin 上的 reserve 计算全部失效，
        // 直接触发 source-backed 重锚重放（replay 内部会绘制新 composer）
        if size != self.viewport.size() {
            self.reflow.observe(size, false);
            self.reflow.schedule_immediate();
            self.maybe_reflow_due(false)?;
            return Ok((self.viewport.composer_top(), self.viewport.composer_height()));
        }
        let previous_size = self.viewport.size();
        let previous_history = self.viewport.history_height();
        let composer_height = self.composer_height_for(size);
        // composer 行数超过内容下方空余时，先滚动终端腾出空间
        self.reserve_composer_rows(size, composer_height)?;
        self.viewport
            .update(size, composer_height, self.stream.on_screen());
        if self.needs_replay_after_layout(previous_size, previous_history) {
            self.reflow.schedule_immediate();
            self.maybe_reflow_due(false)?;
        }
        Ok((self.viewport.composer_top(), composer_height))
    }

    /// 组装当前沉底面板行（todo 快照 + 排队消息 + agent 面板）。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - 面板 ANSI 行；无内容时为空
    fn bottom_panel_lines(&self, cols: usize) -> Vec<String> {
        let queued: Vec<QueuedSubmission> = self.submission_queue.iter().cloned().collect();
        let agent_lines = self
            .agent_panel
            .panel_lines(&self.transcript.subagent_overview());
        super::bottom_panel::render_panel_lines(
            self.transcript.latest_todo_items(),
            &queued,
            &agent_lines,
            cols,
        )
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
    /// - 操作结果
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
    /// - 绘制结果
    pub(in crate::cli) fn draw_composer(&mut self, stdout: &mut io::Stdout) -> Result<()> {
        let Some(composer) = &self.composer else {
            return Ok(());
        };
        // 记录光标最终行：终端高度变化时以此探测内容位移
        let cursor_row = composer.draw(stdout, &self.viewport)?;
        self.last_cursor_row = Some(cursor_row);
        Ok(())
    }

    /// 返回运行期间输入草稿的可变引用。
    ///
    /// 返回:
    /// - 流式阶段 composer 草稿
    pub(in crate::cli) fn stream_draft_mut(&mut self) -> &mut StreamComposerDraft {
        &mut self.stream_draft
    }

    /// 返回运行期间输入草稿的引用。
    ///
    /// 返回:
    /// - 流式阶段 composer 草稿
    pub(in crate::cli) fn stream_draft(&self) -> &StreamComposerDraft {
        &self.stream_draft
    }

    /// 解析运行中输入框应使用的模式。
    ///
    /// 参数:
    /// - `fallback`: 无记录时的回退模式
    ///
    /// 返回:
    /// - 当前草稿或 chrome 模式
    pub(in crate::cli) fn stream_mode(&self, fallback: AgentMode) -> AgentMode {
        self.stream_draft
            .mode
            .or_else(|| self.composer.as_ref().map(|frame| frame.chrome().mode))
            .unwrap_or(fallback)
    }

    /// 将当前流式草稿入队，并清空草稿。
    ///
    /// 参数:
    /// - `fallback_mode`: 草稿未记录模式时使用的模式
    ///
    /// 返回:
    /// - 是否成功入队
    pub(in crate::cli) fn enqueue_stream_draft(
        &mut self,
        fallback_mode: AgentMode,
    ) -> Result<bool> {
        let text = self.stream_draft.text.trim().to_string();
        if text.is_empty() {
            return Ok(false);
        }
        let mode = self.stream_draft.mode.unwrap_or(fallback_mode);
        // 剪贴板附件随草稿一起入队，执行时还原为真实图片或长文本
        let clipboard = std::mem::take(&mut self.stream_draft.clipboard);
        self.submission_queue.push_back(QueuedSubmission {
            mode,
            text,
            clipboard,
        });
        self.stream_draft = StreamComposerDraft {
            mode: Some(mode),
            ..StreamComposerDraft::default()
        };
        // 队列内容由沉底面板常驻展示，无需再向历史区插入提示
        self.redraw_stream_composer()?;
        self.sync_transcript(false)?;
        Ok(true)
    }

    /// 取出全部排队提交。
    ///
    /// 返回:
    /// - 按先进先出顺序排列的提交列表
    pub(in crate::cli) fn take_submission_queue(&mut self) -> Vec<QueuedSubmission> {
        self.submission_queue.drain(..).collect()
    }


    /// 开始一轮流式输出前重置草稿，保留空 composer 供运行期间输入。
    ///
    /// 参数:
    /// - `mode`: 当前轮模式
    ///
    /// 返回:
    /// - 操作结果
    pub(in crate::cli) fn begin_stream_composer(&mut self, mode: AgentMode) -> Result<()> {
        self.stream_draft = StreamComposerDraft {
            mode: Some(mode),
            ..StreamComposerDraft::default()
        };
        self.redraw_stream_composer()
    }

    /// 按流式草稿重绘底部输入框。
    ///
    /// 返回:
    /// - 操作结果
    pub(in crate::cli) fn redraw_stream_composer(&mut self) -> Result<()> {
        let Some(mut chrome) = self
            .composer
            .as_ref()
            .map(|frame| frame.chrome().clone())
            .or_else(|| self.last_chrome.clone())
        else {
            return Ok(());
        };
        if let Some(mode) = self.stream_draft.mode {
            chrome.set_mode(mode);
        }
        let draft = self.stream_draft.clone();
        self.update_composer(
            &chrome,
            &draft.text,
            draft.cursor,
            draft.is_pasted,
            draft.clipboard.block_spans(&draft.text),
            draft.slash_selection,
        )?;
        let mut stdout = io::stdout();
        self.draw_composer(&mut stdout)?;
        stdout.flush()?;
        Ok(())
    }

    /// 结束 composer 绘制并释放底部 viewport 给历史输出。
    ///
    /// 返回:
    /// - 操作结果
    pub(in crate::cli) fn end_composer(&mut self) -> Result<()> {
        self.composer = None;
        let size = TerminalSize::current();
        // 尺寸已变化：交给 replay 重锚，不能先污染 viewport 记账
        if size != self.viewport.size() {
            self.reflow.observe(size, false);
            self.reflow.schedule_immediate();
            self.maybe_reflow_due(false)?;
            return Ok(());
        }
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
    /// - 尺寸变化或历史区域增高时返回 true
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
    pub(in crate::cli) fn observe_input_resize(&mut self, cols: u16, rows: u16) {
        self.observe_size(
            TerminalSize {
                cols: cols.max(1),
                rows: rows.max(1),
            },
            false,
        );
    }

    /// 处理流式阶段的 Resize 事件。
    ///
    /// 以 streaming 语义登记，保证本轮结束后触发补偿性整区重放。
    ///
    /// 参数:
    /// - `cols`: 新终端列数
    /// - `rows`: 新终端行数
    ///
    /// 返回:
    /// - 无
    pub(in crate::cli) fn observe_stream_resize(&mut self, cols: u16, rows: u16) {
        self.observe_size(
            TerminalSize {
                cols: cols.max(1),
                rows: rows.max(1),
            },
            true,
        );
    }
}

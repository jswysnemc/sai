use super::ReplRuntime;
use crate::agent::AgentMode;
use crate::cli::repl_windows_paste::WindowsPasteKey;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::io::IsTerminal;
use std::time::Duration;

impl ReplRuntime {
    /// 将主 TUI 重新定位到 transcript 最新输出。
    ///
    /// 参数:
    /// - `streaming`: 当前是否处于模型流式输出阶段
    ///
    /// 返回:
    /// - transcript 与 composer 重绘结果
    pub(in crate::cli) fn jump_to_output_bottom(&mut self, streaming: bool) -> Result<()> {
        self.replay(streaming)
    }

    /// 保存模型运行期间收到的普通终端输入。
    ///
    /// 参数:
    /// - `event`: 待交给下一次输入框处理的事件
    ///
    /// 返回:
    /// - 无
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::cli) fn queue_input_event(&mut self, event: Event) {
        self.pending_input_events.push_back(event);
    }

    /// 读取模型运行期间保存的最早终端输入。
    ///
    /// 返回:
    /// - 下一条待处理事件
    pub(in crate::cli) fn pop_input_event(&mut self) -> Option<Event> {
        self.pending_input_events.pop_front()
    }

    /// 切换最近命令输出或思考段落的展开状态并重绘 TUI。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否找到可切换的命令输出
    pub(in crate::cli) fn toggle_command_output(&mut self) -> Result<bool> {
        // 1. 测试：只切换 transcript 状态，避免 replay 触碰真实终端
        if cfg!(test) {
            return Ok(self.transcript.toggle_latest_command_output());
        }
        // 2. 交互终端：备用屏 pager 展示全部折叠块，左右切换
        if std::io::stdout().is_terminal() && std::io::stdin().is_terminal() {
            let blocks = self.transcript.expandable_blocks();
            if blocks.is_empty() {
                return Ok(false);
            }
            let start = blocks.len().saturating_sub(1);
            super::super::repl_pager::open_blocks_pager(&blocks, start)?;
            // 备用屏返回后强制重同步 viewport 与 composer，避免输入框错位
            self.resync_after_overlay()?;
            return Ok(true);
        }
        // 3. 非交互：内联展开/折叠
        if !self.transcript.toggle_latest_command_output() {
            return Ok(false);
        }
        self.replay(false)?;
        self.redraw_stream_composer()?;
        Ok(true)
    }

    /// 切换当前实时思考块的展开状态并立即重绘。
    ///
    /// 返回:
    /// - 找到实时思考块时返回 true
    pub(in crate::cli) fn toggle_live_reasoning(&mut self) -> Result<bool> {
        // 流式思考优先；没有正在流的思考时，退而展开最近一个折叠的 diff
        if !self.transcript.toggle_live_reasoning() && !self.transcript.toggle_last_diff() {
            return Ok(false);
        }
        self.sync_transcript(true)?;
        self.redraw_stream_composer()?;
        Ok(true)
    }
}

/// 在流式事件循环 tick 中采样尺寸并执行到期 reflow 与 live 刷新。
///
/// 参数:
/// - `runtime`: 当前 REPL 运行期
///
/// 返回:
/// - 处理是否成功
pub(crate) fn process_stream_tick(runtime: &mut ReplRuntime) -> Result<()> {
    runtime.observe_terminal_size(true)?;
    runtime.maybe_reflow_due(true)?;
    runtime.tick_live()?;
    if runtime.tick_subagents()? {
        // 子 agent 状态变化会改变底部 agent 面板的计数与状态行
        runtime.redraw_stream_composer()?;
    }
    Ok(())
}

/// 处理模型运行期间的非阻塞终端事件。
///
/// Agent 工作时允许编辑底部输入框：Tab 入队，Shift+Tab 切换模式，Ctrl+C 中断。
///
/// 参数:
/// - `runtime`: 当前 REPL 运行期
///
/// 返回:
/// - 收到 Ctrl+C 时返回 true
pub(crate) fn process_stream_input(runtime: &mut ReplRuntime) -> Result<bool> {
    while event::poll(Duration::ZERO)? {
        let input = event::read()?;
        match input {
            Event::Resize(cols, rows) => {
                // 流式期间只登记（streaming 语义），由 25ms tick 的 debounce
                // 到期重放统一重锚；立即重绘会用旧 origin 画错位置
                runtime.observe_stream_resize(cols, rows);
            }
            Event::Paste(text) => {
                let text = strip_control_sequences(&text);
                let draft = runtime.stream_draft_mut();
                draft.windows_paste.reset();
                draft
                    .clipboard
                    .paste_text_into_input(&mut draft.text, &mut draft.cursor, text);
                draft.is_pasted = true;
                draft.slash_selection = 0;
                runtime.redraw_stream_composer()?;
            }
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let replay_key = windows_paste_key(key.code, key.modifiers);
                if replay_key.is_some_and(|candidate| {
                    runtime
                        .stream_draft_mut()
                        .windows_paste
                        .consume_key(candidate)
                }) {
                    continue;
                }
                if super::super::repl_transcript_pager::is_jump_to_output_bottom_key(
                    key.code,
                    key.modifiers,
                ) {
                    runtime.jump_to_output_bottom(true)?;
                    continue;
                }
                let ctrl_o = matches!(key.code, KeyCode::Char('o'))
                    && key.modifiers.contains(KeyModifiers::CONTROL);
                if ctrl_o && runtime.toggle_live_reasoning()? {
                    continue;
                }
                if matches!(key.code, KeyCode::Char('t'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                    && runtime.toggle_todo_panel_compact()
                {
                    runtime.redraw_stream_composer()?;
                    continue;
                }
                // Ctrl+Z 撤回队尾、Ctrl+Y 清空队列：排错入队的消息不必等本轮结束
                if matches!(key.code, KeyCode::Char('z'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    if runtime.undo_last_queued()?.is_none() {
                        runtime.record_meta(
                            crate::i18n::text(
                                "nothing to undo: the queue is empty or the input box is not empty",
                                "无可撤回项：队列为空，或输入框还有内容",
                            )
                            .to_string(),
                        )?;
                    }
                    continue;
                }
                if matches!(key.code, KeyCode::Char('y'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    let count = runtime.clear_queued()?;
                    if count > 0 {
                        runtime.record_meta(if crate::i18n::is_zh() {
                            format!("已清空 {count} 条排队项")
                        } else {
                            format!("cleared {count} queued items")
                        })?;
                    }
                    continue;
                }
                if ctrl_o || matches!(key.code, KeyCode::PageUp) {
                    // 1. 流式期间不打开阻塞式浏览面板：pager 会同步占住事件循环，
                    //    模型流无人读取、工具子进程管道写满后挂起
                    runtime.record_meta(
                        crate::i18n::text(
                            "transcript pager is available after this turn finishes",
                            "会话浏览面板需等本轮结束后再打开",
                        )
                        .to_string(),
                    )?;
                    continue;
                }
                if matches!(key.code, KeyCode::Char('c'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    // 2. Ctrl+C 中断当前轮
                    return Ok(true);
                }
                if matches!(key.code, KeyCode::Enter) && key.modifiers.is_empty() {
                    let paste = {
                        let draft = runtime.stream_draft_mut();
                        let input_before_cursor: String =
                            draft.text.chars().take(draft.cursor).collect();
                        draft.windows_paste.begin_from_system_clipboard(
                            &input_before_cursor,
                            std::time::Instant::now(),
                        )
                    };
                    if let Some(paste) = paste {
                        let draft = runtime.stream_draft_mut();
                        draft.clipboard.replace_recent_text_with_paste(
                            &mut draft.text,
                            &mut draft.cursor,
                            paste.prefix_chars,
                            paste.text,
                        );
                        draft.is_pasted = true;
                        draft.slash_selection = 0;
                        runtime.redraw_stream_composer()?;
                        continue;
                    }
                }
                // 3. 底部 agent 面板优先消费按键（↓ 进入、↑↓ 选择、Enter 应用）
                if runtime.handle_agent_panel_key(key.code)? {
                    continue;
                }
                // 4. 其他键写入运行中输入框
                handle_stream_key(runtime, key.code, key.modifiers)?;
            }
            Event::Key(_) => {}
            _ => {}
        }
    }
    Ok(false)
}

/// 将单个按键应用到运行中 composer 草稿。
///
/// 参数:
/// - `runtime`: REPL 运行期
/// - `code`: 键码
/// - `modifiers`: 修饰键
///
/// 返回:
/// - 是否成功
fn handle_stream_key(
    runtime: &mut ReplRuntime,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<()> {
    match code {
        KeyCode::BackTab => {
            // 部分终端把 Shift+Tab 发成 BackTab：立即生效
            let current = runtime.stream_mode(AgentMode::Yolo);
            let next = cycle_mode(current);
            runtime.stream_draft_mut().mode = Some(next);
            let _ = runtime.apply_stream_mode_live(AgentMode::Yolo);
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Tab if modifiers.contains(KeyModifiers::SHIFT) => {
            // Shift+Tab：立即切换权限模式（当前轮生效）
            let current = runtime.stream_mode(AgentMode::Yolo);
            let next = cycle_mode(current);
            runtime.stream_draft_mut().mode = Some(next);
            let _ = runtime.apply_stream_mode_live(AgentMode::Yolo);
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Tab => {
            // Tab：先补全斜杠命令，无补全时才把草稿入队
            if runtime.stream_draft().text.starts_with('/') {
                let completed = crate::cli::repl_commands::complete_repl_command(
                    &runtime.stream_draft().text,
                );
                if let Some(completed) = completed {
                    let draft = runtime.stream_draft_mut();
                    draft.text = completed.to_string();
                    draft.cursor = draft.text.chars().count();
                    draft.slash_selection = 0;
                    runtime.redraw_stream_composer()?;
                    return Ok(());
                }
            }
            let mode = runtime.stream_mode(AgentMode::Yolo);
            let _ = runtime.enqueue_stream_draft(mode)?;
        }
        KeyCode::Up => {
            // 斜杠面板可见时上下键移动选中项，否则不干预运行中输入
            let suggestions =
                crate::cli::repl_commands::visible_repl_command_suggestions(
                    &runtime.stream_draft().text,
                );
            if !suggestions.is_empty() {
                let draft = runtime.stream_draft_mut();
                draft.slash_selection = (draft.slash_selection % suggestions.len())
                    .checked_sub(1)
                    .unwrap_or(suggestions.len().saturating_sub(1));
                runtime.redraw_stream_composer()?;
            }
        }
        KeyCode::Down => {
            let suggestions =
                crate::cli::repl_commands::visible_repl_command_suggestions(
                    &runtime.stream_draft().text,
                );
            if !suggestions.is_empty() {
                let draft = runtime.stream_draft_mut();
                draft.slash_selection = (draft.slash_selection + 1) % suggestions.len();
                runtime.redraw_stream_composer()?;
            }
        }
        KeyCode::Enter => {
            if modifiers.contains(KeyModifiers::SHIFT) {
                let draft = runtime.stream_draft_mut();
                insert_char(&mut draft.text, &mut draft.cursor, '\n');
                draft.slash_selection = 0;
                draft.is_pasted = false;
                runtime.redraw_stream_composer()?;
            } else {
                // Enter：面板可见时先落选中命令，否则 /model 会被当普通文本发出
                let suggestions =
                    crate::cli::repl_commands::visible_repl_command_suggestions(
                        &runtime.stream_draft().text,
                    );
                if !suggestions.is_empty() {
                    let draft = runtime.stream_draft_mut();
                    let selected = suggestions
                        [draft.slash_selection.min(suggestions.len().saturating_sub(1))];
                    draft.text = selected.command.to_string();
                    draft.cursor = draft.text.chars().count();
                    draft.slash_selection = 0;
                }
                let mode = runtime.stream_mode(AgentMode::Yolo);
                let _ = runtime.enqueue_stream_draft(mode)?;
            }
        }
        KeyCode::Backspace => {
            let draft = runtime.stream_draft_mut();
            if !draft
                .clipboard
                .remove_block_before_cursor(&mut draft.text, &mut draft.cursor)
                && draft.cursor > 0
            {
                remove_char_before(&mut draft.text, &mut draft.cursor);
            }
            draft.slash_selection = 0;
            draft.is_pasted = false;
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Delete => {
            let draft = runtime.stream_draft_mut();
            if !draft
                .clipboard
                .remove_block_at_cursor(&mut draft.text, draft.cursor)
                && draft.cursor < draft.text.chars().count()
            {
                remove_char_at(&mut draft.text, draft.cursor);
            }
            draft.slash_selection = 0;
            draft.is_pasted = false;
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Left => {
            // 剪贴板占位块整体跳过，保持与删除一致的原子性
            let draft = runtime.stream_draft_mut();
            draft.cursor = draft.clipboard.cursor_left(&draft.text, draft.cursor);
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Right => {
            let draft = runtime.stream_draft_mut();
            draft.cursor = draft.clipboard.cursor_right(&draft.text, draft.cursor);
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Home => {
            runtime.stream_draft_mut().cursor = 0;
            runtime.redraw_stream_composer()?;
        }
        KeyCode::End => {
            let draft = runtime.stream_draft_mut();
            draft.cursor = draft.text.chars().count();
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Char('v') if modifiers.contains(KeyModifiers::CONTROL) => {
            let draft = runtime.stream_draft_mut();
            draft.windows_paste.reset();
            draft.is_pasted = draft
                .clipboard
                .paste_into_input(&mut draft.text, &mut draft.cursor)?;
            draft.slash_selection = 0;
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Char(ch)
            if !modifiers.contains(KeyModifiers::CONTROL)
                && !modifiers.contains(KeyModifiers::ALT)
                && !is_control_char(ch) =>
        {
            let draft = runtime.stream_draft_mut();
            draft
                .windows_paste
                .record_char(ch, std::time::Instant::now());
            insert_char(&mut draft.text, &mut draft.cursor, ch);
            draft.slash_selection = 0;
            draft.is_pasted = false;
            runtime.redraw_stream_composer()?;
        }
        _ => {}
    }
    Ok(())
}

/// 把终端按键转换为 Windows 粘贴回放可比较的字符。
///
/// 参数:
/// - `code`: 终端键码
/// - `modifiers`: 终端修饰键
///
/// 返回:
/// - 可参与剪贴板回放匹配的按键
fn windows_paste_key(code: KeyCode, modifiers: KeyModifiers) -> Option<WindowsPasteKey> {
    match code {
        KeyCode::Char(ch)
            if !modifiers.contains(KeyModifiers::CONTROL)
                && !modifiers.contains(KeyModifiers::ALT) =>
        {
            Some(WindowsPasteKey::Char(ch))
        }
        KeyCode::Enter if modifiers.is_empty() => Some(WindowsPasteKey::Enter),
        KeyCode::Tab if modifiers.is_empty() => Some(WindowsPasteKey::Tab),
        _ => None,
    }
}

/// 循环切换 Agent 模式。
fn cycle_mode(mode: AgentMode) -> AgentMode {
    match mode {
        AgentMode::Yolo => AgentMode::Audited,
        AgentMode::Audited => AgentMode::AutoAudit,
        AgentMode::AutoAudit => AgentMode::Plan,
        AgentMode::Plan => AgentMode::Yolo,
    }
}

/// 在光标处插入字符。
fn insert_char(input: &mut String, cursor: &mut usize, ch: char) {
    let byte = input
        .char_indices()
        .nth(*cursor)
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    input.insert(byte, ch);
    *cursor += 1;
}

/// 删除光标前一个字符。
fn remove_char_before(input: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let start = input
        .char_indices()
        .nth(*cursor - 1)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let end = input
        .char_indices()
        .nth(*cursor)
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    input.replace_range(start..end, "");
    *cursor -= 1;
}

/// 删除光标处字符。
fn remove_char_at(input: &mut String, cursor: usize) {
    let Some((start, ch)) = input.char_indices().nth(cursor) else {
        return;
    };
    let end = start + ch.len_utf8();
    input.replace_range(start..end, "");
}

/// 判断是否为不应写入输入框的控制字符。
fn is_control_char(ch: char) -> bool {
    ch.is_control() && ch != '\n' && ch != '\t'
}

/// 去掉终端控制序列，避免粘贴污染输入框。
fn strip_control_sequences(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            // 跳过 CSI / 简单 ESC 序列
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        if ch == '\r' {
            continue;
        }
        out.push(ch);
    }
    out
}

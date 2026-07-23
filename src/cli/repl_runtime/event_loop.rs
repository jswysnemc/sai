use super::ReplRuntime;
use crate::agent::AgentMode;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::io::IsTerminal;
use std::time::Duration;

impl ReplRuntime {
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
    runtime.tick_subagents().map(|_| ())
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
                runtime.observe_input_resize(cols, rows);
                runtime.redraw_stream_composer()?;
            }
            Event::Paste(text) => {
                let text = strip_control_sequences(&text);
                let draft = runtime.stream_draft_mut();
                draft
                    .clipboard
                    .paste_text_into_input(&mut draft.text, &mut draft.cursor, text);
                draft.is_pasted = true;
                draft.slash_selection = 0;
                runtime.redraw_stream_composer()?;
            }
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                if matches!(key.code, KeyCode::Char('o'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    // 1. 允许展开或收起最近命令输出 / 思考段落
                    runtime.toggle_command_output()?;
                    continue;
                }
                if matches!(key.code, KeyCode::Char('c'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    // 2. Ctrl+C 中断当前轮
                    return Ok(true);
                }
                // 3. 其他键写入运行中输入框
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
            // Tab：当前草稿入队，等待本轮结束后执行
            let mode = runtime.stream_mode(AgentMode::Yolo);
            let _ = runtime.enqueue_stream_draft(mode)?;
        }
        KeyCode::Enter => {
            if modifiers.contains(KeyModifiers::SHIFT) {
                let draft = runtime.stream_draft_mut();
                insert_char(&mut draft.text, &mut draft.cursor, '\n');
                draft.slash_selection = 0;
                draft.is_pasted = false;
                runtime.redraw_stream_composer()?;
            } else {
                // Enter：入队，等待本轮结束后执行
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
            let draft = runtime.stream_draft_mut();
            draft.cursor = draft.cursor.saturating_sub(1);
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Right => {
            let draft = runtime.stream_draft_mut();
            draft.cursor = (draft.cursor + 1).min(draft.text.chars().count());
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
            draft.is_pasted = draft
                .clipboard
                .paste_into_input(&mut draft.text, &mut draft.cursor)?;
            draft.slash_selection = 0;
            runtime.redraw_stream_composer()?;
        }
        KeyCode::Char(ch)
            if !modifiers.contains(KeyModifiers::CONTROL)
                && !modifiers.contains(KeyModifiers::ALT) =>
        {
            if !is_control_char(ch) {
                let draft = runtime.stream_draft_mut();
                insert_char(&mut draft.text, &mut draft.cursor, ch);
                draft.slash_selection = 0;
                draft.is_pasted = false;
                runtime.redraw_stream_composer()?;
            }
        }
        _ => {}
    }
    Ok(())
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

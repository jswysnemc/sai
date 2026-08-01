use super::repl_chrome::ReplChrome;
use super::repl_clipboard::ReplClipboardState;
use super::repl_external_events::ReplExternalEvents;
use super::repl_runtime::ReplRuntime;
use super::*;
use crate::agent::ExternalEventWake;

const EXTERNAL_EVENT_INPUT_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(super) struct ReplInputSubmission {
    pub(super) mode: AgentMode,
    pub(super) raw_input: String,
    pub(super) chat_input: clipboard::ClipboardChatInput,
}

/// 输入框产生的下一项工作。
pub(super) enum ReplInputEvent {
    User(ReplInputSubmission),
    Automatic {
        mode: AgentMode,
        wake: ExternalEventWake,
        draft: ReplInputDraft,
    },
}

/// 自动唤醒期间暂存的输入文本与剪贴板附件。
pub(super) struct ReplInputDraft {
    pub(super) text: String,
    pub(super) clipboard_state: ReplClipboardState,
}

/// 清理输入区并恢复终端状态，干净结束本次 REPL 输入循环。
///
/// 参数:
/// - `stdout`: 终端输出句柄
/// - `input_row`: 输入区起始行
/// - `rendered_rows`: 输入区已经渲染的行数
/// - `runtime`: REPL 终端运行期
/// - `terminal_guard`: 终端输入模式守卫
///
/// 返回:
/// - 清理与终端恢复是否成功
fn finish_repl_input(
    stdout: &mut io::Stdout,
    input_row: u16,
    rendered_rows: u16,
    runtime: &mut ReplRuntime,
    terminal_guard: &mut super::terminal_restore::TerminalInputGuard,
) -> Result<()> {
    // 1. 清除 composer 已经绘制的全部终端行
    clear_repl_input(stdout, input_row, rendered_rows)?;
    // 2. 释放 composer 占用空间，使 transcript 尾部保持完整
    runtime.end_composer()?;
    // 3. 恢复 raw mode、粘贴模式与键盘增强协议
    terminal_guard.finish(stdout)
}

/// 读取、编辑并提交 REPL 输入，同时在 debounce 到期时处理 resize 重放。
///
/// 参数:
/// - `mode`: 当前 REPL 模式
/// - `prefill`: 待编辑的预填输入
/// - `prefill_clipboard`: 预填输入关联的剪贴板附件
/// - `history`: 输入历史记录
/// - `chrome`: 可变的输入区 chrome 状态
/// - `runtime`: REPL 终端运行期
/// - `external_events`: 后台完成事件监听器
///
/// 返回:
/// - 用户提交或自动唤醒事件，退出时返回空值
pub(super) fn read_repl_input(
    mut mode: AgentMode,
    prefill: Option<String>,
    prefill_clipboard: Option<ReplClipboardState>,
    history: &[String],
    chrome: &mut ReplChrome,
    runtime: &mut ReplRuntime,
    external_events: &mut ReplExternalEvents,
) -> Result<Option<ReplInputEvent>> {
    let mut stdout = io::stdout();
    let mut input = strip_terminal_control_sequences(&prefill.unwrap_or_default());
    let mut cursor = input.chars().count();
    let mut slash_selection = 0usize;
    let mut history_index = history.len();
    let mut history_clean_index = None::<usize>;
    let mut clipboard_state = prefill_clipboard.unwrap_or_default();
    let mut last_escape = None::<Instant>;
    let mut last_ctrl_c = None::<Instant>;
    // 输入框由 composer 绝对定位绘制；这里禁止直接向终端写换行，
    // 否则屏幕底部会触发受管模型感知不到的滚动，吞掉上方内容
    let mut terminal_guard =
        super::terminal_restore::TerminalInputGuard::enable(&mut stdout, true)?;
    let (_, mut input_row) = cursor::position()?;
    let mut rendered_rows = 0u16;
    let mut is_pasted = false;
    macro_rules! redraw_input {
        () => {
            render_repl_input(
                &mut stdout,
                &mut input_row,
                &mut rendered_rows,
                chrome,
                &input,
                cursor,
                is_pasted,
                &clipboard_state,
                slash_selection,
                runtime,
            )
        };
    }
    redraw_input!()?;
    loop {
        if let Some(wake) = external_events.take_ready() {
            terminal_guard.finish(&mut stdout)?;
            return Ok(Some(ReplInputEvent::Automatic {
                mode,
                wake: wake?,
                draft: ReplInputDraft {
                    text: input,
                    clipboard_state,
                },
            }));
        }
        let queued_event = runtime.pop_input_event();
        if queued_event.is_none() {
            let wait = match (runtime.pending_wait(), external_events.is_armed()) {
                (Some(wait), true) => Some(wait.min(EXTERNAL_EVENT_INPUT_POLL_INTERVAL)),
                (Some(wait), false) => Some(wait),
                (None, true) => Some(EXTERNAL_EVENT_INPUT_POLL_INTERVAL),
                (None, false) => None,
            };
            // 空输入时短轮询，便于灰色操作提示按时切换
            let wait = if input.is_empty() {
                Some(
                    wait.unwrap_or(std::time::Duration::from_secs(1))
                        .min(std::time::Duration::from_secs(1)),
                )
            } else {
                wait
            };
            if let Some(wait) = wait {
                if !event::poll(wait)? {
                    if runtime.process_idle_tick()? || input.is_empty() {
                        input_row = 0;
                        rendered_rows = 0;
                        redraw_input!()?;
                    }
                    continue;
                }
            }
        }
        let event = queued_event.map(Ok).unwrap_or_else(event::read)?;
        match event {
            Event::Resize(cols, rows) => runtime.observe_input_resize(cols, rows),
            Event::Paste(text) => {
                let text = strip_terminal_control_sequences(&text);
                clipboard_state.paste_text_into_input(&mut input, &mut cursor, text);
                slash_selection = 0;
                history_clean_index = None;
                is_pasted = true;
                redraw_input!()?;
            }
            Event::Key(KeyEvent {
                code,
                modifiers,
                kind,
                ..
            }) => {
                // 只处理按下与长按重复事件，避免重新进入原始模式后的释放事件覆盖新输入
                if kind == KeyEventKind::Release {
                    continue;
                }
                if code != KeyCode::Esc {
                    last_escape = None;
                }
                if !matches!(code, KeyCode::Char('c')) || !modifiers.contains(KeyModifiers::CONTROL)
                {
                    last_ctrl_c = None;
                }
                if super::repl_transcript_pager::is_jump_to_output_bottom_key(code, modifiers) {
                    runtime.jump_to_output_bottom(false)?;
                    input_row = 0;
                    rendered_rows = 0;
                    redraw_input!()?;
                    continue;
                }
                // 底部 agent 面板：面板焦点态全键拦截；空输入时 ↓ 进入面板
                if (runtime.agent_panel_active() || input.is_empty())
                    && runtime.handle_agent_panel_idle_key(code)?
                {
                    redraw_input!()?;
                    continue;
                }
                // PageUp：进入备用屏 transcript 浏览（进度自管理，回看不丢位置）
                if code == KeyCode::PageUp {
                    super::repl_transcript_pager::open_transcript_pager(runtime)?;
                    runtime.resync_after_overlay()?;
                    input_row = 0;
                    rendered_rows = 0;
                    redraw_input!()?;
                    continue;
                }
                match code {
                    KeyCode::BackTab => {
                        // 部分终端把 Shift+Tab 发成 BackTab
                        mode = cycle_repl_mode(mode);
                        chrome.set_mode(mode);
                        is_pasted = false;
                        redraw_input!()?;
                    }
                    KeyCode::Tab if modifiers.contains(KeyModifiers::SHIFT) => {
                        // Shift+Tab：循环权限模式
                        mode = cycle_repl_mode(mode);
                        chrome.set_mode(mode);
                        is_pasted = false;
                        redraw_input!()?;
                    }
                    KeyCode::Tab => {
                        // Tab：斜杠命令补全；空闲时也可提示队列能力（无文本则忽略）
                        if input.starts_with('/') {
                            if let Some(completed) = complete_repl_command(&input) {
                                input = completed.to_string();
                                cursor = input.chars().count();
                                history_clean_index = None;
                            }
                            slash_selection = 0;
                        }
                        is_pasted = false;
                        redraw_input!()?;
                    }
                    KeyCode::Esc => {
                        let now = Instant::now();
                        if last_escape.is_some_and(|previous| {
                            now.duration_since(previous) <= REPL_ESC_CLEAR_WINDOW
                        }) {
                            input.clear();
                            cursor = 0;
                            slash_selection = 0;
                            clipboard_state.clear();
                            history_clean_index = None;
                            is_pasted = false;
                            last_escape = None;
                            redraw_input!()?;
                        } else {
                            last_escape = Some(now);
                        }
                    }
                    KeyCode::Left => {
                        // 输入为空时左移光标没有意义，改为打开会话树
                        if input.is_empty() {
                            terminal_guard.finish(&mut stdout)?;
                            return Ok(Some(ReplInputEvent::User(ReplInputSubmission {
                                mode,
                                raw_input: "/tree".to_string(),
                                chat_input: clipboard_state.to_chat_input("/tree"),
                            })));
                        }
                        // 剪贴板占位块整体跳过，保持与删除一致的原子性
                        cursor = clipboard_state.cursor_left(&input, cursor);
                        redraw_input!()?;
                    }
                    KeyCode::Right => {
                        cursor = clipboard_state.cursor_right(&input, cursor);
                        redraw_input!()?;
                    }
                    KeyCode::Home => {
                        cursor = 0;
                        redraw_input!()?;
                    }
                    KeyCode::End => {
                        cursor = input.chars().count();
                        redraw_input!()?;
                    }
                    KeyCode::Up => {
                        let suggestions = visible_repl_command_suggestions(&input);
                        if !suggestions.is_empty() {
                            slash_selection = (slash_selection % suggestions.len())
                                .checked_sub(1)
                                .unwrap_or(suggestions.len().saturating_sub(1));
                            redraw_input!()?;
                        } else {
                            let plain_prefix = String::new();
                            if let Some(next_cursor) = move_cursor_up_by_visual_row(
                                &plain_prefix,
                                &input,
                                cursor,
                                terminal_cols(),
                            ) {
                                cursor = next_cursor;
                                redraw_input!()?;
                            } else if repl_should_browse_history(
                                &input,
                                history,
                                history_clean_index,
                            ) {
                                if input.is_empty() {
                                    history_index = history.len();
                                }
                                history_index = history_index.saturating_sub(1);
                                input = history.get(history_index).cloned().unwrap_or_default();
                                cursor = input.chars().count();
                                history_clean_index = Some(history_index);
                                slash_selection = 0;
                                clipboard_state.clear();
                                is_pasted = false;
                                redraw_input!()?;
                            }
                        }
                    }
                    KeyCode::Down => {
                        let suggestions = visible_repl_command_suggestions(&input);
                        if !suggestions.is_empty() {
                            slash_selection = (slash_selection + 1) % suggestions.len();
                        } else {
                            let plain_prefix = String::new();
                            if let Some(next_cursor) = move_cursor_down_by_visual_row(
                                &plain_prefix,
                                &input,
                                cursor,
                                terminal_cols(),
                            ) {
                                cursor = next_cursor;
                            } else if repl_history_is_clean(&input, history, history_clean_index)
                                && history_index + 1 < history.len()
                            {
                                history_index += 1;
                                input = history.get(history_index).cloned().unwrap_or_default();
                                cursor = input.chars().count();
                                history_clean_index = Some(history_index);
                                slash_selection = 0;
                                clipboard_state.clear();
                                is_pasted = false;
                            } else if repl_history_is_clean(&input, history, history_clean_index)
                                && history_index < history.len()
                            {
                                history_index = history.len();
                                input.clear();
                                cursor = input.chars().count();
                                history_clean_index = None;
                                slash_selection = 0;
                                clipboard_state.clear();
                                is_pasted = false;
                            }
                        }
                        redraw_input!()?;
                    }
                    KeyCode::Enter if modifiers.contains(KeyModifiers::SHIFT) => {
                        insert_newline_at_cursor(&mut input, &mut cursor);
                        slash_selection = 0;
                        history_clean_index = None;
                        is_pasted = false;
                        redraw_input!()?;
                    }
                    KeyCode::Enter => {
                        let suggestions = visible_repl_command_suggestions(&input);
                        if let Some(selected) = suggestions
                            .get(slash_selection.min(suggestions.len().saturating_sub(1)))
                        {
                            input = selected.command.to_string();
                            slash_selection = 0;
                        }
                        input = strip_terminal_control_sequences(&input);
                        if super::repl_commands::is_repl_exit_command(&input) {
                            finish_repl_input(
                                &mut stdout,
                                input_row,
                                rendered_rows,
                                runtime,
                                &mut terminal_guard,
                            )?;
                            return Ok(None);
                        }
                        let chat_input = clipboard_state.to_chat_input(&input);
                        let raw_input = std::mem::take(&mut input);
                        cursor = 0;
                        clipboard_state.clear();
                        is_pasted = false;
                        // 1. 提交后立即显示空 composer，流式输出始终插入其上方
                        redraw_input!()?;
                        terminal_guard.finish(&mut stdout)?;
                        return Ok(Some(ReplInputEvent::User(ReplInputSubmission {
                            mode,
                            raw_input,
                            chat_input,
                        })));
                    }
                    KeyCode::Char('j') if modifiers.contains(KeyModifiers::CONTROL) => {
                        insert_newline_at_cursor(&mut input, &mut cursor);
                        slash_selection = 0;
                        history_clean_index = None;
                        is_pasted = false;
                        redraw_input!()?;
                    }
                    KeyCode::Char('v') if modifiers.contains(KeyModifiers::CONTROL) => {
                        is_pasted = clipboard_state.paste_into_input(&mut input, &mut cursor)?;
                        slash_selection = 0;
                        history_clean_index = None;
                        redraw_input!()?;
                    }
                    KeyCode::Char('g') if modifiers.contains(KeyModifiers::CONTROL) => {
                        clear_repl_input(&mut stdout, input_row, rendered_rows)?;
                        runtime.end_composer()?;
                        terminal_guard.finish(&mut stdout)?;
                        // 长文本占位块展开为正文进入编辑器，图片占位块先摘出，退出后按锚点复位
                        let editor_buffer = super::repl_editor_buffer::prepare_editor_buffer(
                            &input,
                            &clipboard_state,
                        );
                        let had_text_blocks = clipboard_state.has_text_blocks(&input);
                        match edit_input_buffer(&editor_buffer.text) {
                            Ok(edited) => {
                                let cleaned = strip_terminal_control_sequences(&edited);
                                input = super::repl_editor_buffer::restore_editor_buffer(
                                    &editor_buffer,
                                    &cleaned,
                                );
                                cursor = input.chars().count();
                                slash_selection = 0;
                                history_clean_index = None;
                                // 长文本已在编辑器里展开成正文，对应的占位块不再有承载对象；
                                // 只有图片占位块需要保留登记，否则提交时取不到图片数据
                                if had_text_blocks {
                                    clipboard_state.forget_text_blocks();
                                }
                            }
                            Err(err) => {
                                eprintln!("{err}");
                            }
                        }
                        terminal_guard =
                            super::terminal_restore::TerminalInputGuard::enable(&mut stdout, true)?;
                        input_row = 0;
                        rendered_rows = 0;
                        is_pasted = false;
                        redraw_input!()?;
                    }
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        let now = Instant::now();
                        if last_ctrl_c.is_some_and(|previous| {
                            now.duration_since(previous) <= REPL_CTRL_C_EXIT_WINDOW
                        }) {
                            finish_repl_input(
                                &mut stdout,
                                input_row,
                                rendered_rows,
                                runtime,
                                &mut terminal_guard,
                            )?;
                            return Ok(None);
                        }
                        last_ctrl_c = Some(now);
                        input.clear();
                        cursor = 0;
                        slash_selection = 0;
                        clipboard_state.clear();
                        history_clean_index = None;
                        is_pasted = false;
                        redraw_input!()?;
                    }
                    KeyCode::Char('d')
                        if modifiers.contains(KeyModifiers::CONTROL) && input.is_empty() =>
                    {
                        finish_repl_input(
                            &mut stdout,
                            input_row,
                            rendered_rows,
                            runtime,
                            &mut terminal_guard,
                        )?;
                        return Ok(None);
                    }
                    KeyCode::Char('l') if modifiers.contains(KeyModifiers::CONTROL) => {
                        runtime.redraw()?;
                        input_row = 0;
                        rendered_rows = 0;
                        redraw_input!()?;
                    }
                    KeyCode::Char('o') if modifiers.contains(KeyModifiers::CONTROL) => {
                        if runtime.toggle_command_output()? {
                            // pager 返回后重新打开增强输入，并重绘输入框
                            input_row = 0;
                            rendered_rows = 0;
                            redraw_input!()?;
                        }
                    }
                    KeyCode::Char('w') if modifiers.contains(KeyModifiers::CONTROL) => {
                        remove_word_before_cursor(&mut input, &mut cursor);
                        slash_selection = 0;
                        history_clean_index = None;
                        is_pasted = false;
                        redraw_input!()?;
                    }
                    KeyCode::Backspace => {
                        let changed = if clipboard_state
                            .remove_block_before_cursor(&mut input, &mut cursor)
                        {
                            // 已删除完整剪贴板占位块
                            true
                        } else if cursor > 0 {
                            remove_char_before_cursor(&mut input, &mut cursor);
                            true
                        } else {
                            false
                        };
                        slash_selection = 0;
                        if changed {
                            history_clean_index = None;
                        }
                        is_pasted = false;
                        redraw_input!()?;
                    }
                    KeyCode::Delete => {
                        let changed = if clipboard_state.remove_block_at_cursor(&mut input, cursor)
                        {
                            true
                        } else if cursor < input.chars().count() {
                            remove_char_at_cursor(&mut input, cursor);
                            true
                        } else {
                            false
                        };
                        slash_selection = 0;
                        if changed {
                            history_clean_index = None;
                        }
                        is_pasted = false;
                        redraw_input!()?;
                    }
                    KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                        if !is_disallowed_control_char(ch) {
                            insert_char_at_cursor(&mut input, &mut cursor, ch);
                            history_clean_index = None;
                        }
                        slash_selection = 0;
                        is_pasted = false;
                        redraw_input!()?;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

/// 判断当前输入是否仍与选中的历史记录一致。
///
/// 参数:
/// - `input`: 当前输入
/// - `history`: 历史记录
/// - `history_clean_index`: 最近选中的历史下标
///
/// 返回:
/// - 未修改选中历史时返回 true
pub(super) fn repl_history_is_clean(
    input: &str,
    history: &[String],
    history_clean_index: Option<usize>,
) -> bool {
    history_clean_index
        .and_then(|index| history.get(index))
        .is_some_and(|entry| entry == input)
}

/// 判断上方向键是否可以进入历史浏览。
///
/// 参数:
/// - `input`: 当前输入
/// - `history`: 历史记录
/// - `history_clean_index`: 最近选中的历史下标
///
/// 返回:
/// - 输入为空或仍为未修改历史时返回 true
pub(super) fn repl_should_browse_history(
    input: &str,
    history: &[String],
    history_clean_index: Option<usize>,
) -> bool {
    !history.is_empty()
        && (input.is_empty() || repl_history_is_clean(input, history, history_clean_index))
}

/// 循环切换 REPL 权限模式。
///
/// 参数:
/// - `mode`: 当前模式
///
/// 返回:
/// - 下一模式
fn cycle_repl_mode(mode: AgentMode) -> AgentMode {
    match mode {
        AgentMode::Yolo => AgentMode::Audited,
        AgentMode::Audited => AgentMode::AutoAudit,
        AgentMode::AutoAudit => AgentMode::Plan,
        AgentMode::Plan => AgentMode::Yolo,
    }
}

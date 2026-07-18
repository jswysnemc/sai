use super::repl_chrome::ReplChrome;
use super::repl_runtime::ReplRuntime;
use super::*;

pub(super) struct ReplInputSubmission {
    pub(super) mode: AgentMode,
    pub(super) raw_input: String,
    pub(super) chat_input: clipboard::ClipboardChatInput,
}

/// 启用 REPL 原始输入模式并确保编辑光标可见。
///
/// 参数:
/// - `stdout`: 终端输出
///
/// 返回:
/// - 启用是否成功
pub(super) fn enable_repl_terminal_input(stdout: &mut io::Stdout) -> Result<()> {
    terminal::enable_raw_mode()?;
    if let Err(err) = execute!(
        stdout,
        Show,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES),
        EnableBracketedPaste
    ) {
        let _ = terminal::disable_raw_mode();
        return Err(err.into());
    }
    Ok(())
}

/// 恢复 REPL 输入终端模式。
///
/// 参数:
/// - `stdout`: 终端输出
///
/// 返回:
/// - 恢复是否成功
pub(super) fn disable_repl_terminal_input(stdout: &mut io::Stdout) -> Result<()> {
    let restore_result = execute!(stdout, DisableBracketedPaste, PopKeyboardEnhancementFlags);
    let raw_result = terminal::disable_raw_mode();
    restore_result?;
    raw_result?;
    Ok(())
}

/// 读取、编辑并提交 REPL 输入，同时在 debounce 到期时处理 resize 重放。
///
/// 参数:
/// - `mode`: 当前 REPL 模式
/// - `prefill`: 待编辑的预填输入
/// - `history`: 输入历史记录
/// - `chrome`: 可变的输入区 chrome 状态
/// - `runtime`: REPL 终端运行期
///
/// 返回:
/// - 用户提交的输入，或退出时的空值
pub(super) fn read_repl_input(
    mut mode: AgentMode,
    prefill: Option<String>,
    history: &[String],
    chrome: &mut ReplChrome,
    runtime: &mut ReplRuntime,
) -> Result<Option<ReplInputSubmission>> {
    let mut stdout = io::stdout();
    let mut input = strip_terminal_control_sequences(&prefill.unwrap_or_default());
    let mut cursor = input.chars().count();
    let mut slash_selection = 0usize;
    let mut history_index = history.len();
    let mut history_clean_index = None::<usize>;
    let mut clipboard_state = ReplClipboardState::default();
    let mut last_escape = None::<Instant>;
    let mut last_ctrl_c = None::<Instant>;
    // 输入框由 composer 绝对定位绘制；这里禁止直接向终端写换行，
    // 否则屏幕底部会触发受管模型感知不到的滚动，吞掉上方内容
    enable_repl_terminal_input(&mut stdout)?;
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
                slash_selection,
                runtime,
            )
        };
    }
    redraw_input!()?;
    loop {
        if let Some(wait) = runtime.pending_wait() {
            if !event::poll(wait)? {
                if runtime.process_idle_tick()? {
                    input_row = 0;
                    rendered_rows = 0;
                    redraw_input!()?;
                }
                continue;
            }
        }
        match event::read()? {
            Event::Resize(cols, rows) => runtime.observe_input_resize(cols, rows),
            Event::Paste(text) => {
                let text = strip_terminal_control_sequences(&text);
                insert_str_at_cursor(&mut input, &mut cursor, &text);
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
                match code {
                    KeyCode::Tab => {
                        if input.starts_with('/') {
                            if let Some(completed) = complete_repl_command(&input) {
                                input = completed.to_string();
                                cursor = input.chars().count();
                                history_clean_index = None;
                            }
                            slash_selection = 0;
                        } else {
                            mode = match mode {
                                AgentMode::Yolo => AgentMode::Audited,
                                AgentMode::Audited => AgentMode::Plan,
                                AgentMode::Plan => AgentMode::Yolo,
                            };
                            chrome.set_mode(mode);
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
                        cursor = cursor.saturating_sub(1);
                        redraw_input!()?;
                    }
                    KeyCode::Right => {
                        cursor = (cursor + 1).min(input.chars().count());
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
                        let suggestions = repl_command_suggestions(&input);
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
                        let suggestions = repl_command_suggestions(&input);
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
                        let suggestions = repl_command_suggestions(&input);
                        if let Some(selected) = suggestions
                            .get(slash_selection.min(suggestions.len().saturating_sub(1)))
                        {
                            input = selected.command.to_string();
                            slash_selection = 0;
                        }
                        input = strip_terminal_control_sequences(&input);
                        let chat_input = clipboard_state.to_chat_input(&input);
                        let raw_input = std::mem::take(&mut input);
                        cursor = 0;
                        clipboard_state.clear();
                        is_pasted = false;
                        // 1. 提交后立即显示空 composer，流式输出始终插入其上方
                        redraw_input!()?;
                        disable_repl_terminal_input(&mut stdout)?;
                        return Ok(Some(ReplInputSubmission {
                            mode,
                            raw_input,
                            chat_input,
                        }));
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
                        disable_repl_terminal_input(&mut stdout)?;
                        match edit_input_buffer(&input) {
                            Ok(edited) => {
                                input = strip_terminal_control_sequences(&edited);
                                cursor = input.chars().count();
                                slash_selection = 0;
                                history_clean_index = None;
                                clipboard_state.clear();
                            }
                            Err(err) => {
                                eprintln!("{err}");
                            }
                        }
                        enable_repl_terminal_input(&mut stdout)?;
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
                            clear_repl_input(&mut stdout, input_row, rendered_rows)?;
                            runtime.end_composer()?;
                            disable_repl_terminal_input(&mut stdout)?;
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
                        clear_repl_input(&mut stdout, input_row, rendered_rows)?;
                        runtime.end_composer()?;
                        disable_repl_terminal_input(&mut stdout)?;
                        return Ok(None);
                    }
                    KeyCode::Char('l') if modifiers.contains(KeyModifiers::CONTROL) => {
                        runtime.redraw()?;
                        input_row = 0;
                        rendered_rows = 0;
                        redraw_input!()?;
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

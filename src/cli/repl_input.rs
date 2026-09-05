use super::repl_chrome::{chrome_input_content_cols, ReplChrome};
use super::repl_clipboard::{is_paste_key, paste_image_first, ReplClipboardState};
use super::repl_external_events::ReplExternalEvents;
use super::repl_runtime::{QueuePanelIdleResult, ReplRuntime};
use super::repl_windows_paste::{WindowsPasteKey, WindowsPasteState};
use super::*;

const EXTERNAL_EVENT_INPUT_POLL_INTERVAL: Duration = Duration::from_millis(50);

mod cleanup;
mod editing;
mod submission;

use cleanup::finish_repl_input;
use editing::*;
#[cfg(test)]
pub(super) use editing::{repl_history_is_clean, repl_should_browse_history};
pub(super) use submission::{ReplInputDraft, ReplInputEvent, ReplInputSubmission};

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
    let mut windows_paste = WindowsPasteState::default();
    macro_rules! redraw_input {
        () => {
            render_repl_input(
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
        // 回执让位给用户按键：自动轮一旦启动就接管终端，而 take_ready 原本无条件
        // 排在读键之前，用户连第一个字符都打不进去（input 始终为空，靠它判断无效）。
        // 先做一次零超时探测，终端里已有待读事件就走正常按键流程。
        let user_pending = event::poll(Duration::from_secs(0))?;
        if !user_pending {
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
        }
        let queued_event = runtime.pop_input_event();
        if queued_event.is_none() {
            let wait = match (runtime.pending_wait(), external_events.is_armed()) {
                (Some(wait), true) => Some(wait.min(EXTERNAL_EVENT_INPUT_POLL_INTERVAL)),
                (Some(wait), false) => Some(wait),
                (None, true) => Some(EXTERNAL_EVENT_INPUT_POLL_INTERVAL),
                (None, false) => None,
            };
            // 占位提示为静态文本，空输入无需额外唤醒重绘，直接沿用挂起等待
            if let Some(wait) = wait {
                if !event::poll(wait)? {
                    // 内容未变时 draw_lines 会按签名跳过重绘；清零记账等于
                    // 绕开该缓存强制实绘，因此只在确有刷新时才重置
                    if runtime.process_idle_tick()? {
                        input_row = 0;
                        rendered_rows = 0;
                    }
                    redraw_input!()?;
                    continue;
                }
            }
        }
        let event = queued_event.map(Ok).unwrap_or_else(event::read)?;
        match event {
            Event::Resize(cols, rows) => runtime.observe_input_resize(cols, rows),
            Event::Paste(text) => {
                windows_paste.reset();
                // Windows 终端把 Ctrl+V 转成括号粘贴的文本事件，图片不会以文本
                // 形式到达；剪贴板里有图时先按图片插入，否则按普通文本粘贴
                if paste_image_first(&mut clipboard_state, &mut input, &mut cursor) {
                    slash_selection = 0;
                    history_clean_index = None;
                    is_pasted = true;
                    redraw_input!()?;
                    continue;
                }
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
                let replay_key = match code {
                    KeyCode::Char(ch)
                        if !modifiers.contains(KeyModifiers::CONTROL)
                            && !modifiers.contains(KeyModifiers::ALT) =>
                    {
                        Some(WindowsPasteKey::Char(ch))
                    }
                    KeyCode::Enter if modifiers.is_empty() => Some(WindowsPasteKey::Enter),
                    KeyCode::Tab if modifiers.is_empty() => Some(WindowsPasteKey::Tab),
                    _ => None,
                };
                if replay_key.is_some_and(|key| windows_paste.consume_key(key)) {
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
                // Ctrl+C 仍由后面的匹配处理；其余键在队列管理态由面板吞掉
                let skip_queue_for_ctrl_c =
                    matches!(code, KeyCode::Char('c')) && modifiers.contains(KeyModifiers::CONTROL);
                if !skip_queue_for_ctrl_c
                    && (runtime.queue_panel_active()
                        || (code == KeyCode::Up && modifiers.contains(KeyModifiers::CONTROL)))
                {
                    match runtime.handle_queue_panel_idle_key(
                        code,
                        modifiers,
                        input.trim().is_empty(),
                    )? {
                        QueuePanelIdleResult::Ignored => {}
                        QueuePanelIdleResult::Consumed => {
                            input_row = 0;
                            rendered_rows = 0;
                            redraw_input!()?;
                            continue;
                        }
                        QueuePanelIdleResult::Edit(item) => {
                            input = item.text;
                            cursor = input.chars().count();
                            clipboard_state = item.clipboard;
                            mode = item.mode;
                            chrome.set_mode(mode);
                            slash_selection = 0;
                            history_clean_index = None;
                            is_pasted = false;
                            input_row = 0;
                            rendered_rows = 0;
                            redraw_input!()?;
                            continue;
                        }
                        QueuePanelIdleResult::SendNow(item) => {
                            if !input.trim().is_empty() {
                                let leftover = runtime.stream_draft_mut();
                                leftover.text = input;
                                leftover.cursor = cursor;
                                leftover.clipboard = clipboard_state;
                                leftover.mode = Some(mode);
                            }
                            let submission = ReplInputSubmission::from_input(
                                item.mode,
                                item.text,
                                &item.clipboard,
                            );
                            finish_repl_input(
                                &mut stdout,
                                input_row,
                                rendered_rows,
                                runtime,
                                &mut terminal_guard,
                            )?;
                            return Ok(Some(ReplInputEvent::User(submission)));
                        }
                    }
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
                        // Tab：引用补全或斜杠命令补全
                        if let Some((next, next_cursor)) = complete_active_mention(
                            &input,
                            cursor,
                            slash_selection,
                            runtime.mention_skills(),
                            &mut clipboard_state,
                        ) {
                            input = next;
                            cursor = next_cursor;
                            slash_selection = 0;
                            history_clean_index = None;
                        } else if input.starts_with('/') {
                            if let Some(completed) = complete_repl_command(&input, false) {
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
                        // 补全面板打开时，单次 Esc 先收起面板。
                        // 面板是最显眼的界面元素，却只有「双击 Esc（顺带清空草稿
                        // 和剪贴板附件）」能关掉，单击看起来像按键失灵
                        let panel_open =
                            !active_mention_suggestions(&input, cursor, runtime.mention_skills())
                                .is_empty()
                                || !visible_repl_command_suggestions(&input, false).is_empty();
                        if panel_open && !runtime.composer_panels_dismissed() {
                            runtime.dismiss_composer_panels(&input, cursor);
                            last_escape = None;
                            redraw_input!()?;
                            continue;
                        }
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
                    // 词级移动：Ctrl+←/→ 是 emacs/readline 通用键位，
                    // Alt+←/→ 在多数终端上等价。没有这两个绑定时，
                    // 长句子里只能一个字符一个字符挪
                    KeyCode::Left
                        if modifiers.contains(KeyModifiers::CONTROL)
                            || modifiers.contains(KeyModifiers::ALT) =>
                    {
                        cursor = word_start_before(&input, cursor);
                        redraw_input!()?;
                    }
                    KeyCode::Right
                        if modifiers.contains(KeyModifiers::CONTROL)
                            || modifiers.contains(KeyModifiers::ALT) =>
                    {
                        cursor = word_end_after(&input, cursor);
                        redraw_input!()?;
                    }
                    KeyCode::Left => {
                        // 输入为空时左移光标没有意义，改为打开会话树
                        if input.is_empty() {
                            // 与其它退出路径一样走完整收尾：只 finish guard 会让
                            // composer 仍带着过期的 last_composer_signature 注册着，
                            // 而下面要拉起的是一个嵌套全屏界面
                            finish_repl_input(
                                &mut stdout,
                                input_row,
                                rendered_rows,
                                runtime,
                                &mut terminal_guard,
                            )?;
                            return Ok(Some(ReplInputEvent::User(ReplInputSubmission {
                                mode,
                                raw_input: "/tree".to_string(),
                                chat_input: clipboard_state.to_chat_input("/tree"),
                                echo: crate::render::input_atom::InputEcho {
                                    text: "/tree".to_string(),
                                    ..Default::default()
                                },
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
                    // Ctrl+A / Ctrl+E 与 Home / End 等价：Windows 终端与部分
                    // 远程会话下 Home / End 未必能送达，emacs 键位是通用回退
                    KeyCode::Home | KeyCode::Char('a')
                        if code == KeyCode::Home || modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        cursor = 0;
                        redraw_input!()?;
                    }
                    KeyCode::End | KeyCode::Char('e')
                        if code == KeyCode::End || modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        cursor = input.chars().count();
                        redraw_input!()?;
                    }
                    KeyCode::Up => {
                        let mentions =
                            active_mention_suggestions(&input, cursor, runtime.mention_skills());
                        if !mentions.is_empty() {
                            slash_selection = (slash_selection % mentions.len())
                                .checked_sub(1)
                                .unwrap_or(mentions.len().saturating_sub(1));
                            redraw_input!()?;
                        } else {
                            let suggestions = visible_repl_command_suggestions(&input, false);
                            if !suggestions.is_empty() {
                                slash_selection = (slash_selection % suggestions.len())
                                    .checked_sub(1)
                                    .unwrap_or(suggestions.len().saturating_sub(1));
                                redraw_input!()?;
                            } else {
                                let plain_prefix = String::new();
                                // 折行宽度必须与 composer 绘制时一致：绘制用的是去掉
                                // "> " 边距后的内容宽，用整宽会让落点逐行偏移
                                if let Some(next_cursor) = move_cursor_up_by_visual_row(
                                    &plain_prefix,
                                    &input,
                                    cursor,
                                    chrome_input_content_cols(terminal_cols()),
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
                    }
                    KeyCode::Down => {
                        let mentions =
                            active_mention_suggestions(&input, cursor, runtime.mention_skills());
                        if !mentions.is_empty() {
                            slash_selection = (slash_selection + 1) % mentions.len();
                        } else {
                            let suggestions = visible_repl_command_suggestions(&input, false);
                            if !suggestions.is_empty() {
                                slash_selection = (slash_selection + 1) % suggestions.len();
                            } else {
                                let plain_prefix = String::new();
                                if let Some(next_cursor) = move_cursor_down_by_visual_row(
                                    &plain_prefix,
                                    &input,
                                    cursor,
                                    chrome_input_content_cols(terminal_cols()),
                                ) {
                                    cursor = next_cursor;
                                } else if repl_history_is_clean(
                                    &input,
                                    history,
                                    history_clean_index,
                                ) && history_index + 1 < history.len()
                                {
                                    history_index += 1;
                                    input = history.get(history_index).cloned().unwrap_or_default();
                                    cursor = input.chars().count();
                                    history_clean_index = Some(history_index);
                                    slash_selection = 0;
                                    clipboard_state.clear();
                                    is_pasted = false;
                                } else if repl_history_is_clean(
                                    &input,
                                    history,
                                    history_clean_index,
                                ) && history_index < history.len()
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
                        let input_before_cursor: String = input.chars().take(cursor).collect();
                        if let Some(paste) = windows_paste
                            .begin_from_system_clipboard(&input_before_cursor, Instant::now())
                        {
                            clipboard_state.replace_recent_text_with_paste(
                                &mut input,
                                &mut cursor,
                                paste.prefix_chars,
                                paste.text,
                            );
                            slash_selection = 0;
                            history_clean_index = None;
                            is_pasted = true;
                            redraw_input!()?;
                            continue;
                        }
                        if let Some((next, next_cursor)) = complete_active_mention(
                            &input,
                            cursor,
                            slash_selection,
                            runtime.mention_skills(),
                            &mut clipboard_state,
                        ) {
                            input = next;
                            cursor = next_cursor;
                            slash_selection = 0;
                            history_clean_index = None;
                            is_pasted = false;
                            redraw_input!()?;
                            continue;
                        }
                        let suggestions = visible_repl_command_suggestions(&input, false);
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
                        let submission = ReplInputSubmission::from_input(
                            mode,
                            std::mem::take(&mut input),
                            &clipboard_state,
                        );
                        cursor = 0;
                        clipboard_state.clear();
                        is_pasted = false;
                        // 1. 提交后立即显示空 composer，流式输出始终插入其上方
                        redraw_input!()?;
                        terminal_guard.finish(&mut stdout)?;
                        return Ok(Some(ReplInputEvent::User(submission)));
                    }
                    KeyCode::Char('j') if modifiers.contains(KeyModifiers::CONTROL) => {
                        insert_newline_at_cursor(&mut input, &mut cursor);
                        slash_selection = 0;
                        history_clean_index = None;
                        is_pasted = false;
                        redraw_input!()?;
                    }
                    KeyCode::Char('v')
                        if is_paste_key(runtime.paste_image_key(), code, modifiers) =>
                    {
                        windows_paste.reset();
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
                        // 第一次 Ctrl+C：清空草稿并提示再按一次退出（对齐 Claude 双击退出）
                        runtime.record_meta(
                            t("Press Ctrl+C again to exit", "再按一次 Ctrl+C 退出").to_string(),
                        )?;
                        input_row = 0;
                        rendered_rows = 0;
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
                        } else {
                            // 提示语里主动宣传了 Ctrl+O，按下去却毫无反应会让人以为键位坏了
                            runtime.record_meta(
                                t(
                                    "Ctrl+O: no expandable command output or diff yet",
                                    "Ctrl+O：当前还没有可展开的命令输出或 diff",
                                )
                                .to_string(),
                            )?;
                            redraw_input!()?;
                        }
                    }
                    KeyCode::Char('t') if modifiers.contains(KeyModifiers::CONTROL) => {
                        // Ctrl+T：沉底 todo 单行 / 多行切换
                        if runtime.toggle_todo_panel_compact() {
                            input_row = 0;
                            rendered_rows = 0;
                            redraw_input!()?;
                        } else {
                            runtime.record_meta(
                                t(
                                    "Ctrl+T: no active plan to fold",
                                    "Ctrl+T：当前没有进行中的计划可折叠",
                                )
                                .to_string(),
                            )?;
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
                    KeyCode::Char(ch)
                        if !modifiers.contains(KeyModifiers::CONTROL)
                            && !modifiers.contains(KeyModifiers::ALT) =>
                    {
                        if !is_disallowed_control_char(ch) {
                            windows_paste.record_char(ch, Instant::now());
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

use super::*;

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
pub(in crate::cli) fn read_repl_input(
    mut mode: AgentMode,
    prefill: Option<String>,
    prefill_clipboard: Option<ReplClipboardState>,
    history: &[crate::state::input_history::InputHistoryEntry],
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
    let mut terminal_guard = runtime.acquire_input_guard(&mut stdout, true)?;
    // 1. 【终端布局】【固定输入区】全屏使用已知布局坐标，避免光标查询占用终端事件读取器
    let mut input_row = if runtime.fullscreen_active() {
        0
    } else {
        cursor::position()?.1
    };
    let mut rendered_rows = 0u16;
    let mut is_pasted = false;
    let mut windows_paste = WindowsPasteState::default();
    if let Some(draft) = runtime.take_footer_draft() {
        input = draft.text;
        cursor = draft.cursor;
        clipboard_state = draft.clipboard;
        slash_selection = draft.slash_selection;
        is_pasted = draft.is_pasted;
        windows_paste = draft.windows_paste;
    }
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
        if runtime.has_session_switch_request() {
            runtime.save_session_input_draft(crate::cli::repl_runtime::StreamComposerDraft {
                text: input,
                cursor,
                clipboard: clipboard_state,
                slash_selection,
                is_pasted,
                windows_paste,
                mode: Some(mode),
            });
            terminal_guard.finish(&mut stdout)?;
            return Ok(Some(ReplInputEvent::SwitchSession));
        }
        if let Some(target_group) = runtime.take_open_group_request() {
            runtime.save_session_input_draft(crate::cli::repl_runtime::StreamComposerDraft {
                text: input,
                cursor,
                clipboard: clipboard_state,
                slash_selection,
                is_pasted,
                windows_paste,
                mode: Some(mode),
            });
            terminal_guard.finish(&mut stdout)?;
            let cmd = match target_group {
                Some(id) => format!("/group open {id}"),
                None => "/group".to_string(),
            };
            return Ok(Some(ReplInputEvent::User(ReplInputSubmission::control(
                mode, cmd,
            ))));
        }
        for diagnostic in chrome.status_diagnostics() {
            runtime.record_meta(diagnostic)?;
        }
        // 回执让位给用户按键：自动轮一旦启动就接管终端，而 take_ready 原本无条件
        // 排在读键之前，用户连第一个字符都打不进去（input 始终为空，靠它判断无效）。
        // 先做一次零超时探测，终端里已有待读事件就走正常按键流程。
        let queued_event = runtime.pop_input_event();
        let user_pending = queued_event.is_some() || crate::cli::terminal_events::ready()?;
        if !user_pending && !runtime.background_panel_active() {
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
        if queued_event.is_none() {
            let wait = match (runtime.pending_wait(), external_events.is_armed()) {
                (Some(wait), true) => Some(wait.min(EXTERNAL_EVENT_INPUT_POLL_INTERVAL)),
                (Some(wait), false) => Some(wait),
                (None, true) => Some(EXTERNAL_EVENT_INPUT_POLL_INTERVAL),
                (None, false) => None,
            };
            let wait = chrome.status_poll_wait(wait);
            // 占位提示为静态文本，空输入无需额外唤醒重绘，直接沿用挂起等待
            if let Some(wait) = wait {
                if !crate::cli::terminal_events::poll(wait)? {
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
        let mut event = queued_event
            .map(Ok)
            .unwrap_or_else(crate::cli::terminal_events::read)?;
        if let Some(action) = runtime.footer_action(&event) {
            if let Some(command) = action.command() {
                runtime.save_footer_draft(crate::cli::repl_runtime::StreamComposerDraft {
                    text: input,
                    cursor,
                    clipboard: clipboard_state,
                    slash_selection,
                    is_pasted,
                    windows_paste,
                    mode: Some(mode),
                });
                finish_repl_input(
                    &mut stdout,
                    input_row,
                    rendered_rows,
                    runtime,
                    &mut terminal_guard,
                )?;
                return Ok(Some(ReplInputEvent::User(ReplInputSubmission::control(
                    mode,
                    command.into(),
                ))));
            }
            mode = cycle_repl_mode(mode);
            chrome.set_mode(mode);
            redraw_input!()?;
            continue;
        }
        match editing::process_idle_completion_mouse(
            runtime,
            &event,
            &mut input,
            &mut cursor,
            &mut slash_selection,
            &mut clipboard_state,
            &mut history_clean_index,
            &mut is_pasted,
        ) {
            editing::IdleCompletionStep::Ignored => {}
            editing::IdleCompletionStep::Consumed => continue,
            editing::IdleCompletionStep::Redraw => {
                redraw_input!()?;
                continue;
            }
            editing::IdleCompletionStep::Submit => {
                event = Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            }
        }
        if runtime.handle_inline_composer_mouse(&event)? {
            if let Some(position) = runtime.take_composer_mouse_cursor(&input, &clipboard_state) {
                cursor = position;
                slash_selection = 0;
            }
            if !matches!(&event, Event::Mouse(m) if m.kind == crossterm::event::MouseEventKind::Moved)
            {
                input_row = 0;
                rendered_rows = 0;
            }
            redraw_input!()?;
            continue;
        }
        // 全屏视图先处理鼠标与浏览键，其余按键仍交给输入框
        if runtime.handle_fullscreen_event(&event)? == FullscreenEvent::Consumed {
            if let Some(position) = runtime.take_composer_mouse_cursor(&input, &clipboard_state) {
                cursor = position;
                slash_selection = 0;
                last_escape = None;
                redraw_input!()?;
            }
            continue;
        }
        match event {
            Event::Resize(cols, rows) => runtime.observe_input_resize(cols, rows)?,
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
                if paste::handle_unbracketed_paste(
                    &mut windows_paste,
                    &mut clipboard_state,
                    &mut input,
                    &mut cursor,
                    code,
                    modifiers,
                ) {
                    slash_selection = 0;
                    history_clean_index = None;
                    is_pasted = true;
                    redraw_input!()?;
                    continue;
                }
                if code != KeyCode::Esc && last_escape.take().is_some() {
                    // 清空确认窗口被其它按键打断，提示行随之恢复
                    runtime.clear_key_notice();
                }
                if !crate::cli::terminal_events::is_ctrl_char(code, modifiers, 'c') {
                    last_ctrl_c = None;
                }
                let focus = runtime.panel_focus(&input, cursor);
                if runtime.navigate_completion(
                    &input,
                    cursor,
                    &mut slash_selection,
                    code,
                    modifiers,
                ) {
                    redraw_input!()?;
                    continue;
                }
                if !focus.owns_viewport_keys()
                    && crate::cli::repl_transcript_pager::is_jump_to_output_bottom_key(
                        code, modifiers,
                    )
                {
                    runtime.jump_to_output_bottom(false)?;
                    input_row = 0;
                    rendered_rows = 0;
                    redraw_input!()?;
                    continue;
                }
                // Ctrl+C 仍由后面的匹配处理；其余键在队列管理态由面板吞掉
                let skip_queue_for_ctrl_c =
                    crate::cli::terminal_events::is_ctrl_char(code, modifiers, 'c');
                if !skip_queue_for_ctrl_c
                    && (runtime.queue_panel_active()
                        || crate::cli::repl_runtime::queue_panel::is_enter_queue_key(
                            code, modifiers,
                        ))
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
                    crate::cli::repl_transcript_pager::open_transcript_pager(runtime)?;
                    runtime.resync_after_overlay()?;
                    input_row = 0;
                    rendered_rows = 0;
                    redraw_input!()?;
                    continue;
                }
                // 1. 【终端输入】【共享编辑】统一处理首尾移动、词级移动与词/行删除（兼容 Winc/macOS/readline）
                if let Some(outcome) =
                    shortcuts::apply_composer_edit_key(&mut input, &mut cursor, code, modifiers)
                {
                    if outcome == shortcuts::ComposerEditOutcome::TextEdited {
                        slash_selection = 0;
                        history_clean_index = None;
                        is_pasted = false;
                    }
                    redraw_input!()?;
                    continue;
                }
                // 2. 【终端输入】【换行快捷】支持 Shift+Enter、Alt+Enter、Ctrl+J 及空闲态 Ctrl+Enter（兼容旧版 ConPTY Ctrl+J）
                if shortcuts::is_composer_newline_key(code, modifiers)
                    || (code == KeyCode::Enter && modifiers.contains(KeyModifiers::CONTROL))
                {
                    insert_newline_at_cursor(&mut input, &mut cursor);
                    slash_selection = 0;
                    history_clean_index = None;
                    is_pasted = false;
                    redraw_input!()?;
                    continue;
                }
                if is_paste_key(runtime.paste_image_key(), code, modifiers) {
                    windows_paste.reset();
                    is_pasted = clipboard_state.paste_into_input(&mut input, &mut cursor)?;
                    slash_selection = 0;
                    history_clean_index = None;
                    redraw_input!()?;
                    continue;
                }
                if shortcuts::is_external_editor_key(code, modifiers) {
                    // 外部编辑器接管主屏，先退出全屏视图再清理输入区
                    runtime.leave_fullscreen()?;
                    clear_repl_input(&mut stdout, input_row, rendered_rows)?;
                    runtime.end_composer()?;
                    terminal_guard.finish(&mut stdout)?;
                    if let Some(edited) = external_editor::edit(&input, &mut clipboard_state) {
                        input = edited;
                        cursor = input.chars().count();
                        slash_selection = 0;
                        history_clean_index = None;
                    }
                    terminal_guard = crate::cli::terminal_restore::TerminalInputGuard::enable(
                        &mut stdout,
                        true,
                    )?;
                    input_row = 0;
                    rendered_rows = 0;
                    is_pasted = false;
                    redraw_input!()?;
                    continue;
                }
                if shortcuts::is_redraw_key(code, modifiers) {
                    runtime.redraw()?;
                    input_row = 0;
                    rendered_rows = 0;
                    redraw_input!()?;
                    continue;
                }
                if shortcuts::is_fullscreen_toggle_key(code, modifiers) {
                    // Ctrl+O / Alt+O：进入或退出全屏会话视图，输入框随之迁移
                    runtime.toggle_fullscreen()?;
                    input_row = 0;
                    rendered_rows = 0;
                    redraw_input!()?;
                    continue;
                }
                if shortcuts::is_todo_fold_key(code, modifiers) {
                    // Ctrl+T / Alt+T：沉底 todo 单行 / 多行切换
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
                        editing::apply_tab_completion(
                            runtime,
                            &mut input,
                            &mut cursor,
                            &mut slash_selection,
                            &mut clipboard_state,
                            &mut history_clean_index,
                        );
                        is_pasted = false;
                        redraw_input!()?;
                    }
                    KeyCode::Esc => {
                        // 1. 快捷键速查展开时，Esc 只负责收起
                        if runtime.close_shortcuts() {
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
                            runtime.clear_key_notice();
                            redraw_input!()?;
                        } else {
                            last_escape = Some(now);
                            // 2. 有输入时在提示行说明再按一次 Esc 会清空
                            if !input.is_empty() {
                                runtime.arm_clear_input_hint(REPL_ESC_CLEAR_WINDOW);
                                redraw_input!()?;
                            }
                        }
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
                            return Ok(Some(ReplInputEvent::User(
                                ReplInputSubmission::from_input(
                                    mode,
                                    "/tree".to_string(),
                                    &clipboard_state,
                                ),
                            )));
                        }
                        // 剪贴板占位块整体跳过，保持与删除一致的原子性
                        cursor = clipboard_state.cursor_left(&input, cursor);
                        redraw_input!()?;
                    }
                    KeyCode::Right => {
                        cursor = clipboard_state.cursor_right(&input, cursor);
                        redraw_input!()?;
                    }
                    KeyCode::Up => {
                        if history::navigate_up(
                            &mut input,
                            &mut cursor,
                            &mut clipboard_state,
                            history,
                            &mut history_index,
                            &mut history_clean_index,
                            &mut slash_selection,
                            &mut is_pasted,
                        ) {
                            redraw_input!()?;
                        }
                    }
                    KeyCode::Down => {
                        history::navigate_down(
                            &mut input,
                            &mut cursor,
                            &mut clipboard_state,
                            history,
                            &mut history_index,
                            &mut history_clean_index,
                            &mut slash_selection,
                            &mut is_pasted,
                        );
                        redraw_input!()?;
                    }
                    KeyCode::Enter => {
                        if let Some((next, next_cursor)) = complete_active_mention(
                            &input,
                            cursor,
                            slash_selection,
                            runtime,
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
                        if !runtime.composer_panels_dismissed() {
                            if let Some(completed) = crate::cli::repl_completion::accept(
                                &input,
                                cursor,
                                slash_selection,
                                false,
                                true,
                            ) {
                                input = completed;
                                cursor = input.chars().count();
                                slash_selection = 0;
                                history_clean_index = None;
                                redraw_input!()?;
                                continue;
                            }
                        }
                        input = strip_terminal_control_sequences(&input);
                        if crate::cli::repl_commands::is_repl_exit_command(&input) {
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
                    KeyCode::Char('c' | 'C') if modifiers.contains(KeyModifiers::CONTROL) => {
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
                        // 第一次 Ctrl+C：清空草稿，按键提示行显示再按一次退出，不再写入会话记录
                        runtime.arm_exit_hint(REPL_CTRL_C_EXIT_WINDOW);
                        input_row = 0;
                        rendered_rows = 0;
                        redraw_input!()?;
                    }
                    KeyCode::Char('d' | 'D')
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
                    KeyCode::Backspace => {
                        let changed =
                            delete_input_atom(&mut clipboard_state, &mut input, &mut cursor, true);
                        slash_selection = 0;
                        if changed {
                            history_clean_index = None;
                        }
                        is_pasted = false;
                        redraw_input!()?;
                    }
                    KeyCode::Delete => {
                        let changed =
                            delete_input_atom(&mut clipboard_state, &mut input, &mut cursor, false);
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
                        // 空输入时 `?` 打开或收起快捷键速查，不作为字符输入
                        if ch == '?' && runtime.toggle_shortcuts(input.is_empty()) {
                            redraw_input!()?;
                            continue;
                        }
                        runtime.close_shortcuts();
                        if !is_disallowed_control_char(ch) {
                            let text = event_batch::take_text_batch(ch, runtime)?;
                            windows_paste.record_text(&text, Instant::now());
                            insert_str_at_cursor(&mut input, &mut cursor, &text);
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

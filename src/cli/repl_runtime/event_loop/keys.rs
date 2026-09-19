use super::*;

/// 将单个按键应用到运行中 composer 草稿。
///
/// 参数:
/// - `runtime`: REPL 运行期
/// - `ctx`: 轮次开始时抓取的命令上下文
/// - `code`: 键码
/// - `modifiers`: 修饰键
///
/// 返回:
/// - 按键请求中断、退出或继续
pub(super) fn handle_stream_key(
    runtime: &mut ReplRuntime,
    ctx: &StreamCommandContext,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<StreamInputAction> {
    let draft = runtime.stream_draft();
    let (text, cursor, mut selected) = (draft.text.clone(), draft.cursor, draft.slash_selection);
    if runtime.navigate_completion(&text, cursor, &mut selected, code, modifiers) {
        runtime.stream_draft_mut().slash_selection = selected;
        runtime.redraw_stream_composer()?;
        return Ok(StreamInputAction::Continue);
    }
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
            // Tab：先补全 #/@ 引用，再补全斜杠命令，否则入队
            if complete_stream_mention(runtime)? {
                return Ok(StreamInputAction::Continue);
            }
            if runtime.stream_draft().text.starts_with('/') {
                // 运行期间不补全置灰命令：Tab 出一条执行不了的命令没有意义
                let completed = crate::cli::repl_completion::accept(
                    &runtime.stream_draft().text,
                    runtime.stream_draft().cursor,
                    runtime.stream_draft().slash_selection,
                    true,
                    false,
                );
                if let Some(completed) = completed {
                    let draft = runtime.stream_draft_mut();
                    draft.text = completed.to_string();
                    draft.cursor = draft.text.chars().count();
                    draft.slash_selection = 0;
                    runtime.redraw_stream_composer()?;
                    return Ok(StreamInputAction::Continue);
                }
            }
            if runtime.stream_draft().text.trim().is_empty() {
                runtime.record_meta(
                    crate::i18n::text("type a message before queuing", "输入内容后再排队")
                        .to_string(),
                )?;
                return Ok(StreamInputAction::Continue);
            }
            // 与 Enter 同一套分发：置灰命令拒绝留在输入框，避免本轮结束后悄悄执行
            return dispatch_stream_command(runtime, ctx);
        }
        KeyCode::Enter => {
            if modifiers.contains(KeyModifiers::SHIFT) {
                let draft = runtime.stream_draft_mut();
                insert_char(&mut draft.text, &mut draft.cursor, '\n');
                draft.slash_selection = 0;
                draft.is_pasted = false;
                runtime.redraw_stream_composer()?;
                return Ok(StreamInputAction::Continue);
            }
            if complete_stream_mention(runtime)? {
                return Ok(StreamInputAction::Continue);
            }
            if !runtime.composer_panels_dismissed() {
                let draft = runtime.stream_draft();
                if let Some(completed) = crate::cli::repl_completion::accept(
                    &draft.text,
                    draft.cursor,
                    draft.slash_selection,
                    true,
                    true,
                ) {
                    let draft = runtime.stream_draft_mut();
                    draft.text = completed;
                    draft.cursor = draft.text.chars().count();
                    draft.slash_selection = 0;
                    runtime.redraw_stream_composer()?;
                    return Ok(StreamInputAction::Continue);
                }
            }
            return dispatch_stream_command(runtime, ctx);
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
        KeyCode::Char('v') if is_paste_key(runtime.paste_image_key(), code, modifiers) => {
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
            let text = take_text_batch(ch, runtime)?;
            let draft = runtime.stream_draft_mut();
            draft
                .windows_paste
                .record_text(&text, std::time::Instant::now());
            crate::cli::repl_text::insert_str_at_cursor(&mut draft.text, &mut draft.cursor, &text);
            draft.slash_selection = 0;
            draft.is_pasted = false;
            runtime.redraw_stream_composer()?;
        }
        _ => {}
    }
    Ok(StreamInputAction::Continue)
}

use super::*;

/// 三档配置 × 常见按键：只认自己那一档，且不误吞词级移动的 Alt+←/→。
#[test]
fn is_paste_key_matches_only_the_configured_key() {
    let v = KeyCode::Char('v');
    let left = KeyCode::Left;
    let ctrl = KeyModifiers::CONTROL;
    let alt = KeyModifiers::ALT;
    let none = KeyModifiers::NONE;

    assert!(is_paste_key(PasteImageKey::CtrlV, v, ctrl));
    assert!(!is_paste_key(PasteImageKey::CtrlV, v, alt));
    assert!(!is_paste_key(PasteImageKey::CtrlV, v, none));

    assert!(is_paste_key(PasteImageKey::AltV, v, alt));
    assert!(!is_paste_key(PasteImageKey::AltV, v, ctrl));
    assert!(!is_paste_key(PasteImageKey::AltV, v, none));

    assert!(is_paste_key(PasteImageKey::Both, v, ctrl));
    assert!(is_paste_key(PasteImageKey::Both, v, alt));
    assert!(!is_paste_key(PasteImageKey::Both, v, none));

    // 词级移动与别的字符都不该被当成粘贴
    for key in [
        PasteImageKey::CtrlV,
        PasteImageKey::AltV,
        PasteImageKey::Both,
    ] {
        assert!(!is_paste_key(key, left, ctrl));
        assert!(!is_paste_key(key, left, alt));
        assert!(!is_paste_key(key, KeyCode::Char('c'), ctrl));
        assert!(!is_paste_key(key, KeyCode::Enter, none));
    }
}

/// 非 Windows 上括号粘贴带着真文本，探测只会白读一次剪贴板。
#[test]
#[cfg(not(windows))]
fn paste_image_first_is_a_noop_off_windows() {
    let mut state = ReplClipboardState::default();
    let mut input = String::new();
    let mut cursor = 0;
    assert!(!paste_image_first(&mut state, &mut input, &mut cursor));
    assert!(input.is_empty());
}

#[test]
fn short_text_pastes_inline() {
    let mut state = ReplClipboardState::default();
    let mut input = "问: ".to_string();
    let mut cursor = input.chars().count();

    let folded = state.insert_payload(
        &mut input,
        &mut cursor,
        ClipboardPayload::Text("内容".to_string()),
    );

    assert!(!folded);
    assert_eq!(input, "问: 内容");
    assert_eq!(state.to_chat_input(&input).message, "问: 内容");
}

#[test]
fn single_long_line_pastes_as_marker() {
    let mut state = ReplClipboardState::default();
    let mut input = String::new();
    let mut cursor = 0usize;
    // 字符总数未超 LONG_TEXT_CHARS，但单行超 LONG_LINE_CHARS
    let text = "x".repeat(LONG_LINE_CHARS + 1);
    let folded = state.insert_payload(
        &mut input,
        &mut cursor,
        ClipboardPayload::Text(text.clone()),
    );
    assert!(folded);
    assert!(input.starts_with("[text 1 "));
    let chat = state.to_chat_input(&input);
    assert_eq!(chat.message, text);
}

#[test]
fn long_text_pastes_as_marker_and_submits_full_text() {
    let mut state = ReplClipboardState::default();
    let mut input = "总结 ".to_string();
    let mut cursor = input.chars().count();
    let text = "a".repeat(LONG_TEXT_CHARS + 1);

    let folded = state.insert_payload(
        &mut input,
        &mut cursor,
        ClipboardPayload::Text(text.clone()),
    );
    let chat = state.to_chat_input(&input);

    assert!(folded);
    assert!(input.contains("[text 1 201 chars]"));
    assert!(chat.message.contains("<clipboard>"));
    assert!(chat.message.contains(&text));
}

#[test]
fn windows_key_stream_replaces_typed_first_line_with_one_marker() {
    let mut state = ReplClipboardState::default();
    let mut input = "前缀第一行".to_string();
    let mut cursor = input.chars().count();
    let text = format!("第一行\n{}", "第二行\n".repeat(80));

    let folded = state.replace_recent_text_with_paste(
        &mut input,
        &mut cursor,
        "第一行".chars().count(),
        text.clone(),
    );
    let chat = state.to_chat_input(&input);

    assert!(folded);
    assert!(input.starts_with("前缀[text 1 "));
    assert_eq!(input.matches("[text ").count(), 1);
    assert!(chat.message.contains(text.trim()));
}

#[test]
fn image_pastes_as_marker_and_submits_data_url() {
    let mut state = ReplClipboardState::default();
    let mut input = String::new();
    let mut cursor = 0;

    state.insert_payload(
        &mut input,
        &mut cursor,
        ClipboardPayload::ImageDataUrl {
            data_url: "data:image/png;base64,abc".to_string(),
            width: 800,
            height: 600,
        },
    );
    let chat = state.to_chat_input(&input);

    assert_eq!(input, "[image 1 800x600]");
    assert_eq!(chat.message, "请根据剪贴板图片回答。");
    assert_eq!(chat.image_url.as_deref(), Some("data:image/png;base64,abc"));
}

#[test]
fn backspace_removes_whole_marker() {
    let mut state = ReplClipboardState::default();
    let mut input = String::new();
    let mut cursor = 0;
    state.insert_payload(
        &mut input,
        &mut cursor,
        ClipboardPayload::ImageDataUrl {
            data_url: "data:image/png;base64,abc".to_string(),
            width: 800,
            height: 600,
        },
    );

    assert!(state.remove_block_before_cursor(&mut input, &mut cursor));
    assert!(input.is_empty());
    assert_eq!(cursor, 0);
    assert!(state.to_chat_input(&input).image_url.is_none());
}

#[test]
fn delete_removes_whole_marker() {
    let mut state = ReplClipboardState::default();
    let mut input = "x".to_string();
    let mut cursor = 1;
    state.insert_payload(
        &mut input,
        &mut cursor,
        ClipboardPayload::Text("a".repeat(LONG_TEXT_CHARS + 1)),
    );

    assert!(state.remove_block_at_cursor(&mut input, 1));
    assert_eq!(input, "x");
}

#[test]
fn cursor_moves_skip_whole_block() {
    let mut state = ReplClipboardState::default();
    let mut input = "x".to_string();
    let mut cursor = 1;
    state.insert_payload(
        &mut input,
        &mut cursor,
        ClipboardPayload::Text("a".repeat(LONG_TEXT_CHARS + 1)),
    );
    let span = state.block_spans(&input)[0];
    assert_eq!(cursor, span.end);

    // 1. 从块尾左移：整体跳到块首
    assert_eq!(state.cursor_left(&input, span.end), span.start);
    // 2. 从块首右移：整体跳到块尾
    assert_eq!(state.cursor_right(&input, span.start), span.end);
    // 3. 块外移动仍逐字符
    assert_eq!(state.cursor_left(&input, span.start), span.start - 1);
    assert_eq!(
        state.cursor_right(&input, span.end),
        span.end.min(input.chars().count())
    );
}

#[test]
fn block_spans_identify_text_and_image_markers() {
    let mut state = ReplClipboardState::default();
    let mut input = String::new();
    let mut cursor = 0;
    state.insert_payload(
        &mut input,
        &mut cursor,
        ClipboardPayload::Text("a".repeat(LONG_TEXT_CHARS + 1)),
    );
    state.insert_payload(
        &mut input,
        &mut cursor,
        ClipboardPayload::ImageDataUrl {
            data_url: "data:image/png;base64,abc".to_string(),
            width: 10,
            height: 20,
        },
    );

    let spans = state.block_spans(&input);
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].kind, ReplClipboardBlockKind::Text);
    assert_eq!(spans[1].kind, ReplClipboardBlockKind::Image);
    assert_eq!(spans[0].end, spans[1].start);
}

#[test]
fn echo_text_for_submit_expands_only_pasted_text_blocks() {
    let mut state = ReplClipboardState::default();
    let mut input = String::from("前缀 ");
    let mut cursor = input.chars().count();
    let pasted = "a".repeat(LONG_TEXT_CHARS + 1);
    state.paste_text_into_input(&mut input, &mut cursor, pasted.clone());

    let echo = state.echo_text_for_submit(&input);
    assert_eq!(echo.atoms.len(), 1);
    assert!(echo.text.starts_with("前缀 "));
    assert!(echo.text.contains(&pasted));
    assert!(!echo.text.contains("[text "));

    let typed = "x\n".repeat(20);
    let plain_echo = ReplClipboardState::default().echo_text_for_submit(&typed);
    assert!(plain_echo.atoms.is_empty());
    assert_eq!(plain_echo.text, typed);
}

#[test]
fn text_block_uses_distinct_color_without_changing_width() {
    let mut state = ReplClipboardState::default();
    let mut input = String::new();
    let mut cursor = 0;
    state.paste_text_into_input(&mut input, &mut cursor, "a".repeat(LONG_TEXT_CHARS + 1));

    let styled =
        crate::cli::repl_input_render::style_clipboard_line(&input, 0, &state.block_spans(&input));
    assert!(styled.contains("\x1b[48;5;25m"));
    assert_eq!(
        crate::cli::repl_text::visible_width(&styled),
        input.chars().count()
    );
}

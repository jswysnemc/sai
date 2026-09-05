use super::*;
use crate::cli::repl_mentions::{find_mention_trigger, MentionSuggestion};
use crate::render::input_atom::{render_input_atoms, InputAtomKind};

/// 中文与附件混排时保留真实来源，全文展开包含全部粘贴正文。
#[test]
fn mixed_atoms_survive_submission_and_expand_without_guessing_labels() {
    let mut state = ReplClipboardState::default();
    let mut input = "说明 [ordinary] ".to_string();
    let mut cursor = input.chars().count();
    let body = "原始正文\n".repeat(30);
    state.paste_text_into_input(&mut input, &mut cursor, body.clone());
    state.insert_payload(
        &mut input,
        &mut cursor,
        ClipboardPayload::ImageDataUrl {
            data_url: "data:image/png;base64,abc".to_string(),
            width: 32,
            height: 32,
        },
    );
    input.push_str(" 末尾 [image 9 32x32]");

    let echo = state.echo_text_for_submit(&input);
    assert_eq!(echo.atoms.len(), 2);
    assert_eq!(&echo.text[echo.atoms[0].range.clone()], body.trim());
    let collapsed = render_input_atoms(&echo.text, &echo.atoms, false);
    assert!(collapsed.starts_with("说明 [ordinary] "));
    assert!(collapsed.contains(InputAtomKind::Text.style()));
    assert!(collapsed.contains(InputAtomKind::Image.style()));
    assert!(collapsed.ends_with("末尾 [image 9 32x32]"));
    assert!(!collapsed.contains("原始正文"));
    let expanded = render_input_atoms(&echo.text, &echo.atoms, true);
    assert!(expanded.contains(body.trim()));
    assert_eq!(
        state.to_chat_input(&input).image_url.as_deref(),
        Some("data:image/png;base64,abc")
    );
}

/// 引用补全登记为原子块，提交时恢复引用语法，删除时作为整体处理。
#[test]
fn completed_mentions_are_atomic_and_submit_original_reference() {
    for (text, kind) in [
        ("@src/main.rs", InputAtomKind::File),
        ("#test-skill", InputAtomKind::Skill),
    ] {
        let mut state = ReplClipboardState::default();
        let trigger = find_mention_trigger(text, text.chars().count()).unwrap();
        let item = MentionSuggestion {
            insert: text.to_string(),
            label: text.to_string(),
            description: String::new(),
            continue_filter: false,
        };
        let (mut input, mut cursor) = state.complete_mention(text, &trigger, &item);
        let echo = state.echo_text_for_submit(&input);
        assert_eq!(echo.atoms[0].kind, kind);
        assert_eq!(state.to_chat_input(&input).message, text);
        assert!(render_input_atoms(&echo.text, &echo.atoms, false).contains(kind.style()));
        cursor -= 1;
        assert!(state.remove_block_before_cursor(&mut input, &mut cursor));
        assert!(input.trim().is_empty());
    }
}

/// 展开一段粘贴正文时，正文内部的图片标签不参与下一次替换。
#[test]
fn pasted_marker_text_is_not_reinterpreted_as_an_attachment() {
    let mut state = ReplClipboardState::default();
    let mut input = String::new();
    let mut cursor = 0;
    let body = format!("[image 1 32x32] {}", "body ".repeat(60));
    state.paste_text_into_input(&mut input, &mut cursor, body.clone());
    state.insert_payload(
        &mut input,
        &mut cursor,
        ClipboardPayload::ImageDataUrl {
            data_url: "data:image/png;base64,abc".to_string(),
            width: 32,
            height: 32,
        },
    );
    let echo = state.echo_text_for_submit(&input);
    assert_eq!(&echo.text[echo.atoms[0].range.clone()], body.trim());
    assert_eq!(&echo.text[echo.atoms[1].range.clone()], "[image 1 32x32]");
    assert!(state.to_chat_input(&input).message.contains(body.trim()));
}

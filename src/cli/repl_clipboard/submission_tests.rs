use super::*;
use crate::cli::repl_mentions::{find_mention_trigger, MentionSuggestion};
use crate::render::input_atom::{render_input_atoms, InputAtomKind};

/// 【终端】【历史输入】提交、清空并重新读取历史后，原子块必须恢复完整正文和图片。
/// 参数: 无
/// 返回: 无，附件或原文丢失时断言失败
#[test]
fn regression_history_restores_pasted_text_and_image() {
    let temp = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(temp.path());
    let mut state = ReplClipboardState::default();
    let mut input = "说明 ".to_string();
    let mut cursor = input.chars().count();
    state.paste_text_into_input(&mut input, &mut cursor, "完整正文\n".repeat(50));
    state.insert_payload(
        &mut input,
        &mut cursor,
        ClipboardPayload::ImageDataUrl {
            data_url: "data:image/png;base64,original".into(),
            width: 32,
            height: 32,
        },
    );
    let expected = state.to_chat_input(&input);
    let echo = state.echo_text_for_submit(&input);
    crate::state::input_history::append_input_history_entry(&paths, &state.history_entry(&input))
        .unwrap();
    state.clear();
    let entries = crate::state::input_history::load_input_history_entries(&paths).unwrap();
    let restored = crate::cli::repl_input::history::restore_history_entry(entries.last().unwrap());
    let actual = restored.clipboard_state.to_chat_input(&restored.text);
    assert_eq!(actual.image_url, expected.image_url, "历史图片丢失");
    assert_eq!(actual.message, expected.message, "历史正文丢失");
    assert_eq!(
        restored
            .clipboard_state
            .echo_text_for_submit(&restored.text),
        echo
    );
}

/// 【终端】【历史输入】文件和技能恢复样式后仍可整体编辑，且不会修改保存的来源。
/// 参数: 无
/// 返回: 无，引用、计数或快照所有权错误时断言失败
#[test]
fn history_restores_references_and_keeps_original_snapshot_immutable() {
    let mut state = ReplClipboardState::default();
    let mut input = String::new();
    for reference in ["@src/main.rs", "#test-skill"] {
        input.push_str(reference);
        let trigger = find_mention_trigger(&input, input.chars().count()).unwrap();
        (input, _) = state.complete_mention(
            &input,
            &trigger,
            &MentionSuggestion {
                insert: reference.into(),
                label: reference.into(),
                description: String::new(),
                continue_filter: false,
            },
        );
    }
    let mut cursor = input.chars().count();
    state.paste_text_into_input(&mut input, &mut cursor, "first text\n".repeat(40));
    let snapshot = state.history_entry(&input);
    let preserved = snapshot.clone();
    let mut restored = crate::cli::repl_input::history::restore_history_entry(&snapshot);
    assert_eq!(
        restored.clipboard_state.block_spans(&restored.text).len(),
        3
    );
    let restored_echo = restored
        .clipboard_state
        .echo_text_for_submit(&restored.text);
    assert_eq!(restored_echo, state.echo_text_for_submit(&input));
    let styled = render_input_atoms(&restored_echo.text, &restored_echo.atoms, false);
    assert!(styled.contains(InputAtomKind::File.style()));
    assert!(styled.contains(InputAtomKind::Skill.style()));
    cursor = restored.text.chars().count();
    restored.clipboard_state.paste_text_into_input(
        &mut restored.text,
        &mut cursor,
        "second text\n".repeat(40),
    );
    assert!(restored.text.contains("[text 2 "));
    assert!(restored
        .clipboard_state
        .remove_block_before_cursor(&mut restored.text, &mut cursor));
    assert_eq!(snapshot, preserved);
    assert!(restored
        .clipboard_state
        .to_chat_input(&restored.text)
        .message
        .contains("@src/main.rs"));
    assert!(restored
        .clipboard_state
        .to_chat_input(&restored.text)
        .message
        .contains("#test-skill"));
}

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

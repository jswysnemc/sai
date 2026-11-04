use super::*;
use std::time::Duration;

/// 【终端】【粘贴回归】重放 ConPTY 的 CR、带修饰 LF 和释放事件，记录实际提交正文
/// 参数: text 为剪贴板原文，lf 为 ConPTY 使用的 LF 键码，available 为剪贴板是否可读
/// 返回: 粘贴后手动回车产生的提交列表
fn replay_conpty(text: &str, lf: KeyCode, available: bool) -> Vec<String> {
    let start = Instant::now();
    let mut state = WindowsPasteState::default();
    let mut clipboard = ReplClipboardState::default();
    let mut input = String::new();
    let mut cursor = 0;
    let mut submissions = Vec::new();
    for (index, ch) in text.chars().enumerate() {
        let now = start + Duration::from_millis(index as u64);
        let event = match ch {
            '\r' => KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            '\n' => KeyEvent::new(lf, KeyModifiers::CONTROL),
            '\t' => KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            other => KeyEvent::new(KeyCode::Char(other), KeyModifiers::NONE),
        };
        let handled = handle_key_with_reader(
            &mut state,
            &mut clipboard,
            &mut input,
            &mut cursor,
            event,
            now,
            || available.then(|| text.to_string()),
        );
        if !handled {
            match event.code {
                KeyCode::Enter => {
                    submissions.push(clipboard.expand_for_editor(&input).0.trim().to_string());
                    input.clear();
                    cursor = 0;
                    clipboard.clear();
                }
                KeyCode::Char('j') if event.modifiers == KeyModifiers::CONTROL => {
                    crate::cli::repl_text::insert_str_at_cursor(&mut input, &mut cursor, "\n");
                }
                KeyCode::Char(ch) => {
                    let text = ch.to_string();
                    state.record_text(&text, now);
                    crate::cli::repl_text::insert_str_at_cursor(&mut input, &mut cursor, &text);
                }
                KeyCode::Tab => panic!("粘贴制表符进入了补全或入队分支"),
                _ => panic!("非预期事件: {event:?}"),
            }
        }
        let release = KeyEvent::new_with_kind(event.code, event.modifiers, KeyEventKind::Release);
        assert!(!handle_key_with_reader(
            &mut state,
            &mut clipboard,
            &mut input,
            &mut cursor,
            release,
            now,
            || None,
        ));
    }
    // 1. 【终端】【粘贴回归】输入结束后用户单独回车，完整正文只能提交一次
    let now = start + Duration::from_millis(text.chars().count() as u64 + 300);
    assert!(!handle_key_with_reader(
        &mut state,
        &mut clipboard,
        &mut input,
        &mut cursor,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        now,
        || None,
    ));
    submissions.push(clipboard.expand_for_editor(&input).0.trim().to_string());
    submissions
}

/// 【终端】【ConPTY 回归】CRLF 中带 Control 的回车不能发送、重复插入或增加空行
/// 参数: 无；返回无，正文或提交次数不符时失败
#[test]
fn conpty_crlf_ctrl_enter_is_one_paste() {
    let text = format!(
        "{}\r\n\r\n第二段\t缩进\r\n第三段\r\n",
        "长文内容".repeat(100)
    );
    assert_eq!(
        replay_conpty(&text, KeyCode::Enter, true),
        vec![text.replace("\r\n", "\n").trim().to_string()],
    );
}

/// 【终端】【ConPTY 回归】Ctrl+J 不清除已确认的剪贴板回放，后续正文不重复
/// 参数: 无；返回无，正文或提交次数不符时失败
#[test]
fn conpty_crlf_ctrl_j_preserves_replay() {
    let text = "第一段\r\n第二段\r\n第三段\r\n";
    assert_eq!(
        replay_conpty(text, KeyCode::Char('j'), true),
        vec![text.replace("\r\n", "\n").trim().to_string()],
    );
}

/// 【终端】【ConPTY 回归】没有剪贴板匹配时，连续 CRLF 仍只插入一次换行
/// 参数: 无；返回无，保护失效或换行重复时失败
#[test]
fn conpty_crlf_without_clipboard_does_not_submit() {
    let text = "first\r\n\r\nsecond\r\nthird";
    for lf in [KeyCode::Enter, KeyCode::Char('j')] {
        assert_eq!(
            replay_conpty(text, lf, false),
            vec![text.replace("\r\n", "\n")],
        );
    }
}

/// 【终端】【ConPTY 回归】仅 LF 的剪贴板支持带修饰的换行，不吞掉独立空行
/// 参数: 无；返回无，换行或提交次数不符时失败
#[test]
fn conpty_lf_only_preserves_blank_lines() {
    let text = "first\n\nsecond\nthird";
    for lf in [KeyCode::Enter, KeyCode::Char('j')] {
        assert_eq!(replay_conpty(text, lf, true), vec![text.to_string()]);
    }
}

/// 【终端】【快捷键回归】普通输入暂停后 Ctrl+Enter、Ctrl+J 和 Ctrl+C 不属于粘贴
/// 参数: 无；返回无，正常快捷键被粘贴逻辑吞掉时失败
#[test]
fn normal_modified_shortcuts_keep_their_actions() {
    let start = Instant::now();
    for code in [KeyCode::Enter, KeyCode::Char('j'), KeyCode::Char('c')] {
        let mut state = WindowsPasteState::default();
        let mut clipboard = ReplClipboardState::default();
        let mut input = "typed".to_string();
        let mut cursor = 5;
        for (index, ch) in input.chars().enumerate() {
            state.record_text(
                &ch.to_string(),
                start + Duration::from_millis(index as u64 * 50),
            );
        }
        assert!(!handle_key_with_reader(
            &mut state,
            &mut clipboard,
            &mut input,
            &mut cursor,
            KeyEvent::new(code, KeyModifiers::CONTROL),
            start + Duration::from_millis(500),
            || None,
        ));
        assert_eq!(input, "typed");
    }
}

/// 【终端】【粘贴回归】回放期间 Ctrl+C 中断且清理状态，后续普通回车不被吞掉
/// 参数: 无；返回无，中断不可用或后续状态残留时失败
#[test]
fn interrupt_clears_confirmed_replay_and_pending_lf() {
    let now = Instant::now();
    let mut state = WindowsPasteState::default();
    state.record_text("first", now);
    let mut clipboard = ReplClipboardState::default();
    let mut input = "first".to_string();
    let mut cursor = 5;
    assert!(handle_key_with_reader(
        &mut state,
        &mut clipboard,
        &mut input,
        &mut cursor,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        now,
        || Some("first\r\nsecond".to_string()),
    ));
    assert!(!handle_key_with_reader(
        &mut state,
        &mut clipboard,
        &mut input,
        &mut cursor,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        now,
        || None,
    ));
    assert!(!handle_key_with_reader(
        &mut state,
        &mut clipboard,
        &mut input,
        &mut cursor,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        now,
        || None,
    ));
}

/// 【终端】【ConPTY 回归】已确认正文仍在回放时，剪贴板读取或绘制停顿不打断 CRLF
/// 参数: 无；返回无，延迟的配对 LF 清理回放时失败
#[test]
fn confirmed_crlf_survives_a_slow_clipboard_or_redraw() {
    let start = Instant::now();
    let mut state = WindowsPasteState::default();
    let mut clipboard = ReplClipboardState::default();
    let mut input = "first".to_string();
    let mut cursor = 5;
    state.record_text("first", start);
    assert!(handle_key_with_reader(
        &mut state,
        &mut clipboard,
        &mut input,
        &mut cursor,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        start,
        || Some("first\r\nsecond".to_string()),
    ));
    let after_delay = start + Duration::from_millis(500);
    assert!(handle_key_with_reader(
        &mut state,
        &mut clipboard,
        &mut input,
        &mut cursor,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        after_delay,
        || None,
    ));
    for ch in "second".chars() {
        assert!(handle_key_with_reader(
            &mut state,
            &mut clipboard,
            &mut input,
            &mut cursor,
            KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
            after_delay,
            || None,
        ));
    }
    assert_eq!(clipboard.expand_for_editor(&input).0, "first\nsecond");
}

/// 【终端】【快捷键回归】剪贴板首行碰巧相同，普通 Ctrl+Enter 仍只执行发送动作
/// 参数: 无；返回无，普通键入误读取剪贴板时失败
#[test]
fn normal_ctrl_enter_does_not_match_unrelated_clipboard() {
    let start = Instant::now();
    let mut state = WindowsPasteState::default();
    let mut clipboard = ReplClipboardState::default();
    let mut input = "typed".to_string();
    let mut cursor = 5;
    for (index, ch) in input.chars().enumerate() {
        state.record_text(
            &ch.to_string(),
            start + Duration::from_millis(index as u64 * 50),
        );
    }
    assert!(!handle_key_with_reader(
        &mut state,
        &mut clipboard,
        &mut input,
        &mut cursor,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        start + Duration::from_millis(220),
        || panic!("普通 Ctrl+Enter 不应读取剪贴板"),
    ));
    assert_eq!(input, "typed");
}

/// 【终端】【快捷键回归】已确认粘贴结束后，无须等待即可使用 Ctrl+Enter 发送
/// 参数: 无；返回无，已结束粘贴仍吞掉发送按键时失败
#[test]
fn ctrl_enter_is_available_immediately_after_confirmed_paste() {
    let now = Instant::now();
    let mut state = WindowsPasteState::default();
    state.record_text("first", now);
    let mut clipboard = ReplClipboardState::default();
    let mut input = "first".to_string();
    let mut cursor = 5;
    assert!(handle_key_with_reader(
        &mut state,
        &mut clipboard,
        &mut input,
        &mut cursor,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        now,
        || Some("first\r\nsecond\r\n".to_string()),
    ));
    let events = std::iter::once(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL))
        .chain(
            "second"
                .chars()
                .map(|ch| KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)),
        )
        .chain([
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        ]);
    for event in events {
        assert!(handle_key_with_reader(
            &mut state,
            &mut clipboard,
            &mut input,
            &mut cursor,
            event,
            now,
            || None,
        ));
    }
    assert!(!handle_key_with_reader(
        &mut state,
        &mut clipboard,
        &mut input,
        &mut cursor,
        KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
        now,
        || None,
    ));
    assert_eq!(clipboard.expand_for_editor(&input).0, "first\nsecond");
}

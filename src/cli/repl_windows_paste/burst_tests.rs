use super::*;

/// 【终端】【粘贴回归】重放剪贴板不可用时的 Windows 按键流，统计实际提交正文
/// 参数: text 为粘贴正文，clipboard 为不可用或不匹配的剪贴板；返回手动回车后的所有提交
fn replay_with_unmatched_clipboard(text: &str, clipboard: &str) -> Vec<String> {
    let start = Instant::now();
    let mut state = WindowsPasteState::default();
    let mut input = String::new();
    let mut cursor = 0;
    let mut submissions = Vec::new();
    for (index, ch) in text.chars().enumerate() {
        let now = start + Duration::from_millis(index as u64);
        match ch {
            '\n' | '\t' => {
                if ch == '\n' {
                    assert!(state.begin_multiline(&input, clipboard, now).is_none());
                }
                let key = if ch == '\n' {
                    WindowsPasteKey::Enter
                } else {
                    WindowsPasteKey::Tab
                };
                if !state.insert_burst_control(&mut input, &mut cursor, key, now) {
                    submissions.push(std::mem::take(&mut input));
                    cursor = 0;
                }
            }
            _ => {
                input.push(ch);
                cursor += 1;
                state.record_char(ch, now);
            }
        }
    }
    // 1. 【终端】【粘贴回归】暂停后再次回车必须正常提交完整正文
    assert!(!state.insert_burst_control(
        &mut input,
        &mut cursor,
        WindowsPasteKey::Enter,
        start + Duration::from_millis(text.chars().count() as u64 + 300),
    ));
    submissions.push(input);
    submissions
}

/// 【终端】【粘贴回归】剪贴板读取失败时，多行长文只在手动回车后提交一次
/// 参数: 无；返回无，粘贴期间出现提交时断言失败
#[test]
fn unavailable_clipboard_does_not_split_long_multiline_paste() {
    let text = format!("{}\n第二段\n第三段", "长文内容".repeat(100));
    assert_eq!(replay_with_unmatched_clipboard(&text, ""), vec![text]);
}

/// 【终端】【粘贴回归】空行、制表符和短首行都属于同一次粘贴
/// 参数: 无；返回无，控制键误提交时断言失败
#[test]
fn blank_lines_tabs_and_short_first_line_do_not_submit() {
    let text = "甲\n\n\t乙\n丙\n";
    assert_eq!(
        replay_with_unmatched_clipboard(text, ""),
        vec![text.to_string()]
    );
}

/// 【终端】【粘贴回归】系统剪贴板与当前粘贴来源不同时仍保持单次提交
/// 参数: 无；返回无，首行不匹配导致误提交时断言失败
#[test]
fn different_clipboard_does_not_disable_burst_protection() {
    let text = "实际首行\n实际第二行\n实际第三行";
    assert_eq!(
        replay_with_unmatched_clipboard(text, "其他内容\n其他第二行"),
        vec![text.to_string()]
    );
}

/// 【终端】【粘贴回归】普通键入没有形成突发输入，回车仍正常提交
/// 参数: 无；返回无，普通回车被误识别时断言失败
#[test]
fn normal_typing_does_not_suppress_enter_or_tab() {
    let start = Instant::now();
    let mut state = WindowsPasteState::default();
    for (index, ch) in "typed".chars().enumerate() {
        state.record_char(ch, start + Duration::from_millis(index as u64 * 50));
    }
    let mut input = "typed".to_string();
    let mut cursor = input.chars().count();
    for key in [WindowsPasteKey::Enter, WindowsPasteKey::Tab] {
        assert!(!state.insert_burst_control(
            &mut input,
            &mut cursor,
            key,
            start + Duration::from_millis(220),
        ));
    }
    assert_eq!(input, "typed");
}

/// 【终端】【粘贴回归】批量接收后的短暂停顿不允许换行自动提交
/// 参数: 无；返回无，Windows 事件分批到达导致误提交时断言失败
#[test]
fn batched_text_keeps_protection_across_short_event_gaps() {
    let start = Instant::now();
    let mut state = WindowsPasteState::default();
    state.record_text("first", start);
    let mut input = "first".to_string();
    let mut cursor = 5;
    assert!(state.insert_burst_control(
        &mut input,
        &mut cursor,
        WindowsPasteKey::Enter,
        start + Duration::from_millis(60),
    ));
    assert!(state.insert_burst_control(
        &mut input,
        &mut cursor,
        WindowsPasteKey::Enter,
        start + Duration::from_millis(150),
    ));
    assert_eq!(input, "first\n\n");
}

/// 【终端】【粘贴回归】在中文正文中插入换行保留后缀，编辑重置后回车可提交
/// 参数: 无；返回无，字符光标或状态清理错误时断言失败
#[test]
fn unicode_cursor_and_edit_reset_preserve_input() {
    let now = Instant::now();
    let mut state = WindowsPasteState::default();
    state.record_text("甲乙丙", now);
    let mut input = "甲乙丙后缀".to_string();
    let mut cursor = 3;
    assert!(state.insert_burst_control(&mut input, &mut cursor, WindowsPasteKey::Enter, now,));
    assert_eq!(input, "甲乙丙\n后缀");
    assert_eq!(cursor, 4);
    state.reset();
    assert!(!state.insert_burst_control(&mut input, &mut cursor, WindowsPasteKey::Enter, now,));
}

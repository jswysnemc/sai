use super::*;
use crossterm::event::KeyEvent;

/// 【终端图片】【应答样本】生成 crossterm 0.28 对 ASCII APC 的实际事件表示。
/// 参数: text 为协议文本，split_escape 指定 Esc 是否单独到达；返回: 对应事件列表。
fn events(text: &str, split_escape: bool) -> Vec<Event> {
    let mut result = Vec::new();
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        let (code, mut modifiers) = if ch == '\x1b' {
            if split_escape {
                (KeyCode::Esc, KeyModifiers::NONE)
            } else {
                (KeyCode::Char(chars.next().unwrap()), KeyModifiers::ALT)
            }
        } else {
            (KeyCode::Char(ch), KeyModifiers::NONE)
        };
        if matches!(code, KeyCode::Char(ch) if ch.is_uppercase()) {
            modifiers |= KeyModifiers::SHIFT;
        }
        result.push(Event::Key(KeyEvent::new(code, modifiers)));
    }
    result
}

/// 【终端图片】【成功应答】匹配查询编号和 OK，支持任意字节分块带来的 Esc 形式。
/// 参数: 无；返回: 无。
#[test]
fn accepts_matching_success_with_combined_or_split_escape() {
    for split in [false, true] {
        let mut response = ProbeResponse::new(31);
        let results: Vec<_> = events("\x1b_Gi=31;OK\x1b\\", split)
            .into_iter()
            .filter_map(|event| response.push(event))
            .collect();
        assert_eq!(results, [true]);
        assert!(response.into_input().is_empty());
    }
}

/// 【终端图片】【错误应答】失败状态及其他图片编号不能误报协议成功。
/// 参数: 无；返回: 无。
#[test]
fn rejects_errors_and_unrelated_ids() {
    let mut response = ProbeResponse::new(31);
    let unrelated = events("\x1b_Gi=32;OK\x1b\\", false);
    for event in unrelated.clone() {
        assert_eq!(response.push(event), None);
    }
    let results: Vec<_> = events("\x1b_Gi=31;EINVAL: unsupported format\x1b\\", false)
        .into_iter()
        .filter_map(|event| response.push(event))
        .collect();
    assert_eq!(results, [false]);
    assert!(response.into_input().is_empty());
}

/// 【终端图片】【输入顺序】保留查询前后以及应答中穿插的用户输入和窗口事件。
/// 参数: 无；返回: 无。
#[test]
fn preserves_input_around_and_inside_response() {
    let mut response = ProbeResponse::new(31);
    let before = events("draft\x1b", true);
    let during = [Event::Resize(90, 30), Event::Paste("中文内容".into())];
    let after = events("tail", false);
    for event in before.clone() {
        response.push(event);
    }
    for event in events("\x1b_Gi=31;", false) {
        response.push(event);
    }
    for event in during.clone() {
        response.push(event);
    }
    for event in events("OK\x1b\\", false) {
        response.push(event);
    }
    for event in after.clone() {
        response.push(event);
    }
    let expected: Vec<_> = before.into_iter().chain(during).chain(after).collect();
    assert_eq!(response.into_input(), expected);
}

/// 【终端图片】【超时清理】残缺应答不进入草稿，非协议的 Alt 输入完整保留。
/// 参数: 无；返回: 无。
#[test]
fn timeout_discards_only_identified_response() {
    for text in ["\x1b_Gi=31;O", "\x1b_Gi=31;OK\x1b"] {
        let mut response = ProbeResponse::new(31);
        for event in events(text, true) {
            response.push(event);
        }
        assert!(response.into_input().is_empty());
    }
    let mut response = ProbeResponse::new(31);
    let input = events("\x1b_", false);
    for event in input.clone() {
        response.push(event);
    }
    response.push(Event::Paste("text".into()));
    assert_eq!(
        response.into_input(),
        [input, vec![Event::Paste("text".into())]].concat()
    );
}

/// 【终端图片】【应答上限】过长响应有界失败，不作为键盘文本继续处理。
/// 参数: 无；返回: 无。
#[test]
fn rejects_oversized_response() {
    let mut response = ProbeResponse::new(31);
    let text = format!("\x1b_Gi=31;{}", "x".repeat(MAX_RESPONSE));
    let result = events(&text, false)
        .into_iter()
        .find_map(|event| response.push(event));
    assert_eq!(result, Some(false));
    assert!(response.into_input().is_empty());
}

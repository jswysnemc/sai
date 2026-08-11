use super::cell::HistoryCell;
use super::test_support::{chunk, options};
use super::TranscriptStore;
use crate::llm::ChatStreamKind;
use crate::render::activity_animation::strip_ansi_for_test;
use unicode_width::UnicodeWidthStr;

/// 【终端】【正文引导测试】验证定稿正文无引导点、仅等宽缩进，折行不超宽。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn finalized_assistant_body_uses_visual_guide_and_aligned_wraps() {
    let cell = HistoryCell::markdown("abcdefgh".to_string());

    let lines = cell.display_lines(4, &options());
    let plain = lines
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>();

    // 区块前空行 + 两列缩进正文（不再使用与工具相同的 •）
    assert_eq!(plain, vec!["", "  abcd", "  efgh"]);
    assert!(plain
        .iter()
        .all(|line| UnicodeWidthStr::width(line.as_str()) <= 6));
}

/// 【终端】【正文引导测试】验证流式正文与定稿正文使用相同的引导布局。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn live_and_finalized_assistant_body_share_visual_guide() {
    let mut store = TranscriptStore::new(100);
    store.push_chunk(&chunk(ChatStreamKind::Content, "first\nsecond\n"));

    let live = store
        .display_live_tail(80, &options())
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>();
    // live 与定稿共用区块前空行，避免「工作时无空行、完成后突然出现」
    assert_eq!(live, vec![String::new(), "  first".into(), "  second".into()]);

    assert!(store.finalize_live_tail());
    let finalized = store
        .display_tail(80, &options())
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(finalized, live);
}

use super::cell::HistoryCell;
use super::test_support::chunk;
use super::test_support::options;
use super::TranscriptStore;
use crate::llm::ChatStreamKind;
use crate::render::activity_animation::strip_ansi_for_test;
use unicode_width::UnicodeWidthStr;

/// 【终端】【引用折行】引用续行仍保留左侧引导线；无参数，无返回值
#[test]
fn wrapped_blockquote_keeps_gutter_on_every_row() {
    let cell = HistoryCell::markdown("> abcdefghijk".to_string());
    let lines = cell.display_lines(10, &options());
    let plain = lines
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(plain, ["", "  │ abcdefgh", "  │ ijk"]);
}

/// 【终端】【嵌套引用】各层竖线保持间距，空引用行连续；无参数，无返回值
#[test]
fn nested_quote_keeps_gutters_and_blank_lines() {
    let cell = HistoryCell::markdown("> > abcdefghi\n> >\n> end".to_string());
    let plain = cell
        .display_lines(10, &options())
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        plain,
        ["", "  │ │ abcdef", "  │ │ ghi", "  │ │ ", "  │ end"]
    );
}

/// 【终端】【引用重排】中文长行按显示列折行，宽度变化不丢正文；无参数，无返回值
#[test]
fn quote_reflows_cjk_at_current_width_without_losing_text() {
    let body = "引用正文包含中文以及English和更多内容";
    let mut store = TranscriptStore::new(100);
    store.push_chunk(&chunk(ChatStreamKind::Content, &format!("> {body}")));
    assert!(store.finalize_live_tail());
    for width in [8, 10, 17, 40, 10] {
        let plain = store
            .display_tail(width, &options())
            .iter()
            .map(|line| strip_ansi_for_test(line.as_str()))
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        assert!(
            plain
                .iter()
                .all(|line| UnicodeWidthStr::width(line.as_str()) <= width + 2),
            "width={width}: {plain:?}"
        );
        let reconstructed = plain
            .iter()
            .map(|line| {
                line.strip_prefix("  │ ")
                    .expect("quote gutter on every line")
            })
            .collect::<String>();
        assert_eq!(reconstructed, body);
    }
}

/// 【终端】【引用窄窗】嵌套层级与缩进不会占尽正文空间；无参数，无返回值
#[test]
fn deeply_nested_quote_reserves_room_for_cjk_text() {
    let cell = HistoryCell::markdown("  > > > > > 中文内容".to_string());
    let plain = cell
        .display_lines(8, &options())
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    assert!(plain
        .iter()
        .all(|line| UnicodeWidthStr::width(line.as_str()) <= 10));
    assert_eq!(
        plain
            .iter()
            .map(|line| line.strip_prefix("  │ │ │ ").unwrap())
            .collect::<String>(),
        "中文内容"
    );
}

/// 【终端】【引用流式】流式正文和完成后的历史记录布局一致；无参数，无返回值
#[test]
fn quote_streaming_matches_finalized_history() {
    let mut store = TranscriptStore::new(100);
    store.push_chunk(&chunk(ChatStreamKind::Content, "> 中文 **bold** tail "));
    store.push_chunk(&chunk(ChatStreamKind::Content, "continues here\n"));
    let live = store.display_live_tail(14, &options());
    assert!(store.finalize_live_tail());
    let history = store.display_tail(14, &options());
    assert_eq!(history, live);
    assert!(history
        .iter()
        .filter(|line| !strip_ansi_for_test(line.as_str()).is_empty())
        .all(|line| strip_ansi_for_test(line.as_str()).starts_with("  │ ")));
}

/// 【终端】【引用样式】正文保持正常对比度，加粗跨行仍生效；无参数，无返回值
#[test]
fn quote_body_and_inline_styles_survive_wrapping() {
    let cell = HistoryCell::markdown("> plain **abcdefghijk** tail".to_string());
    let lines = cell.display_lines(10, &options());
    let plain = lines
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>();
    assert!(lines[1].as_str().contains("\x1b[2m│ \x1b[0mplain "));
    let middle = plain
        .iter()
        .position(|line| line.contains("cdefghij"))
        .unwrap();
    assert!(lines[middle].as_str().contains("\x1b[1m"));
    assert!(lines.last().unwrap().as_str().contains("\x1b[0m"));
}

/// 【终端】【引用边界】代码块中的大于号保持原文，普通正文不带引用线；无参数，无返回值
#[test]
fn quote_gutter_does_not_spill_into_code_or_plain_text() {
    let cell = HistoryCell::markdown("> quote\n\nplain\n\n```text\n> literal\n```".to_string());
    let plain = cell
        .display_lines(20, &options())
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(plain.iter().filter(|line| line.contains('│')).count(), 1);
    assert!(plain.iter().any(|line| line == "  plain"));
    assert!(plain.iter().any(|line| line.contains("> literal")));
}

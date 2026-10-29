use super::{
    cell::HistoryCell,
    fullscreen_view::FullscreenDocument,
    test_support::{chunk, options},
    TranscriptMode, TranscriptStore,
};
use crate::config::TuiMode;
use crate::llm::ChatStreamKind;
use crate::render::{
    terminal_image::{reserved_image_rows, test_override},
    tui_mode,
};
use std::collections::HashSet;

const SVG: &str = "Diagram\n```svg\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"640\" height=\"240\"><rect width=\"640\" height=\"240\" fill=\"#1475aa\"/></svg>\n```\n";

/// 【终端】【SVG 占位测试】读取图像起始行及协议声明的占位高度。
/// 参数: `document` 为完整渲染文档；返回: 图像起始行和占位行数。
fn image_span(document: &FullscreenDocument) -> (usize, usize) {
    document
        .lines
        .iter()
        .enumerate()
        .find_map(|(row, line)| reserved_image_rows(line.as_str()).map(|height| (row, height)))
        .expect("SVG image placement")
}

/// 【终端】【SVG 占位测试】流式 SVG 位于正文末尾时仍预留全部行，不能裁掉图像下半部分的占位。
/// 参数: 无；返回: 无。
#[test]
fn svg_tail_keeps_all_image_rows() {
    test_override::set(Some(true), Some(false), Some(false));
    for style in [TuiMode::Inline, TuiMode::Fullscreen] {
        tui_mode::set_override(Some(style));
        let mut store = TranscriptStore::new(500);
        store.push_chunk(&chunk(ChatStreamKind::Content, SVG));
        let document = store.render_fullscreen(100, &options(), &HashSet::new());
        let (row, height) = image_span(&document);
        assert!(
            document.lines.len() >= row + height,
            "{style:?}: row={row}, height={height}, total={}",
            document.lines.len()
        );
    }
    tui_mode::set_override(None);
    test_override::set(None, None, None);
}

/// 【终端】【SVG 占位测试】定稿后的工作摘要与下一条用户消息均从 SVG 下方开始。
/// 参数: 无；返回: 无。
#[test]
fn svg_does_not_overlap_following_summary_or_user_message() {
    test_override::set(Some(true), Some(false), Some(false));
    for style in [TuiMode::Inline, TuiMode::Fullscreen] {
        tui_mode::set_override(Some(style));
        let mut store = TranscriptStore::new(500);
        store.push_chunk(&chunk(ChatStreamKind::Content, SVG));
        store.finalize_live_tail();
        store.push_cell(HistoryCell::meta("Worked for 20s".into()));
        store.push_user_echo(TranscriptMode::Yolo, "NEXT-USER".into());
        let document = store.render_fullscreen(100, &options(), &HashSet::new());
        let (row, height) = image_span(&document);
        for label in ["Worked for 20s", "NEXT-USER"] {
            let next = document
                .lines
                .iter()
                .position(|line| line.as_str().contains(label))
                .unwrap();
            assert!(
                next >= row + height,
                "{style:?}: image={row}..{}, {label}={next}",
                row + height
            );
        }
    }
    tui_mode::set_override(None);
    test_override::set(None, None, None);
}

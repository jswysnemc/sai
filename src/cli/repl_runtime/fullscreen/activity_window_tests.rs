use super::*;
use crate::render::transcript::{ParagraphKey, ParagraphSpan};

/// 【终端】【图片裁剪测试】内层滚动和外层裁剪组合时保留原始像素偏移及右侧位置。
/// 参数: 无；返回: 无。
#[test]
fn nested_image_windows_preserve_crop_offset_and_column() {
    let mut source = FullscreenDocument::default();
    source.lines.push(AnsiLine::new("title".into()));
    source.lines.push(AnsiLine::new(
        "                                        \x1b_Ga=p,q=2,C=1,i=77,p=33,c=24,r=8\x1b\\".into(),
    ));
    source
        .lines
        .extend((0..7).map(|_| AnsiLine::new(String::new())));
    let mut projection = DocumentProjection::new(&source);
    projection.copy(4..9);
    let visible = projection.finish();
    assert_eq!(visible.lines.len(), 5);
    let cell_ph = crate::render::terminal_image::kitty_cell_pixel_height();
    assert!(visible.lines[0]
        .as_str()
        .contains(&format!("y={}", 3 * cell_ph)));
    let outer = super::super::image_window::window_lines(&visible.lines, 1, 3, cell_ph);
    assert!(
        outer[0].contains(&format!("y={}", 4 * cell_ph)),
        "{}",
        outer[0]
    );
    assert!(outer[0].contains("\x1b[40C"));
    assert!(outer[0].contains(",r=3,"));
}

/// 【终端】【窗口测试】构造一个带标题和长正文的节点。
/// 参数: 无；返回: 完整文档与对应执行链范围。
fn source() -> (FullscreenDocument, ActivityRegion) {
    let mut document = FullscreenDocument {
        lines: (0..22)
            .map(|index| AnsiLine::new(format!("line-{index}")))
            .collect(),
        ..FullscreenDocument::default()
    };
    document.paragraphs.push(ParagraphSpan {
        key: ParagraphKey::whole(1),
        start: 1,
        end: 22,
        expanded: true,
        control: 1,
    });
    let region = ActivityRegion {
        key: ParagraphKey::whole(0),
        start: 0,
        end: 22,
        scrollable: true,
        active: true,
        footer: None,
    };
    (document, region)
}

/// 【终端】【窗口测试】跟随尾部时标题占用窗口内一行，最后一行详情仍可见。
/// 参数: 无；返回: 无。
#[test]
fn activity_window_pins_context_without_growing_viewport() {
    let (document, region) = source();
    let window = ActivityWindow::at(&document, &region, usize::MAX, 10);
    assert_eq!(window.context, Some(1));
    assert_eq!(window.rows, 13..22);
    assert_eq!(window.rows.len() + 1, 10);
    assert_eq!(window.max, 12);
    let first = ActivityWindow::at(&document, &region, 0, 10);
    assert_eq!(first.context, None);
    assert_eq!(first.rows, 1..11);
}

/// 【终端】【窗口测试】标题和正文合成同一段，点击正文使用可见标题作为锚点。
/// 参数: 无；返回: 无。
#[test]
fn activity_window_projection_merges_context_and_body() {
    let (document, region) = source();
    let window = ActivityWindow::at(&document, &region, usize::MAX, 10);
    let mut projection = DocumentProjection::new(&document);
    projection.copy(0..1);
    projection.copy(window.context.unwrap()..window.context.unwrap() + 1);
    projection.copy(window.rows);
    let visible = projection.finish();
    assert_eq!(visible.lines.len(), 11);
    assert_eq!(visible.paragraphs.len(), 1);
    assert_eq!(visible.paragraphs[0].start, 1);
    assert_eq!(visible.paragraphs[0].end, 11);
    assert_eq!(visible.paragraphs[0].control, 1);
    assert_eq!(visible.paragraph_at(6).unwrap().start, 1);
    assert_eq!(visible.lines[1].as_str(), "line-1");
    assert_eq!(visible.lines[10].as_str(), "line-21");
}

/// 【终端】【窗口测试】滚入下一个节点时替换上下文，不沿用前一个节点标题。
/// 参数: 无；返回: 无。
#[test]
fn activity_window_context_tracks_the_visible_node() {
    let (mut document, region) = source();
    document.paragraphs[0].end = 11;
    document.paragraphs.push(ParagraphSpan {
        key: ParagraphKey::whole(2),
        start: 11,
        end: 22,
        expanded: true,
        control: 11,
    });
    let at_title = ActivityWindow::at(&document, &region, 10, 10);
    assert_eq!(at_title.context, None);
    assert_eq!(at_title.rows.start, 11);
    let inside = ActivityWindow::at(&document, &region, 11, 10);
    assert_eq!(inside.context, Some(11));
}

/// 【终端】【窗口测试】极小窗口优先保留内容，不让标题占据唯一可见行。
/// 参数: 无；返回: 无。
#[test]
fn activity_window_handles_single_row() {
    let (document, region) = source();
    let window = ActivityWindow::at(&document, &region, usize::MAX, 1);
    assert_eq!(window.context, None);
    assert_eq!(window.rows, 21..22);
}

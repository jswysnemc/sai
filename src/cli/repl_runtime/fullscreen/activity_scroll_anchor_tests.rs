use super::*;
use crate::render::transcript::{ParagraphPart, ParagraphSpan};

const GROUP: ParagraphKey = ParagraphKey {
    cell: 1,
    part: ParagraphPart::ActivityGroup,
};
const STEP: ParagraphKey = ParagraphKey {
    cell: 2,
    part: ParagraphPart::Active,
};

/// 【终端】【锚点测试】同一段正文按指定宽度换行，标题和外层引导线不属于正文。
/// 参数: `width` 为正文净宽；返回: 完整文档与链范围。
fn source(width: usize) -> (FullscreenDocument, ActivityRegion) {
    let body = (0..30)
        .map(|index| format!("READ-{index:02} {}", "abcdefghij ".repeat(7)))
        .collect::<Vec<_>>()
        .join("\n");
    let mut document = FullscreenDocument {
        lines: vec![AnsiLine::new("GROUP".into()), AnsiLine::new("TITLE".into())],
        ..FullscreenDocument::default()
    };
    document.lines.extend(
        AnsiLine::wrap_block(&body, width)
            .into_iter()
            .map(|line| AnsiLine::new(format!("  \x1b[2m│\x1b[0m {}", line.as_str()))),
    );
    document.paragraphs.push(ParagraphSpan {
        key: STEP,
        start: 1,
        end: document.lines.len(),
        expanded: true,
        control: 1,
    });
    let region = ActivityRegion {
        key: GROUP,
        start: 0,
        end: document.lines.len(),
        scrollable: true,
        active: true,
        footer: None,
    };
    (document, region)
}

/// 【终端】【锚点测试】视觉行数改变后仍定位同一段正文，往返缩放不跳到早期内容。
/// 参数: 无；返回: 无。
#[test]
fn activity_scroll_anchor_preserves_content_across_reflow() {
    let (wide, group) = source(94);
    let first = wide
        .lines
        .iter()
        .position(|line| line.as_str().contains("READ-14"))
        .unwrap();
    let anchor = ActivityScrollAnchor::capture(&wide, &group, first - 1).unwrap();
    for width in [44, 30, 120, 94] {
        let (narrow, region) = source(width);
        let row = anchor.locate(&narrow, &region).unwrap() + 1;
        assert!(
            narrow.lines[row].as_str().contains("READ-14"),
            "width={width}, row={}",
            narrow.lines[row].as_str()
        );
    }
}

/// 【终端】【锚点测试】行内阅读位置在缩窄后对应同一字符片段，不回到节点开头。
/// 参数: 无；返回: 无。
#[test]
fn activity_scroll_anchor_preserves_position_inside_wrapped_line() {
    let (narrow, group) = source(30);
    let first = narrow
        .lines
        .iter()
        .position(|line| line.as_str().contains("READ-14"))
        .unwrap()
        + 1;
    let anchor = ActivityScrollAnchor::capture(&narrow, &group, first - 1).unwrap();
    let (wide, region) = source(94);
    let row = anchor.locate(&wide, &region).unwrap() + 1;
    assert!(wide.lines[row].as_str().contains("READ-14"));
}

/// 【终端】【锚点测试】标题位置不受耗时文案影响，节点消失后不把锚点转给其他节点。
/// 参数: 无；返回: 无。
#[test]
fn activity_scroll_anchor_handles_titles_and_missing_steps() {
    let (wide, group) = source(94);
    let title = ActivityScrollAnchor::capture(&wide, &group, 0).unwrap();
    let (mut narrow, region) = source(44);
    narrow.lines[1] = AnsiLine::new("TITLE · 42s".into());
    assert_eq!(title.locate(&narrow, &region), Some(0));
    narrow.paragraphs.clear();
    assert_eq!(title.locate(&narrow, &region), None);
}

/// 【终端】【锚点测试】同一内容位置的颜色和缩进不改变计数，空白行不增加字符偏移。
/// 参数: 无；返回: 无。
#[test]
fn activity_scroll_anchor_ignores_color_and_layout_whitespace() {
    assert_eq!(
        content_chars(&AnsiLine::new(
            "  \x1b[2m│\x1b[0m \x1b[32mabc def\x1b[0m".into()
        )),
        6
    );
    assert_eq!(content_chars(&AnsiLine::new("  │   \t".into())), 0);
}

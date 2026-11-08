use super::{layout::FullscreenLayout, paint, state::FullscreenState};
use crate::render::ansi_clip::strip_ansi;
use crate::render::panel_palette::{bg, PanelPalette};
use crate::render::transcript::{AnsiLine, FullscreenDocument, ParagraphKey, ParagraphSpan};

/// 【正文悬停】【块范围】展开正文悬停覆盖标题及内容，保留引导线颜色和相邻块。
/// 参数: 无；返回: 无。
#[test]
fn body_hover_tints_whole_block_without_recoloring_guides() {
    let layout = FullscreenLayout::compute(80, 24, 5);
    let guide = "\x1b[38;2;23;109;123m│";
    let mut state = FullscreenState::new();
    state.apply_document(
        FullscreenDocument {
            lines: vec![
                AnsiLine::new("\x1b[32m●\x1b[0m title".into()),
                AnsiLine::new(format!("{guide}\x1b[0m first")),
                AnsiLine::new(format!("{guide}\x1b[0m second")),
                AnsiLine::new("outside".into()),
            ],
            paragraphs: vec![ParagraphSpan {
                key: ParagraphKey::whole(0),
                start: 0,
                end: 3,
                expanded: true,
                control: 0,
            }],
            ..FullscreenDocument::default()
        },
        usize::from(layout.body_height),
    );
    state.pointer_row = Some(1);
    state.pointer_col = Some(4);
    let hovered = paint::compose(&state, &layout);
    let background = bg(PanelPalette::current().hover_bg);
    let top = usize::from(layout.body_top);
    for row in &hovered.rows[top..top + 3] {
        assert!(row.contains(&background));
    }
    assert!(!hovered.rows[top + 3].contains(&background));
    assert!(hovered.rows[top + 1].contains(guide));
    assert!(hovered.rows[top + 2].contains(guide));
    state.pointer_row = Some(2);
    assert_eq!(paint::compose(&state, &layout).rows, hovered.rows);
    state.pointer_row = None;
    let normal = paint::compose(&state, &layout);
    for row in &normal.rows[top..top + 3] {
        assert!(!row.contains(&background));
    }
    assert_eq!(
        strip_ansi(&normal.rows[top + 1]),
        strip_ansi(&hovered.rows[top + 1])
    );
    // 1. 【正文悬停】【视口裁剪】标题滚出视口时仍只高亮本块可见部分
    state.scroll = 1;
    state.pointer_row = Some(0);
    let clipped = paint::compose(&state, &layout);
    assert!(clipped.rows[top].contains(&background));
    assert!(clipped.rows[top + 1].contains(&background));
    assert!(!clipped.rows[top + 2].contains(&background));
}

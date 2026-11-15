use super::{layout::FullscreenLayout, paint, state::FullscreenState};
use crate::render::ansi_clip::strip_ansi;
use crate::render::transcript::{AnsiLine, FullscreenDocument, ParagraphKey, ParagraphSpan};

/// 【终端测试】【固定标题夹具】创建相邻工具卡片，第一张包含长代码正文。
/// 参数：无；返回：位于第一张正文中部的状态及屏幕布局。
fn fixture() -> (FullscreenState, FullscreenLayout) {
    let layout = FullscreenLayout::compute(80, 16, 5);
    let mut lines = vec![AnsiLine::new("Read file · sample.rs".into())];
    lines.extend((1..20).map(|row| {
        AnsiLine::new(format!(
            "  │ \x1b[32mlet\x1b[0m value_{row} = \x1b[34m{row}\x1b[0m;"
        ))
    }));
    lines.push(AnsiLine::new("Read file · next.rs".into()));
    lines.extend((21..40).map(|row| AnsiLine::new(format!("  │ next-{row}"))));
    let mut state = FullscreenState::new();
    state.apply_document(
        FullscreenDocument {
            lines,
            paragraphs: vec![
                ParagraphSpan {
                    key: ParagraphKey::whole(0),
                    start: 0,
                    end: 20,
                    expanded: true,
                    control: 0,
                },
                ParagraphSpan {
                    key: ParagraphKey::whole(1),
                    start: 20,
                    end: 40,
                    expanded: true,
                    control: 20,
                },
            ],
            ..FullscreenDocument::default()
        },
        usize::from(layout.body_height),
    );
    state.layout = Some(layout);
    state
        .expanded
        .extend([ParagraphKey::whole(0), ParagraphKey::whole(1)]);
    state.scroll = 5;
    state.follow = false;
    (state, layout)
}

/// 【终端测试】【固定标题】主视口滚动时标题保持顶部，到下一张卡片时替换标题。
/// 参数：无；返回：无。
#[test]
fn sticky_header_follows_the_current_tool_while_body_scrolls() {
    let (mut state, layout) = fixture();
    let top = usize::from(layout.body_top);
    for scroll in [1, 5, 19] {
        state.scroll = scroll;
        let rows = paint::compose(&state, &layout).rows;
        assert!(strip_ansi(&rows[top]).contains("Read file · sample.rs"));
        if scroll < 19 {
            assert!(strip_ansi(&rows[top + 1]).contains(&format!("value_{}", scroll + 1)));
        }
    }
    for scroll in [20, 21, 25] {
        state.scroll = scroll;
        assert!(
            strip_ansi(&paint::compose(&state, &layout).rows[top]).contains("Read file · next.rs")
        );
    }
}

/// 【终端测试】【悬停保色】正文悬停仅改变固定标题，代码、差异背景与宽度完全保留。
/// 参数：无；返回：无。
#[test]
fn sticky_header_hover_preserves_code_styles_byte_for_byte() {
    let (mut state, layout) = fixture();
    let top = usize::from(layout.body_top);
    state.document.lines[7] = AnsiLine::new(
        "  │ \x1b[48;2;20;46;30m\x1b[32mlet\x1b[0m value_7 = \x1b[34m7\x1b[0m;".into(),
    );
    let normal = paint::compose(&state, &layout).rows;
    state.pointer_row = Some(2);
    state.pointer_col = Some(10);
    let hovered = paint::compose(&state, &layout).rows;
    assert!(hovered[top].contains(&super::hover::hover_default_fg()));
    assert_eq!(&hovered[top + 1..], &normal[top + 1..]);
    assert!(hovered[top + 2].contains("\x1b[32m"));
    assert!(hovered[top + 2].contains("\x1b[34m"));
    assert!(hovered[top + 2].contains("\x1b[48;2;20;46;30m"));
    assert_eq!(strip_ansi(&hovered[top]), strip_ansi(&normal[top]));
}

/// 【终端测试】【标题命中】固定标题的点击与悬停指向原控制行，正文坐标保持不变。
/// 参数：无；返回：无。
#[test]
fn sticky_header_maps_clicks_to_its_original_control() {
    let (mut state, _) = fixture();
    assert_eq!(state.body_document_row(0), 0);
    assert_eq!(state.body_document_row(1), 6);
    state.pointer_row = Some(0);
    state.pointer_col = Some(6);
    assert_eq!(
        state.hovered_paragraph().unwrap().key,
        ParagraphKey::whole(0)
    );
    let row = state.body_document_row(0);
    assert_eq!(state.toggle_at(row).unwrap().0, ParagraphKey::whole(0));
    assert!(!state.expanded.contains(&ParagraphKey::whole(0)));
}

/// 【终端测试】【标题边界】单行视口与折叠段落不叠加标题，优先保留实际内容。
/// 参数：无；返回：无。
#[test]
fn sticky_header_leaves_single_row_and_collapsed_content_unchanged() {
    let (mut state, mut layout) = fixture();
    layout.body_height = 1;
    state.layout = Some(layout);
    assert!(state.sticky_paragraph().is_none());
    assert_eq!(state.body_document_row(0), 5);
    layout.body_height = 10;
    state.layout = Some(layout);
    state.document.paragraphs[0].expanded = false;
    assert!(state.sticky_paragraph().is_none());
    assert_eq!(state.body_document_row(0), 5);
}

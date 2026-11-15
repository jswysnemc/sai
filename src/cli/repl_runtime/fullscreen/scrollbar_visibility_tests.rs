//! 正文和详情滚动条仅改变显示、不改变预留宽度与命中区域。
use super::super::runtime_tests::{mouse, runtime};
use super::*;
use crossterm::event::{MouseButton, MouseEventKind};

/// 【终端】【跨栏拖动】参数：无；返回：无，验证正文滚动条拖入侧栏后仍跟随并正确释放
#[test]
fn main_scrollbar_keeps_capture_when_pointer_enters_sidebar() {
    use crate::cli::repl_runtime::viewport::TerminalSize;
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(20);
            runtime.enter_fullscreen().unwrap();
            let layout = runtime.fullscreen.as_ref().unwrap().state.layout.unwrap();
            let sidebar = layout.sidebar.unwrap();
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::Down(MouseButton::Left),
                    layout.scrollbar_col,
                    layout.body_top,
                ))
                .unwrap();
            assert!(runtime.fullscreen.as_ref().unwrap().state.dragging);
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::Drag(MouseButton::Left),
                    sidebar.left + 4,
                    layout.body_top + 10,
                ))
                .unwrap();
            let state = &runtime.fullscreen.as_ref().unwrap().state;
            assert_eq!(
                state.scroll,
                scroll_for_track_row(state, 10, usize::from(layout.body_height))
            );
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::Up(MouseButton::Left),
                    sidebar.left + 4,
                    layout.body_top + 10,
                ))
                .unwrap();
            assert!(!runtime.fullscreen.as_ref().unwrap().state.dragging);
        },
    );
}

/// 【终端】【正文显隐】悬停显示实心滑块，移出隐藏，捕获拖出仍显示，释放后隐藏。
#[test]
fn main_scrollbar_hover_drag_and_width() {
    crate::cli::repl_runtime::viewport::TerminalSize::with_size(
        crate::cli::repl_runtime::viewport::TerminalSize {
            cols: 100,
            rows: 30,
        },
        || {
            let mut runtime = runtime(20);
            runtime.enter_fullscreen().unwrap();
            let layout = runtime.fullscreen.as_ref().unwrap().state.layout.unwrap();
            let glyphs = |runtime: &crate::cli::repl_runtime::ReplRuntime| {
                let state = &runtime.fullscreen.as_ref().unwrap().state;
                let rows = compose(state, &layout).rows;
                for line in &rows[1..] {
                    assert_eq!(visible_width(line), usize::from(layout.main_width));
                }
                rows[1..]
                    .iter()
                    .map(|line| {
                        crate::render::ansi_clip::strip_ansi(line)
                            .chars()
                            .nth(usize::from(layout.scrollbar_col))
                            .unwrap()
                    })
                    .collect::<String>()
            };
            assert!(glyphs(&runtime).chars().all(|ch| ch == ' '));
            let row = layout.body_top + 2;
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Moved, layout.scrollbar_col, row))
                .unwrap();
            assert!(glyphs(&runtime).contains('█'));
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 3, row))
                .unwrap();
            assert!(glyphs(&runtime).chars().all(|ch| ch == ' '));
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::Down(MouseButton::Left),
                    layout.scrollbar_col,
                    row,
                ))
                .unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Drag(MouseButton::Left), 0, 29))
                .unwrap();
            assert!(glyphs(&runtime).contains('█'));
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Up(MouseButton::Left), 0, 29))
                .unwrap();
            assert!(glyphs(&runtime).chars().all(|ch| ch == ' '));
        },
    );
}

/// 【终端】【详情引导线】引导线常驻，轨道悬停只加宽固定长度滑块；等宽且不改源文档。
#[test]
fn detail_guide_stays_visible_and_hover_only_widens_thumb() {
    use crate::render::transcript::{ActivityRegion, AnsiLine, FullscreenDocument, ParagraphKey};
    let mut state = FullscreenState::new();
    let mut lines = (0..6)
        .map(|row| AnsiLine::new(format!("  │ output-{row}")))
        .collect::<Vec<_>>();
    super::super::block_scrollbar::decorate(&mut lines[1..], 0, 20);
    state.document = FullscreenDocument {
        lines,
        activities: vec![ActivityRegion {
            key: ParagraphKey::whole(1),
            start: 0,
            end: 6,
            scrollable: true,
            active: true,
            footer: None,
        }],
        ..Default::default()
    };
    let raw = state.document.lines.clone();
    let render = |state: &FullscreenState| {
        (1..6)
            .map(|row| {
                let line = super::super::block_scrollbar::paint_line(
                    state,
                    row,
                    state.document.lines[row].as_str(),
                );
                assert_eq!(
                    visible_width(&line),
                    visible_width(state.document.lines[row].as_str())
                );
                crate::render::ansi_clip::strip_ansi(&line)
                    .chars()
                    .nth(2)
                    .unwrap()
            })
            .collect::<String>()
    };
    assert_eq!(render(&state), "│││││");
    state.pointer_row = Some(1);
    state.pointer_col = Some(2);
    assert_eq!(render(&state), "███││");
    state.pointer_col = Some(5);
    assert_eq!(render(&state), "│││││");
    assert_eq!(state.document.lines, raw);
}

//! 正文和详情滚动条仅改变显示、不改变预留宽度与命中区域。
use super::super::runtime_tests::{mouse, runtime};
use super::*;
use crossterm::event::{MouseButton, MouseEventKind};

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

/// 【终端】【详情显隐】轨道悬停显示比例滑块，普通正文不显示；等宽且不改源文档。
#[test]
fn detail_scrollbar_is_hidden_until_gutter_hover() {
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
    assert_eq!(render(&state), "     ");
    state.pointer_row = Some(1);
    state.pointer_col = Some(2);
    assert_eq!(render(&state), "█││││");
    state.pointer_col = Some(5);
    assert_eq!(render(&state), "     ");
    assert_eq!(state.document.lines, raw);
}

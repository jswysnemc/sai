use super::runtime_tests::mouse;
use crate::cli::repl_runtime::viewport::TerminalSize;
use crate::cli::repl_runtime::ReplRuntime;
use crate::render::transcript::TranscriptRenderOptions;
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};
use crossterm::event::{MouseButton, MouseEventKind};

/// 【终端】【轨道捕获】拖动到正文外仍控制工具进度，释放不创建选区或折叠工具
#[test]
fn tool_scrollbar_drag_captures_and_releases_outside_body() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 100,
            rows: 30,
        },
        || {
            let mut runtime = ReplRuntime::new(
                1000,
                TranscriptRenderOptions {
                    reasoning_mode: ReasoningDisplayMode::Full,
                    tool_call_mode: ToolCallDisplayMode::Full,
                },
            );
            runtime.transcript.push_shell(
                "print".into(),
                (0..100).map(|index| format!("output-{index}\n")).collect(),
                Some(0),
            );
            runtime.enter_fullscreen().unwrap();
            let state = &runtime.fullscreen.as_ref().unwrap().state;
            let layout = state.layout.unwrap();
            let (row, col) = (state.scroll
                ..state
                    .document
                    .lines
                    .len()
                    .min(state.scroll + usize::from(layout.body_height)))
                .find_map(|row| {
                    let text =
                        crate::render::ansi_clip::strip_ansi(state.document.lines[row].as_str());
                    let col = text.chars().take_while(|ch| *ch == ' ').count();
                    (text.chars().nth(col) == Some('│'))
                        .then_some((layout.body_top + (row - state.scroll) as u16, col as u16))
                })
                .unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), col, row))
                .unwrap();
            assert!(runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .state
                .block_drag
                .is_some());
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Drag(MouseButton::Left), 99, 29))
                .unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Up(MouseButton::Left), 99, 29))
                .unwrap();
            let state = &runtime.fullscreen.as_ref().unwrap().state;
            assert!(state.block_drag.is_none());
            assert!(state.selection.is_none());
            assert!(state.press.is_none());
            assert!(state
                .document
                .lines
                .iter()
                .any(|line| line.as_str().contains("output-99")));
        },
    );
}

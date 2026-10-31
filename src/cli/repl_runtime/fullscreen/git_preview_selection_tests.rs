//! 差异正文拖选、复制和跨区域捕获的回归测试。

use super::git_preview_patch::parse;
use super::git_preview_view::PreviewLayout;
use super::runtime_tests::{chrome, mouse, runtime};
use super::selection::{Selection, TextPoint};
use crate::cli::repl_runtime::{viewport::TerminalSize, ReplRuntime};
use crossterm::event::{MouseButton, MouseEventKind};

/// 【审查测试】【选区夹具】创建已加载差异和输入草稿，不启动后台读取。
/// 参数: 无；返回: 固定审查布局的运行期。
fn review() -> ReplRuntime {
    let mut runtime = runtime(2);
    runtime
        .update_composer(&chrome(), "KEPT-DRAFT", 10, false, Vec::new(), 0)
        .unwrap();
    runtime.enter_fullscreen().unwrap();
    let directory = runtime.sidebar_directory();
    let preview = &mut runtime.fullscreen.as_mut().unwrap().sidebar.preview;
    preview.open(directory, "fixture.rs".into());
    preview.error = None;
    preview.content = Some(parse(
        "@@ -1,2 +1,3 @@\n-alpha beta\n+中文gamma\n+delta epsilon\n context\n",
        false,
    ));
    runtime.paint_fullscreen().unwrap();
    runtime
}

/// 【审查测试】【真实拖选】松开时复制代码，不包含行号、增删标记或左侧聊天。
/// 参数: 无；返回: 无，选区缺失或复制错误时断言失败。
#[test]
fn diff_drag_copies_code_and_preserves_chat_and_draft() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = review();
            let session = runtime.fullscreen.as_ref().unwrap();
            let geometry = PreviewLayout::compute(&session.state.layout.unwrap(), 0);
            let row = (geometry.diff_top + 1) as u16;
            let col = (geometry.left + 2 + 7) as u16;
            let scroll = session.state.scroll;
            for (kind, column) in [
                (MouseEventKind::Down(MouseButton::Left), col),
                (MouseEventKind::Drag(MouseButton::Left), col + 5),
                (MouseEventKind::Up(MouseButton::Left), col + 5),
            ] {
                runtime
                    .handle_fullscreen_event(&mouse(kind, column, row))
                    .unwrap();
            }
            let session = runtime.fullscreen.as_ref().unwrap();
            assert_eq!(
                session.state.copied,
                Some(5),
                "diff selection must copy alpha"
            );
            assert_eq!(session.state.scroll, scroll);
            assert!(session.state.selection.is_none() && session.state.press.is_none());
            assert!(!runtime.sidebar_control.dragging);
            let panel = super::git_preview_view::panel_rows(
                &session.sidebar.preview,
                &session.sidebar.git.summary,
                &session.state.layout.unwrap(),
            );
            assert!(panel[usize::from(row)].contains("\x1b[7m"));
            assert_eq!(
                runtime
                    .last_composer_signature
                    .as_ref()
                    .unwrap()
                    .input_geometry
                    .as_ref()
                    .unwrap()
                    .text,
                "KEPT-DRAFT"
            );
        },
    );
}

/// 【审查测试】【跨行方向】宽字符、反向拖动和代码缩进按源列提取，行号不进入剪贴板。
/// 参数: 无；返回: 无，复制内容错误时断言失败。
#[test]
fn multirow_drag_handles_wide_characters_in_both_directions() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            for backward in [false, true] {
                let mut runtime = review();
                let geometry = PreviewLayout::compute(
                    &runtime.fullscreen.as_ref().unwrap().state.layout.unwrap(),
                    0,
                );
                let col = (geometry.left + 9) as u16;
                let first = (col + 2, (geometry.diff_top + 2) as u16);
                let last = (col + 5, (geometry.diff_top + 3) as u16);
                let (anchor, head) = if backward {
                    (last, first)
                } else {
                    (first, last)
                };
                for (kind, (column, row)) in [
                    (MouseEventKind::Down(MouseButton::Left), anchor),
                    (MouseEventKind::Drag(MouseButton::Left), head),
                    (MouseEventKind::Up(MouseButton::Left), head),
                ] {
                    runtime
                        .handle_fullscreen_event(&mouse(kind, column, row))
                        .unwrap();
                }
                let preview = &runtime.fullscreen.as_ref().unwrap().sidebar.preview;
                assert_eq!(
                    super::git_preview_selection::selected_text(preview),
                    "文gamma\ndelta"
                );
            }
        },
    );
}

/// 【审查测试】【横向映射】滚动后选择仍定位源码，缩放保留选区而比较切换清除选区。
/// 参数: 无；返回: 无，源列错位或过期选区保留时断言失败。
#[test]
fn horizontal_selection_survives_resize_and_clears_on_mode_change() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = review();
            runtime
                .fullscreen
                .as_mut()
                .unwrap()
                .sidebar
                .preview
                .horizontal = 6;
            let geometry = PreviewLayout::compute(
                &runtime.fullscreen.as_ref().unwrap().state.layout.unwrap(),
                0,
            );
            let col = (geometry.left + 9) as u16;
            let row = (geometry.diff_top + 1) as u16;
            for (kind, column) in [
                (MouseEventKind::Down(MouseButton::Left), col),
                (MouseEventKind::Drag(MouseButton::Left), col + 4),
                (MouseEventKind::Up(MouseButton::Left), col + 4),
            ] {
                runtime
                    .handle_fullscreen_event(&mouse(kind, column, row))
                    .unwrap();
            }
            let span = Some(Selection {
                anchor: TextPoint { row: 1, col: 6 },
                head: TextPoint { row: 1, col: 10 },
            });
            assert_eq!(
                runtime
                    .fullscreen
                    .as_ref()
                    .unwrap()
                    .sidebar
                    .preview
                    .selection
                    .span,
                span
            );
            assert_eq!(
                super::git_preview_selection::selected_text(
                    &runtime.fullscreen.as_ref().unwrap().sidebar.preview
                ),
                "beta"
            );
            TerminalSize::with_size(
                TerminalSize {
                    cols: 100,
                    rows: 20,
                },
                || runtime.paint_fullscreen().unwrap(),
            );
            let preview = &mut runtime.fullscreen.as_mut().unwrap().sidebar.preview;
            assert_eq!(preview.selection.span, span);
            preview.set_mode(super::git_preview::DiffMode::Staged);
            assert!(preview.selection.span.is_none());
        },
    );
}

/// 【审查测试】【边缘隔离】拖到聊天区和底栏外只改变差异选区，不拖动分界线。
/// 参数: 无；返回: 无，聊天滚动或分栏宽度变化时断言失败。
#[test]
fn drag_capture_and_edge_scroll_stay_in_diff() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = review();
            runtime.fullscreen.as_mut().unwrap().sidebar.preview.content = Some(parse(
                &format!(
                    "@@ -0,0 +1,80 @@\n{}",
                    "+    preserved indentation\n".repeat(80)
                ),
                false,
            ));
            let session = runtime.fullscreen.as_ref().unwrap();
            let geometry = PreviewLayout::compute(&session.state.layout.unwrap(), 0);
            let chat_scroll = session.state.scroll;
            let width = session.state.layout.unwrap().main_width;
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::Down(MouseButton::Left),
                    (geometry.left + 9) as u16,
                    (geometry.diff_top + 1) as u16,
                ))
                .unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Drag(MouseButton::Left), 3, 29))
                .unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Up(MouseButton::Left), 110, 29))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert_eq!(session.sidebar.preview.scroll, 1);
            assert_eq!(session.state.scroll, chat_scroll);
            assert_eq!(session.state.layout.unwrap().main_width, width);
            assert!(!runtime.sidebar_control.dragging);
            assert!(
                super::git_preview_selection::selected_text(&session.sidebar.preview)
                    .starts_with("    preserved indentation\n")
            );
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 3, 1))
                .unwrap();
            assert!(runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .sidebar
                .preview
                .selection
                .span
                .is_none());
        },
    );
}

/// 【审查测试】【大行号】五位行号扩展排版栏后仍能精确选择代码，不复制行号。
/// 参数: 无；返回: 无，复制起点偏移时断言失败。
#[test]
fn selection_accounts_for_wide_line_numbers() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = review();
            runtime.fullscreen.as_mut().unwrap().sidebar.preview.content =
                Some(parse("@@ -10000 +10000 @@\n-old\n+alpha\n", false));
            let geometry = PreviewLayout::compute(
                &runtime.fullscreen.as_ref().unwrap().state.layout.unwrap(),
                0,
            );
            let col = (geometry.left + 10) as u16;
            let row = (geometry.diff_top + 2) as u16;
            for (kind, column) in [
                (MouseEventKind::Down(MouseButton::Left), col),
                (MouseEventKind::Drag(MouseButton::Left), col + 5),
                (MouseEventKind::Up(MouseButton::Left), col + 5),
            ] {
                runtime
                    .handle_fullscreen_event(&mouse(kind, column, row))
                    .unwrap();
            }
            assert_eq!(
                super::git_preview_selection::selected_text(
                    &runtime.fullscreen.as_ref().unwrap().sidebar.preview
                ),
                "alpha"
            );
        },
    );
}

/// 【审查测试】【释放归属】拖选后在顶部侧栏开关处释放仍完成复制，不切换侧栏。
/// 参数: 无；返回: 无，顶部控件抢走释放事件时断言失败。
#[test]
fn release_over_header_toggle_finishes_diff_selection() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = review();
            let layout = runtime.fullscreen.as_ref().unwrap().state.layout.unwrap();
            let geometry = PreviewLayout::compute(&layout, 0);
            let col = (geometry.left + 9) as u16;
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::Down(MouseButton::Left),
                    col,
                    (geometry.diff_top + 2) as u16,
                ))
                .unwrap();
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::Drag(MouseButton::Left),
                    col + 5,
                    (geometry.diff_top + 2) as u16,
                ))
                .unwrap();
            let toggle = super::header_controls::target(&layout).unwrap().start as u16;
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Up(MouseButton::Left), toggle, 0))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert!(!session.sidebar.preview.selection.is_pressed());
            assert_eq!(session.state.layout.unwrap().sidebar, layout.sidebar);
            assert!(session.state.copied.is_some());
        },
    );
}

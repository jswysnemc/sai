use super::runtime_tests::{mouse, runtime};
use super::selection::{selected_text, Selection, TextPoint};
use crossterm::event::{MouseButton, MouseEventKind};
use std::time::Instant;

/// 【全屏视图】【复制回归】拖选只更新可见选区，不重新分配未变化的正文。
///
/// 参数: 无
/// 返回: 无；正文缓存变化时断言失败
#[test]
fn reported_regression_selection_reuses_unchanged_document() {
    let mut runtime = runtime(5);
    runtime.enter_fullscreen().unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let layout = state.layout.unwrap();
    let lines = state.document.lines.as_ptr();
    let row = layout.body_top + 2;
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 3, row))
        .unwrap();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Drag(MouseButton::Left), 12, row))
        .unwrap();
    assert_eq!(
        runtime
            .fullscreen
            .as_ref()
            .unwrap()
            .state
            .document
            .lines
            .as_ptr(),
        lines,
        "dragging a selection must not rebuild the complete transcript"
    );
}

/// 【全屏视图】【复制回归】松开鼠标复制当前选区，正文缓存与选区内容保持一致。
///
/// 参数: 无
/// 返回: 无；复制字符数错误或重新分配正文时断言失败
#[test]
fn releasing_selection_reuses_document_and_copies_selected_text() {
    let mut runtime = runtime(5);
    runtime.enter_fullscreen().unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let layout = state.layout.unwrap();
    let lines = state.document.lines.as_ptr();
    let document_row = state.document.lines.iter().enumerate().skip(state.scroll)
        .find(|(_, line)| line.as_str().contains("turn-4-answer-")).unwrap().0;
    let row = layout.body_top + (document_row - state.scroll) as u16;
    let selection = Selection {
        anchor: TextPoint {
            row: document_row,
            col: 3,
        },
        head: TextPoint {
            row: document_row,
            col: 12,
        },
    };
    let expected = selected_text(
        &state.document.lines,
        &selection,
        usize::from(layout.content_width),
    );
    assert!(!expected.is_empty());
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        let column = if matches!(kind, MouseEventKind::Down(_)) {
            3
        } else {
            12
        };
        runtime
            .handle_fullscreen_event(&mouse(kind, column, row))
            .unwrap();
    }
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert_eq!(state.document.lines.as_ptr(), lines);
    assert_eq!(state.copied, Some(expected.chars().count()));
    assert_eq!(state.selection, Some(selection));
}

/// 【全屏视图】【复制基准】测量真实鼠标拖选和松开复制路径，排除首次正文排版。
///
/// 参数: 无
/// 返回: 无；输出不同会话长度下的鼠标事件耗时
#[test]
#[ignore = "手动运行复制性能基准，避免把机器速度作为普通回归断言"]
fn benchmark_fullscreen_copy_large_history() {
    for turns in [20, 400] {
        let mut runtime = runtime(turns);
        runtime.enter_fullscreen().unwrap();
        let state = &runtime.fullscreen.as_ref().unwrap().state;
        let layout = state.layout.unwrap();
        let line_count = state.document.lines.len();
        let row = layout.body_top + 2;
        let started = Instant::now();
        runtime
            .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 3, row))
            .unwrap();
        for column in 4..18 {
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::Drag(MouseButton::Left),
                    column,
                    row,
                ))
                .unwrap();
        }
        let drag_elapsed = started.elapsed();
        let released = Instant::now();
        runtime
            .handle_fullscreen_event(&mouse(MouseEventKind::Up(MouseButton::Left), 17, row))
            .unwrap();
        eprintln!(
            "copy benchmark: turns={turns} lines={line_count} drag_ms={:.3} release_ms={:.3}",
            drag_elapsed.as_secs_f64() * 1000.0,
            released.elapsed().as_secs_f64() * 1000.0
        );
    }
}

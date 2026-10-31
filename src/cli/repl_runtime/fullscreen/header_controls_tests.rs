//! 顶部侧栏开关和紧凑位置指示的回归测试。

use super::header_controls::{overlay, position, target, SIDEBAR_ICON};
use super::layout::FullscreenLayout;
use super::runtime_tests::{chrome, mouse, runtime};
use super::sidebar_control::{SidebarControl, Visibility};
use super::state::FullscreenState;
use crate::cli::repl_runtime::viewport::TerminalSize;
use crate::render::activity_animation::strip_ansi_for_test;
use crate::render::transcript::{AnsiLine, FullscreenDocument};
use crossterm::event::{MouseButton, MouseEventKind};

/// 【标题测试】【尺寸边界】开关与手动开启共用下限，矮屏或窄屏不绘制无效入口。
/// 参数: 无；返回: 无，入口越界或错误显示时断言失败。
#[test]
fn sidebar_toggle_only_exists_when_manual_open_is_possible() {
    for (cols, rows, visible) in [
        (80, 30, false),
        (87, 30, false),
        (160, 7, false),
        (88, 8, true),
        (100, 30, true),
        (160, 30, true),
    ] {
        let layout = FullscreenLayout::compute(cols, rows, 4);
        assert_eq!(target(&layout).is_some(), visible);
        let mut lines = vec!["x".repeat(usize::from(cols))];
        overlay(&SidebarControl::default(), &layout, &mut lines);
        let text = strip_ansi_for_test(&lines[0]);
        assert_eq!(
            crate::cli::repl_text::visible_width(&text),
            usize::from(cols)
        );
        assert_eq!(text.contains(SIDEBAR_ICON), visible);
        if let Some(range) = target(&layout) {
            assert_eq!(range.end, usize::from(cols) - 1);
            assert_eq!(range.len(), 3);
        }
    }
}

/// 【标题测试】【点击开关】关闭、开启均不发送消息，不清除草稿，配置关闭时仍可手动开启。
/// 参数: 无；返回: 无，状态错误或草稿损坏时断言失败。
#[test]
fn header_toggle_opens_and_closes_without_touching_draft() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(3);
            runtime.sidebar_control.visibility = Some(Visibility::Off);
            runtime
                .update_composer(&chrome(), "KEPT-DRAFT", 10, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            for shown in [true, false, true] {
                let layout = runtime.fullscreen.as_ref().unwrap().state.layout.unwrap();
                let col = target(&layout).unwrap().start as u16 + 2;
                runtime
                    .handle_fullscreen_event(&mouse(
                        MouseEventKind::Down(MouseButton::Left),
                        col,
                        0,
                    ))
                    .unwrap();
                runtime
                    .handle_fullscreen_event(&mouse(MouseEventKind::Up(MouseButton::Left), col, 0))
                    .unwrap();
                let session = runtime.fullscreen.as_ref().unwrap();
                assert_eq!(session.state.layout.unwrap().sidebar.is_some(), shown);
                assert!(!session.state.message_picker.active);
                assert!(session.state.press.is_none() && session.state.selection.is_none());
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
                assert_eq!(runtime.queue_len(), 0);
            }
        },
    );
}

/// 【标题测试】【悬停反馈】顶部开关只反色实际范围，离开时清除，保持背景和列宽。
/// 参数: 无；返回: 无，悬停残留或绘制串扰时断言失败。
#[test]
fn hover_reverses_only_toggle_and_clears_on_departure() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(2);
            runtime.enter_fullscreen().unwrap();
            let col = target(&runtime.fullscreen.as_ref().unwrap().state.layout.unwrap())
                .unwrap()
                .start as u16;
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Moved, col, 0))
                .unwrap();
            assert!(runtime.sidebar_control.header_hovered);
            let header = &runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .state
                .previous
                .as_ref()
                .unwrap()[0];
            assert!(header.contains(&format!("\x1b[7m {SIDEBAR_ICON} \x1b[27m")));
            assert!(!session_has_duplicate_button(&runtime));
            assert!(!header.contains("\x1b[4m"));
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 4, 2))
                .unwrap();
            assert!(!runtime.sidebar_control.header_hovered);
            let header = &runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .state
                .previous
                .as_ref()
                .unwrap()[0];
            assert!(!header.contains(&format!("\x1b[7m {SIDEBAR_ICON}")));
        },
    );
}

/// 【标题测试】【按钮去重】检测当前侧栏是否仍含旧关闭文字。
/// 参数: runtime 为全屏运行期；返回: 是否存在重复按钮。
fn session_has_duplicate_button(runtime: &crate::cli::repl_runtime::ReplRuntime) -> bool {
    runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .previous
        .as_ref()
        .unwrap()
        .iter()
        .any(|line| line.contains("[x]"))
}

/// 【标题测试】【位置指示】短轨道只反映主会话浏览位置，不添加快捷键或覆盖标题。
/// 参数: 无；返回: 无，位置计算错误时断言失败。
#[test]
fn position_tracks_top_middle_and_latest() {
    let layout = FullscreenLayout::compute(120, 30, 4);
    let mut state = FullscreenState::new();
    state.apply_document(
        FullscreenDocument {
            lines: vec![AnsiLine::new("line".into()); 100],
            ..Default::default()
        },
        usize::from(layout.body_height),
    );
    for (scroll, expected) in [
        (0, "0%"),
        (state.max_scroll(usize::from(layout.body_height)) / 2, "49%"),
        (state.max_scroll(usize::from(layout.body_height)), "100%"),
    ] {
        state.scroll = scroll;
        let text = strip_ansi_for_test(&position(&state, &layout));
        assert!(text.ends_with(expected), "{text}");
        assert_eq!(
            text.chars().filter(|character| *character == '●').count(),
            1
        );
        assert!(crate::cli::repl_text::visible_width(&text) <= 10);
    }
}

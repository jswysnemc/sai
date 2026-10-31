//! 侧栏开关、关闭控件与拖动尺寸的回归测试。

use super::git_preview_patch::parse;
use super::runtime_tests::{chrome, mouse, runtime};
use super::sidebar_action::SidebarAction;
use super::sidebar_control::{SidebarControl, Visibility};
use super::sidebar_git::GitSummary;
use crate::cli::repl_commands::{stream_command_policy, StreamCommandPolicy};
use crate::cli::repl_runtime::viewport::TerminalSize;
use crate::config::SidebarDefault;
use crate::render::ansi_clip::strip_ansi;
use crossterm::event::{MouseButton, MouseEventKind};

/// 【侧栏测试】【响应边界】验证自动断点、手动开启下限和两种分栏的尺寸夹紧。
/// 参数: 无；返回: 无。
#[test]
fn visibility_and_width_respect_available_space() {
    let mut control = SidebarControl::default();
    assert!(control.geometry(139, 30, false).is_none());
    assert!(control.geometry(160, 15, false).is_none());
    assert_eq!(control.geometry(160, 30, false).unwrap().width, 40);
    control.default = SidebarDefault::Off;
    assert!(control.geometry(240, 60, false).is_none());
    control.visibility = Some(Visibility::On);
    assert!(control.geometry(87, 30, false).is_none());
    assert!(control.geometry(120, 7, false).is_none());
    assert_eq!(control.geometry(100, 24, false).unwrap().width, 36);
    for width in [0, 28, 64, u16::MAX] {
        control.width = Some(width);
        control.review_width = Some(width);
        for cols in [8, 40, 88, 100, 160, 240] {
            for review in [false, true] {
                let layout = control.layout(cols, 30, 5, review);
                assert!(layout.main_width > 0 && layout.content_width > 0);
                if let Some(side) = layout.sidebar {
                    assert_eq!(side.left + side.width, cols);
                    assert_eq!(side.left, layout.main_width);
                    if cols >= 88 {
                        assert!(side.width >= 28);
                        assert!(layout.main_width >= if review { 40 } else { 60 });
                    }
                }
            }
        }
    }
}

/// 【侧栏测试】【尺寸恢复】终端缩窄只限制绘制宽度，放大后恢复上次手动宽度。
/// 参数: 无；返回: 无。
#[test]
fn resize_keeps_separate_width_preferences() {
    let control = SidebarControl {
        visibility: Some(Visibility::On),
        width: Some(70),
        review_width: Some(110),
        ..Default::default()
    };
    assert_eq!(control.geometry(100, 30, false).unwrap().width, 40);
    assert_eq!(control.geometry(200, 30, false).unwrap().width, 70);
    assert_eq!(control.geometry(100, 30, true).unwrap().width, 60);
    assert_eq!(control.geometry(200, 30, true).unwrap().width, 110);
}

/// 【侧栏测试】【命令覆盖】配置刷新及全屏切换不丢失手动开关，命令在生成期间可立即执行。
/// 参数: 无；返回: 无。
#[test]
fn commands_and_config_preserve_explicit_choices() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(0);
            runtime.configure_sidebar(SidebarDefault::Off);
            runtime.enter_fullscreen().unwrap();
            assert!(!runtime.sidebar_visible(160));
            assert!(runtime.handle_tui_command("/TUI sidebar ON").unwrap());
            assert!(runtime.sidebar_visible(160));
            runtime.configure_sidebar(SidebarDefault::Off);
            assert!(runtime.sidebar_visible(160));
            runtime.leave_fullscreen().unwrap();
            runtime.enter_fullscreen().unwrap();
            assert!(runtime.sidebar_visible(160));
            runtime.handle_tui_command("/tui sidebar off").unwrap();
            assert!(!runtime.sidebar_visible(240));
            runtime.configure_sidebar(SidebarDefault::Auto);
            assert!(!runtime.sidebar_visible(240));
            runtime.handle_tui_command("/tui sidebar auto").unwrap();
            assert!(runtime.sidebar_visible(160));
            assert!(!runtime.sidebar_visible(100));
            assert!(!runtime.handle_tui_command("/tuition sidebar off").unwrap());
            for invalid in ["/tui", "/tui sidebar bad", "/tui sidebar on extra"] {
                assert!(runtime.handle_tui_command(invalid).unwrap());
                assert_eq!(runtime.sidebar_control.visibility(), Visibility::Auto);
            }
            for command in [
                "/tui",
                "/tui sidebar on",
                "/tui sidebar off",
                "/TUI sidebar auto",
            ] {
                assert_eq!(
                    stream_command_policy(command),
                    StreamCommandPolicy::Immediate
                );
            }
            let candidates = crate::cli::repl_completion::arguments("/tui sidebar ", true);
            assert_eq!(candidates.len(), 3);
            assert!(candidates.iter().all(|candidate| !candidate.disabled));
            assert_eq!(
                crate::cli::repl_completion::accept("/tui", 4, 0, false, false).as_deref(),
                Some("/tui ")
            );
        },
    );
}

/// 【侧栏测试】【关闭控件】反色关闭按钮隐藏侧栏，尺寸变化不重开，草稿与左侧背景保持不变。
/// 参数: 无；返回: 无。
#[test]
fn close_button_hides_sidebar_without_losing_draft() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(2);
            runtime
                .update_composer(&chrome(), "KEPT-DRAFT", 4, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            let row = &session.state.previous.as_ref().unwrap()[0];
            assert!(strip_ansi(row).ends_with("[x] "));
            assert!(row.contains("\x1b[48;5;235m"));
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 157, 0))
                .unwrap();
            let row = &runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .state
                .previous
                .as_ref()
                .unwrap()[0];
            assert!(row.contains("\x1b[7m[x]"));
            assert!(!row.contains("\x1b[4m"));
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 157, 0))
                .unwrap();
            assert!(runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .state
                .layout
                .unwrap()
                .sidebar
                .is_none());
            assert!(!runtime.sidebar_visible(240));
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
            runtime.handle_tui_command("/tui sidebar on").unwrap();
            assert!(runtime.sidebar_visible(160));
        },
    );
}

/// 【侧栏测试】【拖动分流】拖动侧栏或审查分界线只重排左右区域，不创建文字选区。
/// 参数: 无；返回: 无。
#[test]
fn divider_drag_resizes_sidebar_and_review_independently() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(2);
            runtime
                .update_composer(&chrome(), "KEPT-DRAFT", 4, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 120, 5))
                .unwrap();
            assert!(runtime.sidebar_control.dragging);
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Drag(MouseButton::Left), 96, 20))
                .unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Up(MouseButton::Left), 96, 20))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert_eq!(session.state.layout.unwrap().main_width, 96);
            assert!(session.state.selection.is_none() && session.state.press.is_none());
            assert!(!runtime.sidebar_control.dragging);
            runtime.fullscreen.as_mut().unwrap().sidebar.git.summary = GitSummary {
                files: vec![(" M".into(), "test.rs".into())],
                ..Default::default()
            };
            runtime.apply_sidebar_action(SidebarAction::GitFile(0));
            runtime.fullscreen.as_mut().unwrap().sidebar.preview.content =
                Some(parse("@@ -1 +1 @@\n-old\n+new\n", false));
            runtime.paint_fullscreen().unwrap();
            assert_eq!(
                runtime
                    .fullscreen
                    .as_ref()
                    .unwrap()
                    .state
                    .layout
                    .unwrap()
                    .main_width,
                80
            );
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 80, 28))
                .unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Drag(MouseButton::Left), 70, 28))
                .unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Up(MouseButton::Left), 70, 28))
                .unwrap();
            assert_eq!(
                runtime
                    .fullscreen
                    .as_ref()
                    .unwrap()
                    .state
                    .layout
                    .unwrap()
                    .main_width,
                70
            );
            assert_eq!(runtime.sidebar_control.width, Some(64));
            assert_eq!(runtime.sidebar_control.review_width, Some(90));
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
            runtime.fullscreen.as_mut().unwrap().sidebar.preview.close();
            runtime.paint_fullscreen().unwrap();
            assert_eq!(
                runtime
                    .fullscreen
                    .as_ref()
                    .unwrap()
                    .state
                    .layout
                    .unwrap()
                    .main_width,
                96
            );
        },
    );
}

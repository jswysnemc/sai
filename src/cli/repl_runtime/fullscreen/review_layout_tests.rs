//! 固定分栏、左右独立样式和各区域独立滚动的回归测试。

use super::git_preview_patch::parse;
use super::git_preview_view::PreviewLayout;
use super::layout::FullscreenLayout;
use super::runtime_tests::{chrome, mouse, runtime};
use super::sidebar_action::{SidebarAction, SidebarSection};
use super::sidebar_git::GitSummary;
use super::sidebar_regions::regions;
use crate::cli::repl_runtime::viewport::TerminalSize;
use crate::render::activity_animation::strip_ansi_for_test;
use crossterm::event::MouseEventKind;

/// 【审查测试】【固定布局】所有尺寸都使用预留的右侧列，不在聊天上叠加窗口。
/// 参数: 无；返回: 无。
#[test]
fn review_uses_fixed_equal_columns_at_all_widths() {
    for cols in [40, 80, 100, 160, 240] {
        let layout = FullscreenLayout::compute_for(cols, 30, 4, true);
        let geometry = PreviewLayout::compute(&layout, 40);
        assert!(layout.review);
        assert_eq!(layout.main_width, cols / 2);
        assert_eq!(geometry.left, usize::from(layout.main_width));
        assert_eq!(geometry.left + geometry.width, usize::from(cols));
        assert!(geometry.diff_top + geometry.visible < usize::from(layout.rows));
    }
}

/// 【审查测试】【滚动分流】文件、差异和聊天滚动互不影响，所有标题保持位置不变。
/// 参数: 无；返回: 无。
#[test]
fn chat_files_and_diff_scroll_independently_without_hiding_chat() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 34,
        },
        || {
            let mut runtime = runtime(3);
            runtime
                .update_composer(&chrome(), "KEPT-DRAFT", 10, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            runtime.fullscreen.as_mut().unwrap().sidebar.git.summary = GitSummary {
                state: "ready".into(),
                files: (0..40)
                    .map(|index| (" M".into(), format!("file-{index}.rs")))
                    .collect(),
                ..Default::default()
            };
            runtime.paint_fullscreen().unwrap();
            runtime.apply_sidebar_action(SidebarAction::GitFile(0));
            let patch = format!(
                "@@ -0,0 +1,80 @@\n{}",
                (0..80)
                    .map(|index| format!("+CODE-{index}\n"))
                    .collect::<String>()
            );
            let preview = &mut runtime.fullscreen.as_mut().unwrap().sidebar.preview;
            preview.error = None;
            preview.content = Some(parse(&patch, false));
            runtime.paint_fullscreen().unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            let layout = session.state.layout.unwrap();
            let geometry = PreviewLayout::compute(&layout, 40);
            let screen = session.state.previous.as_ref().unwrap();
            let left = screen
                .iter()
                .map(|line| {
                    strip_ansi_for_test(line)
                        .chars()
                        .take(usize::from(layout.main_width))
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            assert!(left.contains("turn-2-answer"), "{left}");
            assert!(!left.contains("Review") && !left.contains("CODE-"));
            assert!(screen[0].contains("\x1b[48;5;235m"));
            let panel = super::git_preview_view::panel_rows(
                &session.sidebar.preview,
                &session.sidebar.git.summary,
                &layout,
            );
            assert!(panel[..geometry.diff_top]
                .iter()
                .all(|line| !line.contains("\x1b[48;")));
            assert!(panel[geometry.diff_top + 1].contains("\x1b[48;2;"));
            assert!(!panel.last().unwrap().contains("\x1b[48;"));
            let sticky = panel[geometry.diff_top - 2].clone();
            assert!(strip_ansi_for_test(&sticky).contains("file-0.rs"));
            let chat = session.state.scroll;
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::ScrollDown,
                    110,
                    geometry.files_top as u16,
                ))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert_eq!(session.sidebar.preview.file_scroll, 3);
            assert_eq!(session.sidebar.preview.scroll, 0);
            assert_eq!(session.state.scroll, chat);
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::ScrollDown,
                    110,
                    geometry.diff_top as u16,
                ))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert_eq!(session.sidebar.preview.file_scroll, 3);
            assert_eq!(session.sidebar.preview.scroll, 3);
            assert_eq!(session.state.scroll, chat);
            assert_eq!(
                super::git_preview_view::panel_rows(
                    &session.sidebar.preview,
                    &session.sidebar.git.summary,
                    &layout
                )[geometry.diff_top - 2],
                sticky
            );
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::ScrollUp, 10, 6))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert!(session.state.scroll < chat);
            assert_eq!(session.sidebar.preview.file_scroll, 3);
            assert_eq!(session.sidebar.preview.scroll, 3);
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 110, 0))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert_eq!(session.sidebar.preview.file_scroll, 3);
            assert_eq!(session.sidebar.preview.scroll, 3);
            assert!(
                strip_ansi_for_test(&session.state.previous.as_ref().unwrap()[0])
                    .contains("Review")
            );
        },
    );
}

/// 【侧栏测试】【分组局部滚动】Changes 列表滚动时，子任务、待办和三个标题均不移动。
/// 参数: 无；返回: 无。
#[test]
fn sidebar_sections_have_independent_scroll_offsets_and_fixed_headers() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(0);
            runtime.enter_fullscreen().unwrap();
            runtime.fullscreen.as_mut().unwrap().sidebar.git.summary = GitSummary {
                state: "ready".into(),
                files: (0..60)
                    .map(|index| (" M".into(), format!("file-{index}")))
                    .collect(),
                ..Default::default()
            };
            runtime.paint_fullscreen().unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            let layout = session.state.layout.unwrap();
            let panels = regions(&session.sidebar.document, usize::from(layout.composer_top));
            let git = panels
                .iter()
                .find(|region| region.section == SidebarSection::Git)
                .unwrap();
            let before = panels
                .iter()
                .map(|region| (region.section, region.top))
                .collect::<Vec<_>>();
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::ScrollDown,
                    156,
                    (git.top + 1) as u16,
                ))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert_eq!(session.sidebar.scrolls.git, 3);
            assert_eq!(session.sidebar.scrolls.agents, 0);
            assert_eq!(session.sidebar.scrolls.todo, 0);
            assert_eq!(
                regions(&session.sidebar.document, usize::from(layout.composer_top))
                    .iter()
                    .map(|region| (region.section, region.top))
                    .collect::<Vec<_>>(),
                before
            );
        },
    );
}

/// 【样式测试】【前景保留】去掉背景参数后，真实 RGB 前景与加粗仍完整保留。
/// 参数: 无；返回: 无。
#[test]
fn background_removal_preserves_foreground_color_components() {
    let text = "\x1b[48;5;235;38;2;40;107;48;1mtext\x1b[0m";
    let output = super::without_background(text);
    assert!(output.contains("38;2;40;107;48;1m"));
    assert!(!output.contains("48;5;235"));
    assert_eq!(strip_ansi_for_test(&output), "text");
}

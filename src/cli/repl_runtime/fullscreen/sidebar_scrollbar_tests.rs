//! 右侧滚动条的边界、独立滚动和交互隔离测试

use super::runtime_tests::{chrome, mouse, runtime};
use super::sidebar_action::SidebarSection;
use super::sidebar_git::GitSummary;
use super::sidebar_scrollbar::{scrollbars, ScrollTarget};
use crate::cli::repl_runtime::viewport::TerminalSize;
use crate::render::ansi_clip::strip_ansi;
use crossterm::event::{Event, MouseButton, MouseEventKind};

/// 【侧栏测试】【滑块拖动】概览仅滚动文件分组，轨道到达首尾，标题、聊天和草稿保持原位
/// 参数：无；返回：无，任一偏移或隔离断言失败时测试失败
#[test]
fn sidebar_scrollbar_hover_and_drag_preserve_other_regions() {
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
            runtime.fullscreen.as_mut().unwrap().sidebar.git.summary = GitSummary {
                state: "ready".into(),
                files: (0..60)
                    .map(|i| (" M".into(), format!("file-{i}.rs")))
                    .collect(),
                ..Default::default()
            };
            runtime.paint_fullscreen().unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            let layout = session.state.layout.unwrap();
            let bar = scrollbars(&session.sidebar, &layout)
                .into_iter()
                .find(|bar| bar.target == ScrollTarget::Section(SidebarSection::Git))
                .unwrap();
            let chat_scroll = session.state.scroll;
            let header = strip_ansi(&session.state.previous.as_ref().unwrap()[bar.top - 1]);
            let y = (bar.top + bar.thumb().start) as u16;
            let idle = session.state.previous.as_ref().unwrap()[usize::from(y)].clone();
            assert!(strip_ansi(&idle).ends_with('┃'));
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 159, y))
                .unwrap();
            assert_eq!(runtime.sidebar_control.scrollbar.hovered, Some(bar.target));
            assert_ne!(
                runtime
                    .fullscreen
                    .as_ref()
                    .unwrap()
                    .state
                    .previous
                    .as_ref()
                    .unwrap()[usize::from(y)],
                idle
            );
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 159, y))
                .unwrap();
            assert!(runtime.sidebar_control.scrollbar.dragging.is_some());
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Drag(MouseButton::Left), 120, 29))
                .unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Up(MouseButton::Left), 120, 29))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert_eq!(session.sidebar.scrolls.git, bar.total - bar.visible);
            assert_eq!(
                (
                    session.sidebar.scrolls.agents,
                    session.sidebar.scrolls.todo,
                    session.sidebar.scrolls.background
                ),
                (0, 0, 0)
            );
            assert_eq!(session.state.scroll, chat_scroll);
            assert!(session.state.selection.is_none() && session.state.press.is_none());
            assert!(runtime.sidebar_control.scrollbar.dragging.is_none());
            assert_eq!(
                strip_ansi(&session.state.previous.as_ref().unwrap()[bar.top - 1]),
                header
            );
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

/// 【侧栏测试】【轨道翻页】点击轨道只翻页，失焦与终端缩放清理交互，内容收缩移除滚动条
/// 参数：无；返回：无，滚动或清理行为错误时测试失败
#[test]
fn scrollbar_track_pages_and_focus_loss_clears_dragging() {
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
                    .map(|i| (" M".into(), format!("file-{i}.rs")))
                    .collect(),
                ..Default::default()
            };
            runtime.paint_fullscreen().unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            let bar = scrollbars(&session.sidebar, &session.state.layout.unwrap())[0];
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::Down(MouseButton::Left),
                    159,
                    (bar.top + bar.visible - 1) as u16,
                ))
                .unwrap();
            assert_eq!(
                runtime.fullscreen.as_ref().unwrap().sidebar.scrolls.git,
                bar.visible
            );
            assert!(runtime.sidebar_control.scrollbar.dragging.is_none());
            let session = runtime.fullscreen.as_ref().unwrap();
            let bar = scrollbars(&session.sidebar, &session.state.layout.unwrap())[0];
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::Down(MouseButton::Left),
                    159,
                    (bar.top + bar.thumb().start) as u16,
                ))
                .unwrap();
            runtime.handle_fullscreen_event(&Event::FocusLost).unwrap();
            assert!(runtime.sidebar_control.scrollbar.dragging.is_none());
            assert!(runtime.sidebar_control.scrollbar.hovered.is_none());
            runtime
                .fullscreen
                .as_mut()
                .unwrap()
                .sidebar
                .git
                .summary
                .files
                .clear();
            runtime.paint_fullscreen().unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert!(scrollbars(&session.sidebar, &session.state.layout.unwrap()).is_empty());
        },
    );
}

/// 【侧栏测试】【审查滚动】文件清单与差异正文各有轨道，拖动差异不改变文件或聊天位置
/// 参数：无；返回：无，分组偏移、选区和固定页脚错误时测试失败
#[test]
fn review_scrollbars_keep_file_and_diff_positions_independent() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(1);
            runtime
                .update_composer(&chrome(), "KEPT-DRAFT", 4, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            let directory = runtime.sidebar_directory();
            let session = runtime.fullscreen.as_mut().unwrap();
            session.sidebar.git.summary = GitSummary {
                state: "ready".into(),
                files: (0..60)
                    .map(|i| (" M".into(), format!("file-{i}.rs")))
                    .collect(),
                ..Default::default()
            };
            session.sidebar.preview.open(directory, "file-0.rs".into());
            session.sidebar.preview.content = Some(super::git_preview_patch::parse(
                &format!(
                    "@@ -1,80 +1,80 @@\n{}",
                    (0..80).map(|i| format!(" line-{i}\n")).collect::<String>()
                ),
                false,
            ));
            runtime.paint_fullscreen().unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            let layout = session.state.layout.unwrap();
            let bars = scrollbars(&session.sidebar, &layout);
            assert_eq!(bars.len(), 2);
            let files = bars
                .iter()
                .find(|bar| bar.target == ScrollTarget::ReviewFiles)
                .copied()
                .unwrap();
            let diff = bars
                .iter()
                .find(|bar| bar.target == ScrollTarget::ReviewDiff)
                .copied()
                .unwrap();
            let chat_scroll = session.state.scroll;
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::Down(MouseButton::Left),
                    159,
                    (files.top + files.visible - 1) as u16,
                ))
                .unwrap();
            assert_eq!(
                runtime
                    .fullscreen
                    .as_ref()
                    .unwrap()
                    .sidebar
                    .preview
                    .file_scroll,
                files.visible
            );
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::Down(MouseButton::Left),
                    159,
                    diff.top as u16,
                ))
                .unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Drag(MouseButton::Left), 159, 29))
                .unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Up(MouseButton::Left), 159, 29))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert_eq!(session.sidebar.preview.file_scroll, files.visible);
            assert_eq!(session.sidebar.preview.scroll, diff.total - diff.visible);
            assert_eq!(session.state.scroll, chat_scroll);
            assert!(session.sidebar.preview.selection.span.is_none());
            assert!(session.pending_copy.is_none());
            let mut full_rows = vec![" ".repeat(160); 30];
            super::sidebar_scrollbar::decorate(
                &scrollbars(&session.sidebar, &layout),
                &runtime.sidebar_control.scrollbar,
                &mut full_rows,
                159,
                0,
            );
            assert!(strip_ansi(&full_rows[diff.top + diff.visible - 1]).ends_with('┃'));
            assert!(strip_ansi(&full_rows[29]).trim().is_empty());
        },
    );
}

/// 【侧栏测试】【分界定位】正文同色竖线保持原样，真正分隔线悬停变色且宽度保持不变
/// 参数：无；返回：无，误改正文或终端列数变化时测试失败
#[test]
fn divider_feedback_targets_layout_column_and_preserves_text() {
    let palette = super::sidebar_palette::SidebarPalette::current();
    let first = format!(
        "{}│\x1b[0m 正文",
        super::sidebar_palette::fg(palette.border)
    );
    let column = crate::cli::repl_text::visible_width(&first);
    let mut rows = vec![format!(
        "{first}{}│\x1b[0m Sidebar",
        super::sidebar_palette::fg(palette.border)
    )];
    let original = strip_ansi(&rows[0]);
    let mut control = super::sidebar_control::SidebarControl::default();
    control.divider_hovered = true;
    super::sidebar_resize::decorate_divider(&control, &mut rows, column, 0, 30);
    assert!(rows[0].starts_with(&first));
    assert!(rows[0][first.len()..].contains(&super::sidebar_palette::fg(palette.accent)));
    assert_eq!(strip_ansi(&rows[0]), original);
    assert!(!rows[0].contains("\x1b[7m"));
}

/// 【侧栏测试】【短轨道】单行轨道不重置已有偏移，明暗主题的悬停底色分别降低和提高亮度
/// 参数：无；返回：无，短轨道或主题颜色错误时测试失败
#[test]
fn short_scrollbars_and_theme_feedback_have_stable_boundaries() {
    let bar = super::sidebar_scrollbar::Scrollbar {
        target: ScrollTarget::ReviewDiff,
        top: 5,
        visible: 1,
        total: 80,
        offset: 40,
    };
    assert_eq!(bar.offset_at(5, 0), 40);
    assert_eq!(bar.thumb(), 0..1);
    let light = super::sidebar_palette::SidebarPalette::for_background((250, 248, 246));
    let dark = super::sidebar_palette::SidebarPalette::for_background((20, 22, 24));
    assert!(light.hover_bg.0 < 250 && dark.hover_bg.0 > 20);
    assert!(light.border.0 < light.hover_bg.0);
    assert!(dark.border.0 > dark.hover_bg.0);
    let line = super::sidebar_widgets::highlight(
        &format!("{}+12\x1b[0m -3", super::sidebar_palette::fg(light.added)),
        false,
    );
    assert!(line.contains(&super::sidebar_palette::fg(light.added)));
    assert!(!line.contains("\x1b[7m"));
}

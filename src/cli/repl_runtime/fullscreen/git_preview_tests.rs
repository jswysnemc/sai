//! 文件差异、比较模式、界面组件与焦点隔离的回归测试。

use super::git_preview::{DiffMode, GitPreview, ReviewHover};
use super::git_preview_patch::{parse, LineKind};
use super::git_preview_view::PreviewLayout;
use super::layout::FullscreenLayout;
use super::runtime_tests::{chrome, mouse, runtime};
use super::sidebar_action::SidebarAction;
use super::sidebar_git::GitSummary;
use super::sidebar_palette::SidebarPalette;
use crate::cli::repl_runtime::{viewport::TerminalSize, FullscreenEvent, ReplRuntime};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use std::path::Path;

/// 【Git预览测试】【Git 辅助】在隔离仓库执行测试命令。
/// 参数: root 为工作目录，args 为 Git 参数；返回: 标准输出。
fn git(root: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// 【Git预览测试】【仓库准备】创建同时存在暂存与工作树改动的仓库。
/// 参数: 无；返回: 隔离目录。
fn repository() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    for args in [
        vec!["init"],
        vec!["config", "user.name", "Fixture"],
        vec!["config", "user.email", "fixture@example.invalid"],
    ] {
        git(root.path(), &args);
    }
    std::fs::write(root.path().join("tracked.txt"), "original\n").unwrap();
    git(root.path(), &["add", "tracked.txt"]);
    git(root.path(), &["commit", "-m", "test: preview fixture"]);
    std::fs::write(root.path().join("tracked.txt"), "staged\n").unwrap();
    git(root.path(), &["add", "tracked.txt"]);
    std::fs::write(root.path().join("tracked.txt"), "working\n").unwrap();
    std::fs::write(root.path().join("new.txt"), "untracked content\n").unwrap();
    root
}

/// 【Git预览测试】【等待读取】推动异步请求并验证加载完成，没有阻塞绘制线程。
/// 参数: preview 为状态，root 为工作目录；返回: 无。
async fn loaded(preview: &mut GitPreview, root: &Path) {
    for _ in 0..100 {
        preview.tick(root);
        if preview.content.is_some() || preview.error.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(preview.error.is_none(), "{:?}", preview.error);
    assert!(preview.content.is_some());
    assert!(preview.wait().is_none());
}

/// 【Git预览测试】【键盘辅助】通过真实事件入口发送浏览键。
/// 参数: runtime 为运行期，code 为键码；返回: 事件分流结果。
fn key(runtime: &mut ReplRuntime, code: KeyCode) -> FullscreenEvent {
    runtime
        .handle_fullscreen_event(&Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
        .unwrap()
}

/// 【Git预览测试】【结构化差异】解析双行号与增删内容，不显示 Git 控制头，不执行文本控制序列。
/// 参数: 无；返回: 无。
#[test]
fn patch_parser_produces_numbered_rows_and_removes_control_headers() {
    let content = parse(
        "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -2,2 +2,3 @@\n-old\n+new\n+++code\n context\n",
        false,
    );
    assert_eq!((content.added, content.removed), (2, 1));
    assert_eq!(content.lines[0].text, "@@ -2,2 +2,3 @@");
    assert_eq!(content.lines[1].kind, LineKind::Removed);
    assert_eq!(content.lines[1].old, Some(2));
    assert_eq!(content.lines[2].new, Some(2));
    assert_eq!(content.lines[3].text, "++code");
    assert_eq!(
        (content.lines[4].old, content.lines[4].new),
        (Some(3), Some(4))
    );
    assert!(!content
        .lines
        .iter()
        .any(|line| line.text.starts_with("diff --git")));
    let content = parse("@@ -0,0 +1 @@\n+bad\x1b[2J\ttext\n", false);
    assert!(!content.lines[1].text.contains('\x1b'));
    assert!(content.lines[1].text.contains("    text"));
}

/// 【Git预览测试】【差异分段】保留旧新范围及行数，多个片段重新定位行号，不混入函数说明。
/// 参数: 无；返回: 无。
#[test]
fn patch_sections_keep_standard_ranges_and_restart_line_numbers() {
    let content = parse(
        "@@ -119,2 +124,3 @@ fn render()\n-old\n+new\n context\n@@ -236 +245 @@ fn draw()\n-before\n+after\n",
        false,
    );
    assert_eq!(content.lines[0].text, "@@ -119,2 +124,3 @@");
    assert_eq!(content.lines[1].old, Some(119));
    assert_eq!(content.lines[2].new, Some(124));
    assert_eq!(content.lines[4].text, "@@ -236 +245 @@");
    assert_eq!(content.lines[5].old, Some(236));
    assert_eq!(content.lines[6].new, Some(245));
    assert!(content
        .lines
        .iter()
        .all(|line| !line.text.contains("Original line")));
}

/// 【Git预览测试】【比较模式】工作树、暂存区与未暂存比较均正确读取，不改变 Git 状态。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn loads_each_comparison_without_mutating_repository() {
    let root = repository();
    let before = git(root.path(), &["status", "--porcelain"]);
    let mut preview = GitPreview::default();
    preview.open(root.path().into(), "tracked.txt".into());
    assert!(preview.wait().is_some());
    loaded(&mut preview, root.path()).await;
    assert!(preview
        .content
        .as_ref()
        .unwrap()
        .lines
        .iter()
        .any(|line| line.text == "working" && line.kind == LineKind::Added));
    preview.set_mode(DiffMode::Staged);
    loaded(&mut preview, root.path()).await;
    assert!(preview
        .content
        .as_ref()
        .unwrap()
        .lines
        .iter()
        .any(|line| line.text == "staged" && line.kind == LineKind::Added));
    preview.set_mode(DiffMode::Unstaged);
    loaded(&mut preview, root.path()).await;
    assert!(preview
        .content
        .as_ref()
        .unwrap()
        .lines
        .iter()
        .any(|line| line.text == "staged" && line.kind == LineKind::Removed));
    preview.open(root.path().into(), "new.txt".into());
    loaded(&mut preview, root.path()).await;
    assert!(preview
        .content
        .as_ref()
        .unwrap()
        .lines
        .iter()
        .any(|line| line.text == "untracked content"));
    assert_eq!(git(root.path(), &["status", "--porcelain"]), before);
}

/// 【Git预览测试】【旧结果隔离】快速切换文件和工作区后不显示旧请求内容。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn switching_files_or_directories_discards_previous_request() {
    let root = repository();
    let mut preview = GitPreview::default();
    preview.open(root.path().into(), "tracked.txt".into());
    preview.open(root.path().into(), "new.txt".into());
    loaded(&mut preview, root.path()).await;
    assert_eq!(preview.path.as_deref(), Some("new.txt"));
    assert!(!preview
        .content
        .as_ref()
        .unwrap()
        .lines
        .iter()
        .any(|line| line.text == "working"));
    let other = tempfile::tempdir().unwrap();
    assert!(preview.tick(other.path()));
    assert!(preview.path.is_none() && preview.content.is_none() && preview.wait().is_none());
}

/// 【Git预览测试】【交互与焦点】文件条目可点击，浏览输入不修改会话，关闭后回到原状态。
/// 参数: 无；返回: 无。
#[test]
fn file_widgets_open_preview_and_keep_conversation_state() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(2);
            runtime
                .update_composer(&chrome(), "kept draft", 10, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            runtime.fullscreen.as_mut().unwrap().sidebar.git.summary = GitSummary {
                state: "ready".into(),
                files: vec![(" M".into(), "src/changed.rs".into())],
                ..Default::default()
            };
            runtime.paint_fullscreen().unwrap();
            let row = runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .sidebar
                .document
                .rows
                .iter()
                .position(|row| row.action == Some(SidebarAction::GitFile(0)))
                .unwrap();
            let before = runtime.fullscreen.as_ref().unwrap().state.scroll;
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::Down(MouseButton::Left),
                    156,
                    row as u16,
                ))
                .unwrap();
            assert_eq!(
                runtime
                    .fullscreen
                    .as_ref()
                    .unwrap()
                    .sidebar
                    .preview
                    .path
                    .as_deref(),
                Some("src/changed.rs")
            );
            assert_eq!(
                key(&mut runtime, KeyCode::Char('x')),
                FullscreenEvent::Consumed
            );
            assert_eq!(key(&mut runtime, KeyCode::Tab), FullscreenEvent::Consumed);
            assert_eq!(
                runtime.fullscreen.as_ref().unwrap().sidebar.preview.mode,
                DiffMode::Staged
            );
            key(&mut runtime, KeyCode::Esc);
            assert!(runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .sidebar
                .preview
                .path
                .is_none());
            assert_eq!(runtime.fullscreen.as_ref().unwrap().state.scroll, before);
            key(&mut runtime, KeyCode::F(6));
            assert!(runtime.fullscreen.as_ref().unwrap().sidebar.focused);
            assert_eq!(
                runtime
                    .handle_fullscreen_event(&Event::Paste("ignored paste".into()))
                    .unwrap(),
                FullscreenEvent::Consumed
            );
            assert_eq!(
                key(&mut runtime, KeyCode::Char('x')),
                FullscreenEvent::Consumed
            );
            key(&mut runtime, KeyCode::Esc);
            assert_eq!(key(&mut runtime, KeyCode::Char('x')), FullscreenEvent::Pass);
        },
    );
}

/// 【Git预览测试】【固定分栏】审查区只写入右侧，红蓝背景仅覆盖增删代码行。
/// 参数: 无；返回: 无。
#[test]
fn preview_layout_and_components_stay_within_body() {
    let light = SidebarPalette::for_background((255, 255, 255));
    let dark = SidebarPalette::for_background((0, 0, 0));
    assert_ne!(light.text, dark.text);
    for (cols, rows, wanted) in [(160, 30, 4), (100, 20, 4), (40, 16, 12)] {
        let layout = FullscreenLayout::compute_for(cols, rows, wanted, true);
        let geometry = PreviewLayout::compute(&layout, 1);
        let mut screen = vec![" ".repeat(usize::from(cols)); usize::from(layout.composer_top)];
        let mut preview = GitPreview::default();
        preview.path = Some("src/changed.rs".into());
        preview.content = Some(parse("@@ -1 +1 @@\n-before\n+after\n", false));
        let git = GitSummary {
            state: "ready".into(),
            files: vec![(" M".into(), "src/changed.rs".into())],
            ..Default::default()
        };
        super::git_preview_view::overlay(&preview, &git, &layout, &mut screen);
        for line in &screen {
            assert!(
                crate::cli::repl_text::visible_width(line) <= usize::from(cols),
                "{line:?}"
            );
        }
        let panel = super::git_preview_view::panel_rows(&preview, &git, &layout);
        assert!(panel[..geometry.diff_top]
            .iter()
            .all(|line| !line.contains("\x1b[48;")));
        assert!(!panel[geometry.diff_top].contains("Original line"));
        assert!(panel[geometry.diff_top].contains("@@ -1 +1 @@"));
        assert!(panel[geometry.diff_top + 1].contains("\x1b[48;2;"));
        assert!(panel[geometry.diff_top + 2].contains("\x1b[48;2;"));
        assert!(!panel.last().unwrap().contains("\x1b[48;"));
        assert_eq!(geometry.left, usize::from(layout.main_width));
        assert!(geometry.close_at((geometry.left + geometry.width - 3) as u16, 0));
    }
}

/// 【审查测试】【悬停反色】文件、模式和关闭按钮响应悬停，移到左侧后清除且不阻止聊天。
/// 参数: 无；返回: 无。
#[test]
fn review_controls_reverse_on_hover_and_clear_without_intercepting_chat() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(1);
            runtime.enter_fullscreen().unwrap();
            runtime.fullscreen.as_mut().unwrap().sidebar.git.summary = GitSummary {
                state: "ready".into(),
                files: vec![(" M".into(), "src/changed.rs".into())],
                ..Default::default()
            };
            runtime.paint_fullscreen().unwrap();
            runtime.apply_sidebar_action(SidebarAction::GitFile(0));
            runtime.paint_fullscreen().unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            let layout = session.state.layout.unwrap();
            let geometry = PreviewLayout::compute(&layout, 1);
            for (column, row, target) in [
                (90, geometry.files_top as u16, ReviewHover::File(0)),
                (85, 1, ReviewHover::Mode(DiffMode::Working)),
                (157, 0, ReviewHover::Close),
            ] {
                runtime
                    .handle_fullscreen_event(&mouse(MouseEventKind::Moved, column, row))
                    .unwrap();
                let session = runtime.fullscreen.as_ref().unwrap();
                assert_eq!(session.sidebar.preview.hovered, Some(target));
                let panel = super::git_preview_view::panel_rows(
                    &session.sidebar.preview,
                    &session.sidebar.git.summary,
                    &layout,
                );
                assert!(panel[usize::from(row)].contains("\x1b[7m"));
                assert!(!panel[usize::from(row)].contains("\x1b[4m"));
                runtime
                    .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 5, 5))
                    .unwrap();
                let session = runtime.fullscreen.as_ref().unwrap();
                assert!(session.sidebar.preview.hovered.is_none());
                let panel = super::git_preview_view::panel_rows(
                    &session.sidebar.preview,
                    &session.sidebar.git.summary,
                    &layout,
                );
                assert!(panel.iter().all(|line| !line.contains("\x1b[7m")));
            }
            assert_eq!(geometry.hover_at(80, 1, 0, 1), None);
            assert_eq!(
                geometry.hover_at(90, (geometry.diff_top - 2) as u16, 0, 1),
                None
            );
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::Moved,
                    90,
                    geometry.files_top as u16,
                ))
                .unwrap();
            runtime
                .handle_fullscreen_event(&mouse(
                    MouseEventKind::ScrollUp,
                    90,
                    geometry.files_top as u16,
                ))
                .unwrap();
            assert!(runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .sidebar
                .preview
                .hovered
                .is_none());
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 5, 5))
                .unwrap();
            assert_eq!(key(&mut runtime, KeyCode::Char('x')), FullscreenEvent::Pass);
        },
    );
}

/// 【审查测试】【矮屏固定标题】矮屏优先保留文件标题和正文，列表不挤占所有代码行。
/// 参数: 无；返回: 无。
#[test]
fn short_review_keeps_filename_and_room_for_diff() {
    for height in 9..16 {
        let layout = FullscreenLayout::compute_for(100, height, 4, true);
        let geometry = PreviewLayout::compute(&layout, 60);
        assert!(geometry.visible >= 2);
        let mut preview = GitPreview::default();
        preview.path = Some("src/changed.rs".into());
        let git = GitSummary {
            files: (0..60)
                .map(|index| (" M".into(), format!("file-{index}")))
                .collect(),
            ..Default::default()
        };
        let panel = super::git_preview_view::panel_rows(&preview, &git, &layout);
        assert!(panel[geometry.diff_top - 2].contains("src/changed.rs"));
    }
}

/// 【审查测试】【长路径】窄屏仍显示末尾文件名，横向滚动和换页不改变固定标题。
/// 参数: 无；返回: 无。
#[test]
fn long_filename_stays_visible_in_sticky_header_at_narrow_width() {
    let layout = FullscreenLayout::compute_for(80, 28, 4, true);
    let path = "src/a-very-long-directory/deeper-module/changed.rs";
    let git = GitSummary {
        files: vec![(" M".into(), path.into())],
        ..Default::default()
    };
    let geometry = PreviewLayout::compute(&layout, 1);
    let mut preview = GitPreview::default();
    preview.path = Some(path.into());
    preview.content = Some(parse(
        &format!(
            "@@ -0,0 +1,100 @@\n{}",
            "+long-code-line-for-horizontal-scroll\n".repeat(100)
        ),
        false,
    ));
    let before = super::git_preview_view::panel_rows(&preview, &git, &layout);
    assert!(before[geometry.files_top].contains("changed.rs"));
    assert!(before[geometry.diff_top - 2].contains("changed.rs"));
    preview.horizontal = 8;
    preview.scroll_by(12, geometry.visible);
    let after = super::git_preview_view::panel_rows(&preview, &git, &layout);
    assert_eq!(after[geometry.diff_top - 2], before[geometry.diff_top - 2]);
    assert_ne!(after[geometry.diff_top], before[geometry.diff_top]);
}

/// 【审查测试】【模式适配】宽屏和窄屏三种比较控件都不截断，绘制与鼠标命中使用同一宽度。
/// 参数: 无；返回: 无。
#[test]
fn mode_controls_fit_narrow_review_and_match_click_columns() {
    for cols in [40, 80, 100, 160] {
        let layout = FullscreenLayout::compute_for(cols, 28, 4, true);
        let geometry = PreviewLayout::compute(&layout, 1);
        let mut start = 0;
        for mode in [DiffMode::Working, DiffMode::Staged, DiffMode::Unstaged] {
            let width = geometry.mode_label(mode).len() + 4;
            assert_eq!(
                geometry.mode_at((geometry.left + 2 + start) as u16, 1),
                Some(mode)
            );
            start += width + 1;
        }
        assert!(start - 1 <= geometry.width - 3);
    }
}

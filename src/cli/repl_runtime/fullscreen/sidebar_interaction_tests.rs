//! 侧栏整行点击、分组折叠与悬停反馈的回归测试。

use super::runtime_tests::{chrome, mouse, runtime};
use super::sidebar_git::GitSummary;
use super::sidebar_style;
use crate::cli::repl_runtime::{viewport::TerminalSize, ReplRuntime};
use crate::render::activity_animation::strip_ansi_for_test;
use crossterm::event::{MouseButton, MouseEventKind};

/// 【侧栏测试】【标题定位】从实际文档中查找当前可见标题。
/// 参数: runtime 为运行期，label 为标题；返回: 屏幕行号。
fn heading(runtime: &ReplRuntime, label: &str) -> u16 {
    let sidebar = &runtime.fullscreen.as_ref().unwrap().sidebar;
    let index = sidebar
        .document
        .rows
        .iter()
        .position(|row| strip_ansi_for_test(&row.text).contains(label))
        .unwrap();
    sidebar
        .screen_row(
            &runtime.fullscreen.as_ref().unwrap().state.layout.unwrap(),
            index,
        )
        .unwrap()
}

/// 【侧栏测试】【全高内容】读取包括输入区右侧页脚的完整侧栏，不把正文绘制基线当成全屏快照。
/// 参数: runtime 为界面；返回: 全高侧栏行。
fn panel(runtime: &ReplRuntime) -> Vec<String> {
    let session = runtime.fullscreen.as_ref().unwrap();
    super::sidebar_view::panel_rows(
        &session.sidebar,
        &session.state.layout.unwrap(),
        &runtime.sidebar_control,
    )
}

/// 【侧栏测试】【整行点击】待办标题右侧留白也能切换，不改变草稿或正文状态。
/// 参数: 无；返回: 无。
#[test]
fn todo_heading_padding_is_clickable() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(2);
            runtime.transcript.push_tool_result("todo".into(), true,
            r#"{"items":[{"status":"in_progress","text":"CURRENT-TASK"},{"status":"pending","text":"NEXT-TASK"}]}"#.into());
            runtime
                .update_composer(&chrome(), "KEPT-DRAFT", 10, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            let before = runtime.todo_panel_compact;
            let scroll = runtime.fullscreen.as_ref().unwrap().state.scroll;
            let row = heading(&runtime, "Todo");
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 156, row))
                .unwrap();
            assert_ne!(runtime.todo_panel_compact, before);
            let session = runtime.fullscreen.as_ref().unwrap();
            assert_eq!(session.state.scroll, scroll);
            assert!(session.state.selection.is_none() && session.state.press.is_none());
        },
    );
}

/// 【侧栏测试】【Git 折叠】点击标题隐藏文件，再次点击恢复，其他分组保持可见。
/// 参数: 无；返回: 无。
#[test]
fn git_heading_collapses_and_restores_files() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(0);
            runtime
                .update_composer(&chrome(), "draft", 5, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            runtime.fullscreen.as_mut().unwrap().sidebar.git.summary = GitSummary {
                state: "ready".into(),
                branch: "feature/sidebar".into(),
                files: vec![(" M".into(), "src/changed.rs".into())],
                unstaged: 1,
                ..Default::default()
            };
            runtime.paint_fullscreen().unwrap();
            let row = heading(&runtime, "Changes");
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 124, row))
                .unwrap();
            let sidebar = &runtime.fullscreen.as_ref().unwrap().sidebar;
            assert!(!sidebar
                .document
                .rows
                .iter()
                .any(|row| row.text.contains("changed.rs")));
            assert!(sidebar
                .document
                .rows
                .iter()
                .any(|row| row.text.contains("Todo")));
            let row = heading(&runtime, "Changes");
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 156, row))
                .unwrap();
            assert!(runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .sidebar
                .document
                .rows
                .iter()
                .any(|row| row.text.contains("changed.rs")));
        },
    );
}

/// 【侧栏测试】【悬停提示】整行悬停显示动作说明，离开侧栏后取消高亮。
/// 参数: 无；返回: 无。
#[test]
fn heading_hover_explains_action_and_clears_on_exit() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(0);
            runtime
                .update_composer(&chrome(), "draft", 5, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            let row = heading(&runtime, "Changes");
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 156, row))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert_eq!(
                session
                    .sidebar
                    .target_at(&session.state.layout.unwrap(), 156, row),
                session.sidebar.hovered
            );
            let painted = session.state.previous.as_ref().unwrap();
            assert!(painted[usize::from(row)].contains("\x1b[7m"));
            assert!(!painted[usize::from(row)].contains("\x1b[4m"));
            assert!(panel(&runtime)
                .iter()
                .any(|line| line.contains("Click to collapse")));
            // 1. 【侧栏测试】【滚动边界】即使已在顶部，滚轮也必须清除原有悬停反馈
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::ScrollUp, 156, row))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert!(session.sidebar.hovered.is_none());
            assert!(!panel(&runtime)
                .iter()
                .any(|line| line.contains("Click to collapse")));
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 156, row))
                .unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 10, 1))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert!(session.sidebar.hovered.is_none());
            assert!(!panel(&runtime)
                .iter()
                .any(|line| line.contains("Click to collapse")));
        },
    );
}

/// 【侧栏测试】【命中边界】分隔线、只读详情和底部提示不触发动作。
/// 参数: 无；返回: 无。
#[test]
fn separator_details_and_footer_are_not_clickable() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(0);
            runtime.enter_fullscreen().unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            let layout = session.state.layout.unwrap();
            let sidebar = &session.sidebar;
            let left = layout.sidebar.unwrap().left;
            assert_eq!(sidebar.target_at(&layout, left + 1, 3), Some(0));
            assert_eq!(sidebar.target_at(&layout, 154, 3), Some(0));
            assert_eq!(sidebar.target_at(&layout, 156, 0), None);
            assert_eq!(sidebar.target_at(&layout, left, 0), None);
            assert_eq!(sidebar.target_at(&layout, 159, 0), None);
            assert_eq!(sidebar.target_at(&layout, 156, layout.rows - 1), None);
            let detail = heading(&runtime, "Checking working tree");
            assert_eq!(sidebar.target_at(&layout, 156, detail), None);
        },
    );
}

/// 【侧栏测试】【子任务折叠】隐藏子任务详情时保留主会话入口，再次展开恢复条目。
/// 参数: 无；返回: 无。
#[test]
fn subagents_can_fold_without_hiding_main_navigation() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(0);
            runtime
                .transcript
                .push_tool_call("subagent".into(), r#"{"description":"CHILD-TASK"}"#.into());
            runtime.transcript.push_tool_result(
                "subagent".into(),
                true,
                r#"{"subagent":{"id":"sidebar-fold","status":"completed"}}"#.into(),
            );
            runtime.enter_fullscreen().unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 154, 3))
                .unwrap();
            let sidebar = &runtime.fullscreen.as_ref().unwrap().sidebar;
            assert!(sidebar.sections.agents_collapsed);
            assert!(!sidebar
                .document
                .rows
                .iter()
                .any(|row| row.text.contains("CHILD-TASK")));
            assert!(sidebar.document.rows[1].text.contains("Main conversation"));
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 154, 3))
                .unwrap();
            assert!(runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .sidebar
                .document
                .rows
                .iter()
                .any(|row| row.text.contains("CHILD-TASK")));
        },
    );
}

/// 【侧栏测试】【滚动命中】文档偏移后整行点击仍对应可见条目，缩放不丢失折叠状态。
/// 参数: 无；返回: 无。
#[test]
fn scrolled_heading_uses_document_offset_and_retains_fold_on_resize() {
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
                files: (0..40)
                    .map(|index| (" M".into(), format!("file-{index}")))
                    .collect(),
                ..Default::default()
            };
            runtime.paint_fullscreen().unwrap();
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 156, 4))
                .unwrap();
            let row = heading(&runtime, "Changes");
            assert_eq!(row, 5);
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 156, row))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert!(session.sidebar.sections.git_collapsed);
            assert_eq!(session.sidebar.scrolls.git, 0);
            TerminalSize::with_size(
                TerminalSize {
                    cols: 100,
                    rows: 30,
                },
                || runtime.paint_fullscreen().unwrap(),
            );
            runtime.paint_fullscreen().unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert!(session.sidebar.sections.git_collapsed);
            assert!(!session
                .sidebar
                .document
                .rows
                .iter()
                .any(|row| row.text.contains("file-0")));
        },
    );
}

/// 【侧栏测试】【视觉列宽】面板标题计数右对齐，完整边框和内容不超出列宽。
/// 参数: 无；返回: 无。
#[test]
fn headings_and_paths_fit_sidebar_width() {
    for width in [32, 36, 40] {
        let title = super::sidebar_widgets::heading("Changes", "12", false, width);
        assert!(crate::cli::repl_text::visible_width(&title) <= width);
        if width >= 32 {
            assert_eq!(crate::cli::repl_text::visible_width(&title), width);
            assert!(strip_ansi_for_test(&title).ends_with("12"));
            assert_eq!(sidebar_style::clip(&title, width), title);
        }
        let item = super::sidebar_widgets::item("sidebar_view.rs", width, false);
        assert_eq!(crate::cli::repl_text::visible_width(&item), width);
        assert!(item.contains("sidebar_view.rs"));
    }
}

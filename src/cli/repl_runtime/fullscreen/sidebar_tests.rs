use super::layout::FullscreenLayout;
use super::runtime_tests::{chrome, mouse, runtime};
use super::sidebar_git::GitSummary;
use super::sidebar_view;
use crate::cli::repl_runtime::viewport::TerminalSize;
use crate::render::activity_animation::strip_ansi_for_test;
use crossterm::event::{MouseButton, MouseEventKind};

/// 【侧栏测试】【内容读取】获取当前全屏绘制结果。
/// 参数: runtime 为运行期；返回: 无终端样式的行。
fn screen(runtime: &crate::cli::repl_runtime::ReplRuntime) -> Vec<String> {
    runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .previous
        .as_ref()
        .unwrap()
        .iter()
        .map(|line| strip_ansi_for_test(line))
        .collect()
}

/// 【侧栏测试】【宽窄断点】宽屏保留正文净宽，窄屏和矮屏回退为原布局。
/// 参数: 无；返回: 无。
#[test]
fn sidebar_breakpoint_preserves_readable_conversation_width() {
    for cols in [80, 100, 139] {
        let layout = FullscreenLayout::compute(cols, 30, 4);
        assert!(layout.sidebar.is_none());
        assert_eq!(layout.main_width, cols);
    }
    for cols in [140, 160, 200, 300] {
        let layout = FullscreenLayout::compute(cols, 30, 4);
        let side = layout.sidebar.unwrap();
        assert!(layout.content_width >= 100);
        assert!((36..=44).contains(&side.width));
        assert_eq!(layout.scrollbar_col + 1, side.left);
        assert_eq!(side.left + side.width, cols);
        assert!(layout.in_sidebar(cols - 1, 0));
        assert!(!layout.in_sidebar(side.left - 1, 0));
        assert!(!layout.in_sidebar(side.left, layout.composer_top));
    }
    assert!(FullscreenLayout::compute(200, 15, 4).sidebar.is_none());
}

/// 【侧栏测试】【内容与缩放】三类概览出现在右侧，输入框不重复显示，缩窄后回到底部。
/// 参数: 无；返回: 无。
#[test]
fn wide_sidebar_moves_overviews_and_restores_bottom_at_narrow_width() {
    let mut runtime = runtime(2);
    runtime
        .transcript
        .push_tool_call("subagent".into(), r#"{"description":"CHILD-TASK"}"#.into());
    runtime.transcript.push_tool_result(
        "subagent".into(),
        true,
        r#"{"subagent":{"id":"sidebar-fixture","status":"completed"}}"#.into(),
    );
    runtime.transcript.push_tool_result("todo".into(), true, r#"{"items":[{"status":"in_progress","text":"CURRENT-TODO"},{"status":"pending","text":"NEXT-TODO"}]}"#.into());
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            runtime
                .update_composer(&chrome(), "KEPT-DRAFT", 10, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            let session = runtime.fullscreen.as_mut().unwrap();
            session.sidebar.git.summary = GitSummary {
                state: "ready".into(),
                branch: "feature/sidebar".into(),
                files: vec![(" M".into(), "src/example.rs".into())],
                unstaged: 1,
                ..Default::default()
            };
            runtime.paint_fullscreen().unwrap();
            let text = screen(&runtime).join("\n");
            for expected in [
                "Subagents",
                "Changes",
                "Todo",
                "CHILD-TASK",
                "example.rs",
                "CURRENT-TODO",
            ] {
                assert!(text.contains(expected), "{text}");
            }
            let bottom =
                strip_ansi_for_test(&runtime.composer.as_ref().unwrap().panel_lines().join("\n"));
            assert!(!bottom.contains("Todo") && !bottom.contains("subagents"));
            let side = runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .state
                .layout
                .unwrap()
                .sidebar
                .unwrap();
            for line in screen(&runtime) {
                assert!(unicode_width::UnicodeWidthStr::width(line.as_str()) <= 160);
            }
            assert!(screen(&runtime)[0]
                .chars()
                .skip(usize::from(side.left))
                .collect::<String>()
                .contains("Subagents"));
        },
    );
    runtime.paint_fullscreen().unwrap();
    assert!(runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .layout
        .unwrap()
        .sidebar
        .is_none());
    let bottom = strip_ansi_for_test(&runtime.composer.as_ref().unwrap().panel_lines().join("\n"));
    assert!(
        bottom.contains("Todo") && bottom.contains("subagents"),
        "{bottom}"
    );
}

/// 【侧栏测试】【交互隔离】侧栏独立滚动，子任务点击切换，待办点击只改变侧栏折叠状态。
/// 参数: 无；返回: 无。
#[test]
fn sidebar_scroll_and_click_do_not_move_or_select_conversation() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(3);
            runtime
                .transcript
                .push_tool_call("subagent".into(), r#"{"description":"CHILD-TASK"}"#.into());
            runtime.transcript.push_tool_result(
                "subagent".into(),
                true,
                r#"{"subagent":{"id":"sidebar-click","status":"completed"}}"#.into(),
            );
            runtime
                .update_composer(&chrome(), "draft", 5, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            let column = runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .state
                .layout
                .unwrap()
                .sidebar
                .unwrap()
                .left
                + 4;
            let before = runtime.fullscreen.as_ref().unwrap().state.scroll;
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), column, 2))
                .unwrap();
            assert_eq!(
                runtime.transcript.viewing_subagent_id(),
                Some("sidebar-click")
            );
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), column, 1))
                .unwrap();
            assert!(runtime.transcript.viewing_subagent_id().is_none());
            let session = runtime.fullscreen.as_mut().unwrap();
            session.sidebar.git.summary = GitSummary {
                state: "ready".into(),
                files: (0..60)
                    .map(|i| (" M".into(), format!("file-{i}")))
                    .collect(),
                unstaged: 60,
                ..Default::default()
            };
            runtime.paint_fullscreen().unwrap();
            let before = runtime
                .fullscreen
                .as_ref()
                .unwrap()
                .state
                .scroll
                .max(before);
            runtime
                .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, column, 4))
                .unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert_eq!(session.sidebar.scrolls.git, 3);
            assert!(session.state.selection.is_none() && session.state.press.is_none());
            assert_eq!(session.state.scroll, before);
            assert!(!session.state.dragging);
        },
    );
}

/// 【侧栏测试】【安全排版】文件名中的控制序列不能执行，待办全部可浏览。
/// 参数: 无；返回: 无。
#[test]
fn sidebar_sanitizes_user_text_and_keeps_full_todo_list() {
    let todos = (0..20)
        .map(|i| crate::render::transcript::TodoSnapshotItem {
            status: "pending".into(),
            text: format!("todo-{i}"),
        })
        .collect::<Vec<_>>();
    let git = GitSummary {
        state: "ready".into(),
        files: vec![("??".into(), "a\n\x1b[2Jb".into())],
        ..Default::default()
    };
    let document = sidebar_view::document(
        &[],
        &todos,
        &git,
        sidebar_view::SidebarView {
            viewing: None,
            compact: false,
            sections: &Default::default(),
            width: 32,
            frame: 0,
        },
    );
    let text = document
        .rows
        .iter()
        .map(|row| row.text.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(!text.contains("\x1b[2J"));
    assert!(text.contains("todo-19"));
    for row in document.rows {
        assert!(crate::cli::repl_text::visible_width(&row.text) <= 32);
    }
}

/// 【侧栏测试】【工作树作用域】查看隔离子任务时查询其目录，返回主会话后恢复主目录。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn git_directory_follows_viewed_subagent_worktree() {
    use crate::tools::subagent_state as state;
    let main = tempfile::tempdir().unwrap();
    let child = tempfile::tempdir().unwrap();
    let owner = main.path().to_string_lossy().into_owned();
    let (snapshot, _cancel) =
        state::create_subagent_for_owner(&owner, "worktree task".into(), "general".into(), 8);
    state::set_subagent_worktree(
        &snapshot.id,
        Some(child.path().to_string_lossy().into_owned()),
        Some("child-branch".into()),
        Some(owner.clone()),
    );
    let mut runtime = runtime(0);
    runtime.transcript.push_tool_call(
        "subagent".into(),
        r#"{"description":"worktree task"}"#.into(),
    );
    runtime.transcript.push_tool_result(
        "subagent".into(),
        true,
        serde_json::json!({"subagent":{"id":snapshot.id,"status":"running"}}).to_string(),
    );
    crate::runtime_cwd::scope(main.path().into(), async {
        assert_eq!(runtime.sidebar_directory(), main.path());
        assert!(runtime.transcript.enter_subagent_view(0));
        assert_eq!(runtime.sidebar_directory(), child.path());
        runtime.transcript.exit_subagent_view();
        assert_eq!(runtime.sidebar_directory(), main.path());
    })
    .await;
    state::clear_subagents_for_owner(&owner);
}

//! 后台任务侧栏显示与入口交互的回归测试。

use super::runtime_tests::{chrome, mouse, runtime};
use super::sidebar_action::{SidebarAction, SidebarSection};
use crate::cli::repl_runtime::{FullscreenEvent, ReplRuntime, viewport::TerminalSize};
use crate::render::ansi_clip::strip_ansi;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};

/// 【后台任务测试】【屏幕读取】读取完整侧栏，不依赖正文或输入区的绘制缓存。
/// 参数: runtime 为运行期；返回: 去除样式后的侧栏文本。
fn sidebar_text(runtime: &ReplRuntime) -> String {
    let session = runtime.fullscreen.as_ref().unwrap();
    strip_ansi(
        &super::sidebar_view::panel_rows(
            &session.sidebar,
            &session.state.layout.unwrap(),
            &runtime.sidebar_control,
        )
        .join("\n"),
    )
}

/// 【后台任务测试】【按键发送】通过真实全屏事件分发器执行按键。
/// 参数: runtime 为运行期，code 为键码；返回: 无。
fn key(runtime: &mut ReplRuntime, code: KeyCode) {
    assert_eq!(
        runtime
            .handle_fullscreen_event(&Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
            .unwrap(),
        FullscreenEvent::Consumed
    );
}

/// 【后台任务测试】【入口点击】从实际可见行查找操作，确保绘制与鼠标命中一致。
/// 参数: runtime 为运行期，action 为目标动作；返回: 无。
fn click(runtime: &mut ReplRuntime, action: SidebarAction) {
    let session = runtime.fullscreen.as_ref().unwrap();
    let layout = session.state.layout.unwrap();
    let index = session
        .sidebar
        .document
        .rows
        .iter()
        .position(|row| row.action == Some(action))
        .unwrap();
    let row = session.sidebar.screen_row(&layout, index).unwrap();
    assert_eq!(
        runtime
            .handle_fullscreen_event(&mouse(
                MouseEventKind::Down(MouseButton::Left),
                layout.cols - 4,
                row
            ))
            .unwrap(),
        FullscreenEvent::Consumed
    );
}

/// 【后台任务测试】【侧栏入口】没有运行任务时仍显示管理入口，避免功能无法发现。
/// 参数: 无；返回: 无。
#[test]
fn background_manager_is_visible_in_sidebar_without_tasks() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(0);
            runtime
                .update_composer(&chrome(), "kept draft", 10, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            let text = sidebar_text(&runtime);
            assert!(text.contains("Background tasks"), "{text}");
            assert!(text.contains("Manage tasks"), "{text}");
        },
    );
}

/// 【后台任务测试】【浮层交接】鼠标和键盘均能打开管理器，关闭后恢复草稿、滚动和侧栏。
/// 参数: 无；返回: 无。
#[test]
fn background_sidebar_opens_manager_without_submitting_or_losing_draft() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(3);
            runtime
                .update_composer(&chrome(), "kept draft", 4, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            let signature = runtime.last_composer_signature.clone();
            let scroll = runtime.fullscreen.as_ref().unwrap().state.scroll;
            click(&mut runtime, SidebarAction::Background(None));
            assert!(runtime.background_panel_active());
            let text = strip_ansi(
                &runtime
                    .background_panel
                    .previous
                    .as_ref()
                    .unwrap()
                    .join("\n"),
            );
            assert!(text.contains("Background tasks"), "{text}");
            assert!(text.contains("Esc"), "{text}");
            assert_eq!(
                runtime
                    .handle_fullscreen_event(&Event::Paste("not submitted".into()))
                    .unwrap(),
                FullscreenEvent::Consumed
            );
            runtime.draw_composer().unwrap();
            assert!(runtime.last_composer_signature.is_none());
            key(&mut runtime, KeyCode::Esc);
            assert!(!runtime.background_panel_active());
            assert!(runtime.last_composer_signature == signature);
            assert_eq!(runtime.fullscreen.as_ref().unwrap().state.scroll, scroll);
            assert!(runtime.take_next_control_command().is_none());
            assert_eq!(runtime.queue_len(), 0);
            key(&mut runtime, KeyCode::F(6));
            key(&mut runtime, KeyCode::End);
            key(&mut runtime, KeyCode::Enter);
            assert!(runtime.background_panel_active());
            key(&mut runtime, KeyCode::Esc);
            assert!(!runtime.background_panel_active());
        },
    );
}

/// 【后台任务测试】【小屏入口】手动开启的小屏及拥挤侧栏仍能看到管理入口，折叠不隐藏入口。
/// 参数: 无；返回: 无。
#[test]
fn background_manager_stays_visible_at_small_sizes_and_when_collapsed() {
    for (cols, rows) in [(88, 8), (100, 12), (140, 16), (160, 30)] {
        TerminalSize::with_size(TerminalSize { cols, rows }, || {
            let mut runtime = runtime(0);
            runtime.sidebar_control.visibility = Some(super::sidebar_control::Visibility::On);
            runtime.enter_fullscreen().unwrap();
            runtime
                .fullscreen
                .as_mut()
                .unwrap()
                .sidebar
                .git
                .summary
                .files = (0..50)
                .map(|i| (" M".into(), format!("file-{i}")))
                .collect();
            runtime.paint_fullscreen().unwrap();
            assert!(
                sidebar_text(&runtime).contains("Manage tasks"),
                "{cols}x{rows}"
            );
            click(
                &mut runtime,
                SidebarAction::Toggle(SidebarSection::Background),
            );
            assert!(sidebar_text(&runtime).contains("Manage tasks"));
            click(&mut runtime, SidebarAction::Background(None));
            assert!(runtime.background_panel_active());
            for line in runtime.background_panel.previous.as_ref().unwrap() {
                assert!(crate::render::ansi_clip::ansi_visible_width(line) <= usize::from(cols));
            }
            key(&mut runtime, KeyCode::Esc);
        });
    }
}

/// 【后台任务测试】【存储样本】创建非当前会话命令和隐藏网关，不启动或停止真实进程。
/// 参数: paths 为临时应用路径；返回: 任务存储与可见任务。
fn seed(
    paths: &crate::paths::SaiPaths,
) -> (
    crate::tools::command::BackgroundCommandStore,
    crate::tools::command::BackgroundCommandTask,
) {
    use crate::tools::command::{BackgroundCommandStore, BackgroundCommandTask};
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    store.init().unwrap();
    let task = BackgroundCommandTask {
        id: "sidebar-background".into(),
        runtime_process_id: None,
        runtime_owner_kind: Some("session".into()),
        runtime_owner_id: Some("another-session".into()),
        runtime_process_kind: None,
        goal_id: None,
        label: "PERSISTED-COMMAND".into(),
        command: "echo fixture".into(),
        cwd: "/tmp".into(),
        pid: std::process::id(),
        pgid: None,
        status: "running".into(),
        stdout_log: store.logs_dir().join("out.log").display().to_string(),
        stderr_log: store.logs_dir().join("err.log").display().to_string(),
        started_at: 0,
        updated_at: 1,
        timeout_seconds: 0,
        completion_notified: false,
    };
    std::fs::write(&task.stdout_log, "LOG-FIXTURE\n").unwrap();
    std::fs::write(&task.stderr_log, "").unwrap();
    let mut gateway = task.clone();
    gateway.id = "hidden-gateway".into();
    gateway.runtime_owner_kind = Some("gateway".into());
    store.save(&[task.clone(), gateway]).unwrap();
    (store, task)
}

/// 【后台任务测试】【异步刷新】通过空闲事件循环等待持久化状态进入侧栏。
/// 参数: runtime 为运行期，status 为预期状态；返回: 无，超时则断言失败。
async fn wait_status(runtime: &mut ReplRuntime, status: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            TerminalSize::with_size(
                TerminalSize {
                    cols: 160,
                    rows: 30,
                },
                || runtime.process_idle_tick().unwrap(),
            );
            if runtime
                .background_panel
                .tasks()
                .first()
                .is_some_and(|task| task.status == status)
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

/// 【后台任务测试】【持久化链路】没有启动卡片也会加载跨会话任务，自动更新终态并打开真实日志。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn background_sidebar_refreshes_persisted_tasks_and_opens_selected_logs() {
    let temp = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(temp.path());
    let (store, mut task) = seed(&paths);
    let mut runtime = runtime(0);
    runtime.configure_background_tasks(&paths, &crate::config::AppConfig::default());
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || runtime.enter_fullscreen().unwrap(),
    );
    wait_status(&mut runtime, "running").await;
    assert_eq!(runtime.background_panel.tasks().len(), 1);
    let text = sidebar_text(&runtime);
    assert!(
        text.contains("PERSISTED-COMMAND") && text.contains("running"),
        "{text}"
    );
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            assert!(runtime.pending_wait().is_some());
        },
    );
    task.status = "exited".into();
    task.completion_notified = true;
    store.save(&[task]).unwrap();
    wait_status(&mut runtime, "exited").await;
    assert!(sidebar_text(&runtime).contains("exited"));
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            click(&mut runtime, SidebarAction::Background(Some(0)));
            assert!(runtime.background_panel_active());
            key(&mut runtime, KeyCode::Enter);
        },
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            TerminalSize::with_size(
                TerminalSize {
                    cols: 160,
                    rows: 30,
                },
                || runtime.process_idle_tick().unwrap(),
            );
            if runtime
                .background_panel
                .previous
                .as_ref()
                .is_some_and(|lines| lines.iter().any(|line| line.contains("LOG-FIXTURE")))
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(store.load().unwrap()[0].completion_notified);
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            key(&mut runtime, KeyCode::Esc);
            assert!(runtime.background_panel_active());
            key(&mut runtime, KeyCode::Esc);
            assert!(!runtime.background_panel_active());
        },
    );
}

/// 【后台任务测试】【流式隔离】模型输出继续更新历史，浮层及缩放不覆盖草稿，关闭后恢复正文。
/// 参数: 无；返回: 无。
#[test]
fn background_manager_survives_stream_refresh_and_terminal_resize() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(1);
            runtime
                .update_composer(&chrome(), "stream draft", 5, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            runtime.stream_active = true;
            runtime.stream_draft.text = "stream draft".into();
            click(&mut runtime, SidebarAction::Background(None));
            runtime.transcript.push_chunk(&crate::llm::ChatStreamChunk {
                kind: crate::llm::ChatStreamKind::Content,
                text: "STREAM-WHILE-MANAGING".into(),
            });
            super::super::event_loop::process_stream_tick(&mut runtime).unwrap();
            assert!(runtime.background_panel_active());
            assert_eq!(runtime.stream_draft.text, "stream draft");
            TerminalSize::with_size(TerminalSize { cols: 60, rows: 12 }, || {
                runtime
                    .handle_fullscreen_event(&Event::Resize(60, 12))
                    .unwrap();
                let lines = runtime.background_panel.previous.as_ref().unwrap();
                assert_eq!(lines.len(), 12);
                assert!(
                    lines
                        .iter()
                        .all(|line| crate::render::ansi_clip::ansi_visible_width(line) <= 60)
                );
                assert!(
                    !lines
                        .iter()
                        .any(|line| line.contains("STREAM-WHILE-MANAGING"))
                );
                key(&mut runtime, KeyCode::Esc);
            });
            runtime.paint_fullscreen().unwrap();
            let session = runtime.fullscreen.as_ref().unwrap();
            assert!(
                session
                    .state
                    .previous
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(|line| line.contains("STREAM-WHILE-MANAGING"))
            );
            assert_eq!(runtime.stream_draft.text, "stream draft");
        },
    );
}

/// 【后台任务测试】【权限交接】管理界面不遮挡后来到达的审批或提问，草稿仍然保留。
/// 参数: 无；返回: 无。
#[test]
fn background_manager_yields_to_permission_prompt() {
    TerminalSize::with_size(
        TerminalSize {
            cols: 160,
            rows: 30,
        },
        || {
            let mut runtime = runtime(0);
            runtime
                .update_composer(&chrome(), "kept draft", 4, false, Vec::new(), 0)
                .unwrap();
            runtime.enter_fullscreen().unwrap();
            let signature = runtime.last_composer_signature.clone();
            click(&mut runtime, SidebarAction::Background(None));
            assert!(runtime.background_panel_active());
            runtime.pause_for_permission_prompt().unwrap();
            assert!(!runtime.background_panel_active());
            assert!(runtime.last_composer_signature == signature);
        },
    );
}

//! 全屏工作状态必须留在固定区，不能随正文滚动或改变历史布局。

use super::runtime_tests::{chrome, mouse, runtime};
use crate::cli::repl_runtime::ReplRuntime;
use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::render::activity_animation::{strip_ansi_for_test, ACTIVITY_FRAME_INTERVAL};
use crate::render::work_status::WorkStatus;
use crossterm::event::MouseEventKind;
use std::time::Duration;

/// 【全屏测试】【正文读取】返回当前可滚动文档的纯文本。
/// @param runtime 为已进入全屏的运行期
/// @returns 不含终端样式的完整正文
fn body(runtime: &ReplRuntime) -> String {
    runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .document
        .lines
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 【全屏测试】【面板读取】返回固定在输入框上方的面板纯文本。
/// @param runtime 为已创建输入框的运行期
/// @returns 固定面板文本
fn panel(runtime: &ReplRuntime) -> String {
    strip_ansi_for_test(&runtime.composer.as_ref().unwrap().panel_lines().join("\n"))
}

/// 【全屏测试】【状态固定】滚动查看旧消息时，工作与重连状态仍在固定面板。
#[test]
fn work_status_stays_in_fixed_panel_while_scrolled() {
    let mut runtime = runtime(4);
    runtime
        .update_composer(&chrome(), "draft", 5, false, Vec::new(), 0)
        .unwrap();
    runtime.transcript.set_work_status(WorkStatus::Working);
    runtime.enter_fullscreen().unwrap();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollUp, 10, 5))
        .unwrap();
    let before = runtime.fullscreen.as_ref().unwrap().state.scroll;
    assert!(!body(&runtime).contains("Working"));
    assert!(panel(&runtime).contains("Working"));

    runtime
        .transcript
        .set_work_status(WorkStatus::Reconnecting {
            attempt: 2,
            max_attempts: 3,
        });
    runtime.paint_fullscreen().unwrap();
    let session = runtime.fullscreen.as_ref().unwrap();
    let layout = session.state.layout.unwrap();
    assert_eq!(session.state.scroll, before);
    assert_eq!(layout.composer_top + layout.composer_height, 24);
    assert!(!body(&runtime).contains("Reconnecting"));
    assert!(panel(&runtime).contains("Reconnecting... 2/3"));
}

/// 【全屏测试】【动效分离】没有新内容时，工作状态动效只刷新固定区，思考正文留在执行链内。
/// 参数：无；返回：无。
#[test]
fn live_reasoning_animates_only_in_fixed_panel() {
    let mut runtime = runtime(2);
    runtime
        .update_composer(&chrome(), "", 0, false, Vec::new(), 0)
        .unwrap();
    runtime.transcript.set_work_status(WorkStatus::Thinking);
    runtime.transcript.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: "inspect the current window\ncheck its controls".into(),
    });
    runtime
        .transcript
        .set_live_animation_elapsed_for_test(Duration::ZERO);
    runtime.enter_fullscreen().unwrap();
    let first_body = runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .document
        .lines
        .clone();
    let first_panel = runtime.composer.as_ref().unwrap().panel_lines().to_vec();
    // 1. 【全屏测试】【动效分离】第 8 帧仍在流光缓冲区，第 24 帧流光经过文字且笑脸与秒数不变
    runtime
        .transcript
        .set_live_animation_elapsed_for_test(ACTIVITY_FRAME_INTERVAL * 24);
    runtime.paint_fullscreen().unwrap();
    assert_eq!(
        runtime.fullscreen.as_ref().unwrap().state.document.lines,
        first_body
    );
    assert_ne!(
        runtime.composer.as_ref().unwrap().panel_lines(),
        first_panel
    );
    // 2. 【全屏测试】【动效分离】可见内容不变，确保面板差异来自文字样式而非表情或计时
    assert_eq!(
        panel(&runtime),
        strip_ansi_for_test(&first_panel.join("\n"))
    );
    assert!(panel(&runtime).contains("Thinking"));
    assert!(body(&runtime).contains("inspect the current window"));
}

/// 【全屏测试】【状态清理】完成后移除固定状态，返回普通视图仍使用原来的状态位置。
#[test]
fn completed_status_does_not_leak_into_history_or_inline_panel() {
    let mut runtime = runtime(1);
    runtime
        .update_composer(&chrome(), "", 0, false, Vec::new(), 0)
        .unwrap();
    runtime.transcript.set_work_status(WorkStatus::Working);
    runtime.enter_fullscreen().unwrap();
    assert!(panel(&runtime).contains("Working"));
    runtime.transcript.clear_work_status();
    runtime.paint_fullscreen().unwrap();
    assert!(!panel(&runtime).contains("Working"));
    assert!(!body(&runtime).contains("Working"));
    runtime.transcript.set_work_status(WorkStatus::Working);
    runtime.leave_fullscreen().unwrap();
    assert!(!runtime
        .bottom_panel_lines(80)
        .join("\n")
        .contains("Working"));
}

//! 队列调序、稳定选择、删除恢复与限高视图的回归。

use super::queue_panel::{is_enter_queue_key, QueuePanelState};
use super::queue_panel_view::{page_size, render_at};
use super::viewport::TerminalSize;
use super::{QueueInsertAt, QueuedSubmission, ReplRuntime};
use crate::agent::AgentMode;
use crate::cli::repl_clipboard::ReplClipboardState;
use crate::render::ansi_clip::strip_ansi;
use crate::render::transcript::TranscriptRenderOptions;
use crossterm::event::{KeyCode, KeyModifiers};

/// 【队列测试】【运行期】创建纯内存队列，避免调用模型或依赖当前仓库。
/// 参数: count 为消息数量；返回: 带稳定标识的运行期。
fn runtime(count: usize) -> ReplRuntime {
    let runtime = ReplRuntime::new(
        1000,
        TranscriptRenderOptions {
            reasoning_mode: crate::render::ReasoningDisplayMode::Summary,
            tool_call_mode: crate::render::ToolCallDisplayMode::Summary,
        },
    );
    for index in 0..count {
        runtime.lock_queue().push_back(QueuedSubmission::new(
            AgentMode::Yolo,
            format!("message-{index:02}"),
            ReplClipboardState::default(),
        ));
    }
    runtime
}

/// 【队列测试】【管理按键】参数: runtime 为运行期，code/modifiers 为按键；返回: 无。
fn key(runtime: &mut ReplRuntime, code: KeyCode, modifiers: KeyModifiers) {
    assert!(runtime.handle_queue_panel_key(code, modifiers).unwrap());
}

/// 【队列测试】【排序一致】调序和置顶只移动整条消息，时机与稳定标识不变。
/// 参数: 无；返回: 无。
#[test]
fn reorder_and_top_preserve_identity_and_timing() {
    let mut runtime = runtime(4);
    runtime.lock_queue()[2].insert_at = QueueInsertAt::Request;
    let id = runtime.lock_queue()[2].id.clone();
    key(&mut runtime, KeyCode::Up, KeyModifiers::CONTROL);
    key(&mut runtime, KeyCode::Up, KeyModifiers::NONE);
    key(&mut runtime, KeyCode::Up, KeyModifiers::SHIFT);
    assert_eq!(runtime.lock_queue()[1].id, id);
    assert_eq!(runtime.queue_panel.selected(), 1);
    key(&mut runtime, KeyCode::Char('t'), KeyModifiers::NONE);
    assert_eq!(runtime.lock_queue()[0].id, id);
    assert_eq!(runtime.lock_queue()[0].insert_at, QueueInsertAt::Request);
    key(&mut runtime, KeyCode::Down, KeyModifiers::SHIFT);
    assert_eq!(runtime.lock_queue()[1].id, id);
}

/// 【队列测试】【恢复删除】最后一项删除后仍能恢复，取回编辑不进入恢复历史且保留原时机。
/// 参数: 无；返回: 无。
#[test]
fn undo_delete_restores_last_item_and_edit_is_not_undoable() {
    let mut runtime = runtime(1);
    runtime.lock_queue()[0].insert_at = QueueInsertAt::Request;
    let id = runtime.lock_queue()[0].id.clone();
    key(&mut runtime, KeyCode::Up, KeyModifiers::CONTROL);
    key(&mut runtime, KeyCode::Char('d'), KeyModifiers::NONE);
    assert_eq!(runtime.queue_len(), 1);
    key(&mut runtime, KeyCode::Esc, KeyModifiers::NONE);
    assert!(runtime.queue_panel.is_active());
    assert!(!runtime.queue_panel.pending_delete);
    key(&mut runtime, KeyCode::Char('d'), KeyModifiers::NONE);
    key(&mut runtime, KeyCode::Char('d'), KeyModifiers::NONE);
    assert_eq!(runtime.queue_len(), 0);
    assert!(runtime.queue_panel.is_active());
    key(&mut runtime, KeyCode::Char('u'), KeyModifiers::NONE);
    assert_eq!(runtime.lock_queue()[0].id, id);
    key(&mut runtime, KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(runtime.stream_draft().text, "message-00");
    assert_eq!(runtime.queue_draft_timing, QueueInsertAt::Request);
    assert!(runtime.queue_panel.deleted.is_empty());
    runtime.enqueue_stream_draft(AgentMode::Yolo).unwrap();
    assert_eq!(runtime.lock_queue()[0].insert_at, QueueInsertAt::Request);
    assert_eq!(runtime.queue_draft_timing, QueueInsertAt::Turn);
}

/// 【队列测试】【附件恢复】删除恢复保留完整长文本附件，恢复栈上限二十条。
/// 参数: 无；返回: 无。
#[test]
fn deleted_history_is_bounded_and_preserves_attachments() {
    let mut runtime = runtime(25);
    let body = "attachment payload ".repeat(400);
    let mut text = String::new();
    let mut clipboard = ReplClipboardState::default();
    clipboard.paste_text_into_input(&mut text, &mut 0, body.clone());
    runtime.lock_queue()[24].text = text;
    runtime.lock_queue()[24].clipboard = clipboard;
    key(&mut runtime, KeyCode::Up, KeyModifiers::CONTROL);
    key(&mut runtime, KeyCode::Char('d'), KeyModifiers::NONE);
    key(&mut runtime, KeyCode::Char('d'), KeyModifiers::NONE);
    key(&mut runtime, KeyCode::Char('u'), KeyModifiers::NONE);
    let item = runtime.lock_queue()[24].clone();
    assert_eq!(
        item.clipboard.to_chat_input(&item.text).message,
        body.trim()
    );
    for _ in 0..25 {
        key(&mut runtime, KeyCode::Char('d'), KeyModifiers::NONE);
        key(&mut runtime, KeyCode::Char('d'), KeyModifiers::NONE);
    }
    assert_eq!(runtime.queue_panel.deleted.len(), 20);
}

/// 【队列测试】【后台消费】被确认删除的消息消失时取消确认，不能误删补到原位置的另一条消息。
/// 参数: 无；返回: 无。
#[test]
fn asynchronous_consumption_preserves_selection_and_cancels_stale_delete() {
    let mut runtime = runtime(4);
    key(&mut runtime, KeyCode::Up, KeyModifiers::CONTROL);
    let id = runtime.queue_panel.selected_id.clone();
    runtime.lock_queue().pop_front();
    runtime.clamp_queue_panel();
    assert_eq!(runtime.queue_panel.selected(), 2);
    assert_eq!(runtime.queue_panel.selected_id, id);
    key(&mut runtime, KeyCode::Char('d'), KeyModifiers::NONE);
    runtime.lock_queue().pop_back();
    runtime.clamp_queue_panel();
    assert!(!runtime.queue_panel.pending_delete);
    key(&mut runtime, KeyCode::Char('d'), KeyModifiers::NONE);
    assert_eq!(runtime.queue_len(), 2);
}

/// 【队列测试】【限高列表】上百条队列不增高面板，窄屏仍显示调序、恢复和编辑控件。
/// 参数: 无；返回: 无。
#[test]
fn manager_rows_stay_bounded_and_click_regions_match_visible_buttons() {
    TerminalSize::with_size(TerminalSize { cols: 80, rows: 32 }, || {
        let runtime = runtime(100);
        let items = runtime.queued_items();
        let mut panel = QueuePanelState::default();
        panel.activate(items.len());
        for width in [40, 80, 160] {
            let rows = render_at(&panel, &items, true, width);
            assert!(rows.len() <= page_size() + 3);
            assert!(rows.iter().any(|row| row.text.contains("message-99")));
            assert!(rows
                .iter()
                .all(|row| crate::cli::repl_text::visible_width(&row.text) <= width));
            let plain = rows
                .iter()
                .map(|row| strip_ansi(&row.text))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(plain.contains("[ u") && plain.contains("[ Enter"));
            for row in &rows {
                assert!(!row.text.contains("\x1b[48;"));
                assert!(row.hits.iter().all(|(columns, _)| columns.end <= width));
            }
        }
    });
    assert!(is_enter_queue_key(KeyCode::Char('m'), KeyModifiers::ALT));
}

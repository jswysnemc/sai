use super::*;
use crate::cli::repl_clipboard::ReplClipboardState;
use crate::cli::repl_runtime::QueueInsertAt;
use crate::render::transcript::TranscriptRenderOptions;
use crate::state::input_history::{
    InputHistoryAttachment, InputHistoryAttachmentKind, InputHistoryEntry,
};

/// 【终端队列】【测试环境】创建独立终端运行期，不修改真实输入历史。
///
/// 参数: 无
/// 返回: 测试运行期
fn runtime() -> ReplRuntime {
    ReplRuntime::new(
        100,
        TranscriptRenderOptions {
            reasoning_mode: render::ReasoningDisplayMode::Summary,
            tool_call_mode: render::ToolCallDisplayMode::Summary,
        },
    )
}

/// 【终端队列】【测试消息】构造带默认附件状态的排队消息。
///
/// 参数: `text` 为提交正文
/// 返回: 默认轮次间隔提交
fn queued(text: &str) -> QueuedSubmission {
    QueuedSubmission::new(
        AgentMode::Yolo,
        text.to_string(),
        ReplClipboardState::default(),
    )
}

/// 【终端队列】【失败回归】取出正在执行的消息后，后续两条仍在运行期并保持顺序。
///
/// 参数: 无
/// 返回: 无；消息数量、顺序或标识变化时断言失败
#[test]
fn reported_regression_failure_keeps_unsent_queue() {
    let mut runtime = runtime();
    let items = [
        queued("active"),
        queued("first pending"),
        queued("second pending"),
    ];
    let ids = items.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
    for (index, item) in items.into_iter().enumerate() {
        runtime.restore_queued_submission(index, item);
    }
    let (_, active) = runtime.take_next_turn_submission().unwrap();
    assert_eq!(active.id, ids[0]);
    notify_preserved_queue(&mut runtime).unwrap();
    assert_eq!(runtime.queue_len(), 2);
    assert!(!runtime.has_ready_turn_submission());
    assert!(runtime.take_next_turn_submission().is_none());
    runtime.resume_submission_queue();
    let pending = runtime.take_turn_interval_queue();
    assert_eq!(
        pending
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>(),
        ids[1..]
    );
    assert_eq!(pending[0].text, "first pending");
    assert_eq!(pending[1].text, "second pending");
}

/// 【终端队列】【附件恢复】启动失败恢复原位置、插入点、模式、长文本和图片附件。
///
/// 参数: 无
/// 返回: 无；任何提交数据丢失或请求间隔消息改变时断言失败
#[test]
fn restored_submission_keeps_position_and_all_attachments() {
    let mut runtime = runtime();
    let entry = InputHistoryEntry {
        text: "[Pasted 1] [Image 1]".to_string(),
        attachments: vec![
            InputHistoryAttachment {
                marker: "[Pasted 1]".to_string(),
                content: "long text ".repeat(1_000),
                kind: InputHistoryAttachmentKind::Text,
            },
            InputHistoryAttachment {
                marker: "[Image 1]".to_string(),
                content: "data:image/png;base64,AAAA".to_string(),
                kind: InputHistoryAttachmentKind::Image,
            },
        ],
    };
    let mut request = queued("next request");
    request.insert_at = QueueInsertAt::Request;
    runtime.restore_queued_submission(0, request.clone());
    let item = QueuedSubmission::new(
        AgentMode::Audited,
        entry.text.clone(),
        ReplClipboardState::from_history_entry(&entry),
    );
    let id = item.id.clone();
    runtime.restore_queued_submission(1, item);
    runtime.restore_queued_submission(2, queued("later"));
    let (position, item) = runtime.take_next_turn_submission().unwrap();
    assert_eq!(position, 1);
    runtime.restore_queued_submission(position, item);
    let items = runtime.take_submission_queue();
    assert_eq!(items[0].id, request.id);
    assert_eq!(items[0].insert_at, QueueInsertAt::Request);
    assert_eq!(items[1].id, id);
    assert_eq!(items[1].mode, AgentMode::Audited);
    assert_eq!(items[1].clipboard.history_entry(&items[1].text), entry);
    let chat = items[1].clipboard.to_chat_input(&items[1].text);
    assert!(chat.message.contains(entry.attachments[0].content.trim()));
    assert_eq!(
        chat.image_urls,
        ["data:image/png;base64,AAAA"]
    );
    assert_eq!(items[2].text, "later");
}

/// 【终端队列】【命令恢复】已有草稿时，控制命令和后续消息全部留在队列中。
///
/// 参数: 无
/// 返回: 无；草稿被覆盖或命令丢失时断言失败
#[test]
fn queued_control_command_does_not_overwrite_existing_draft() {
    let mut runtime = runtime();
    runtime.stream_draft_mut().text = "unfinished draft".to_string();
    runtime.restore_queued_submission(0, queued("/model"));
    runtime.restore_queued_submission(1, queued("later"));
    let (position, item) = runtime.take_next_turn_submission().unwrap();
    restore_control_submission(&mut runtime, position, item).unwrap();
    assert_eq!(runtime.stream_draft().text, "unfinished draft");
    let items = runtime.take_submission_queue();
    assert_eq!(items[0].text, "/model");
    assert_eq!(items[1].text, "later");
}

/// 【终端队列】【命令恢复】空输入框接收控制命令，其他消息保持可管理状态。
///
/// 参数: 无
/// 返回: 无；输入或剩余队列不正确时断言失败
#[test]
fn queued_control_command_returns_to_empty_draft() {
    let mut runtime = runtime();
    runtime.restore_queued_submission(0, queued("/model"));
    runtime.restore_queued_submission(1, queued("later"));
    let (position, item) = runtime.take_next_turn_submission().unwrap();
    restore_control_submission(&mut runtime, position, item).unwrap();
    assert_eq!(runtime.stream_draft().text, "/model");
    assert_eq!(runtime.stream_draft().cursor, 6);
    assert_eq!(runtime.take_submission_queue()[0].text, "later");
}

/// 【终端队列】【暂停投递】失败暂停同时覆盖两种插入点，自动续轮不能绕过用户确认。
///
/// 参数: 无
/// 返回: 无；自动投递消耗条目或主动提交无法恢复时断言失败
#[tokio::test]
async fn paused_queue_waits_for_a_user_submission() {
    let mut runtime = runtime();
    let mut request = queued("next request");
    request.insert_at = QueueInsertAt::Request;
    runtime.restore_queued_submission(0, request);
    assert!(!runtime.has_ready_turn_submission());
    runtime.restore_queued_submission(1, queued("next turn"));
    let source = runtime.inter_message_source();
    runtime.pause_submission_queue();
    for automatic in [true, false] {
        let submission = repl_runner_submission(
            clipboard::ClipboardChatInput {
                message: "continue".to_string(),
                image_urls: Vec::new(),
            },
            AgentMode::Yolo,
            render::ReasoningDisplayMode::Summary,
            render::ToolCallDisplayMode::Summary,
            render::StreamRenderOptions::default(),
            automatic,
        );
        runtime.resume_queue_for_submission(&submission);
        if automatic {
            assert!(!runtime.has_ready_turn_submission());
            assert!(runtime.take_next_turn_submission().is_none());
            assert!(source.peek().await.unwrap().is_none());
            assert_eq!(runtime.queue_len(), 2);
        } else {
            assert!(runtime.has_ready_turn_submission());
            assert_eq!(source.peek().await.unwrap().unwrap().prompt, "next request");
            assert_eq!(
                runtime.take_next_turn_submission().unwrap().1.text,
                "next turn"
            );
        }
    }
}

/// 【终端队列】【显式发送】暂停后仍可通过管理面板发送选中条目并恢复后续消费。
///
/// 参数: 无
/// 返回: 无；显式发送没有解除暂停时断言失败
#[test]
fn queue_panel_send_now_resumes_paused_submissions() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut runtime = runtime();
    runtime.restore_queued_submission(0, queued("first"));
    runtime.restore_queued_submission(1, queued("second"));
    runtime.pause_submission_queue();
    runtime
        .handle_queue_panel_idle_key(KeyCode::Up, KeyModifiers::CONTROL, true)
        .unwrap();
    let action = runtime
        .handle_queue_panel_idle_key(KeyCode::Char('s'), KeyModifiers::NONE, true)
        .unwrap();
    assert!(matches!(
        action,
        crate::cli::repl_runtime::QueuePanelIdleResult::SendNow(_)
    ));
    assert!(runtime.has_ready_turn_submission());
}

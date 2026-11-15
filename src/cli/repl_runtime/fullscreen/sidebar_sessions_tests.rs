//! 会话浏览回归：视图切换不替换执行数据、不消费输入、不写工作区指针。

use super::sidebar_sessions::{SessionEntry, SessionStatus};
use super::sidebar_sessions_view::key;
use super::*;
use crate::render::transcript::TranscriptRenderOptions;

/// 【会话测试】【运行夹具】无参数；返回摘要模式的独立终端运行期
fn runtime() -> ReplRuntime {
    ReplRuntime::new(
        5000,
        TranscriptRenderOptions {
            reasoning_mode: crate::render::ReasoningDisplayMode::Summary,
            tool_call_mode: crate::render::ToolCallDisplayMode::Summary,
        },
    )
}

/// 【会话测试】【条目夹具】参数 id 为标识；返回稳定目录条目
fn entry(id: &str) -> SessionEntry {
    SessionEntry {
        id: id.into(),
        title: format!("Session {id}"),
        state_dir: std::path::PathBuf::from(format!("/unused/{id}")),
        updated_at: String::new(),
        status: SessionStatus::Idle,
    }
}

/// 【会话测试】【直接请求】无参数；返回无，点击立即请求宿主切换但不修改执行句柄
#[test]
fn click_requests_host_switch_without_interrupting_execution() {
    let mut runtime = runtime();
    runtime.session_browser.current = Some(entry("current"));
    runtime.session_browser.entries = vec![entry("target")];
    runtime.stream_active = true;
    runtime.browse_session(key(&entry("target")));
    assert!(runtime.has_session_switch_request());
    assert!(runtime.stream_active);
    assert_eq!(
        runtime.take_session_switch_request().as_deref(),
        Some("target")
    );
    assert!(runtime.take_next_control_command().is_none());
}

/// 【会话测试】【稳定动作】无参数；返回无，排序与标题变化不改变会话点击归属
#[test]
fn action_key_uses_directory_not_order_or_title() {
    let first = entry("first");
    let mut renamed = first.clone();
    renamed.title = "renamed".into();
    assert_eq!(key(&first), key(&renamed));
    assert_ne!(key(&first), key(&entry("second")));
}

/// 【会话测试】【自适应列表】无参数；返回无，含控制字符的外部标题不会破坏终端且各行不超宽
#[test]
fn session_rows_are_safe_and_bounded() {
    let mut sessions = super::sidebar_sessions::SidebarSessions::default();
    let mut current = entry("current");
    current.title = "\x1b[2J标题\n下一行".into();
    sessions.current = Some(current);
    sessions.entries = vec![entry("other")];
    for width in [1, 10, 25, 40] {
        for row in super::sidebar_sessions_view::rows(&sessions, false, width) {
            assert!(!row.text.contains("\x1b[2J"));
            assert!(!row.text.contains('\n'));
            assert!(crate::cli::repl_text::visible_width(&row.text) <= width);
        }
    }
}

/// 【会话测试】【完整状态隔离】无参数；返回无，切换分别保存输出、草稿、队列和主会话阅读位置
#[test]
fn session_workspace_restores_draft_queue_transcript_and_reading_position() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(root.path());
    let first = crate::state::StateStore::new(&paths).unwrap();
    let second_info = crate::state::create_session_detached(&paths, Some("second")).unwrap();
    let second = crate::state::StateStore::for_session(&paths, &second_info.id).unwrap();
    let mut runtime = runtime();
    runtime.bind_session_browser(&paths, &first);
    runtime.fullscreen = Some(FullscreenSession::enter().unwrap());
    runtime.fullscreen.as_mut().unwrap().state.scroll = 8;
    runtime.fullscreen.as_mut().unwrap().state.follow = false;
    runtime.transcript.push_meta("FIRST-ONLY".into());
    runtime.stream_draft.text = "first draft".into();
    runtime.stream_active = true;
    let first_queue = runtime.submission_queue.clone();
    first_queue
        .lock()
        .unwrap()
        .push_back(super::super::QueuedSubmission::new(
            crate::agent::AgentMode::Yolo,
            "first queued".into(),
            Default::default(),
        ));
    assert!(!runtime.switch_session_workspace(&paths, &second).unwrap());
    assert!(!runtime.stream_active);
    assert_eq!(runtime.queue_len(), 0);
    runtime.stream_draft.text = "second draft".into();
    runtime.transcript.push_meta("SECOND-ONLY".into());
    assert!(runtime.switch_session_workspace(&paths, &first).unwrap());
    assert!(runtime.stream_active);
    assert_eq!(runtime.stream_draft.text, "first draft");
    assert!(std::sync::Arc::ptr_eq(
        &first_queue,
        &runtime.submission_queue
    ));
    assert_eq!(runtime.queue_len(), 1);
    assert_eq!(runtime.fullscreen.as_ref().unwrap().state.scroll, 8);
    assert!(!runtime.fullscreen.as_ref().unwrap().state.follow);
    let doc = runtime
        .transcript
        .render_fullscreen(80, &runtime.options, &Default::default());
    let text = doc
        .lines
        .iter()
        .map(|line| line.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("FIRST-ONLY"));
    assert!(!text.contains("SECOND-ONLY"));
    assert!(runtime.switch_session_workspace(&paths, &second).unwrap());
    assert_eq!(runtime.stream_draft.text, "second draft");
}

/// 【会话测试】【后台输出隔离】无参数；返回无，后台事件持续投影但不改变前台内容或阅读位置
#[test]
fn background_events_update_only_their_session_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(root.path());
    let first = crate::state::StateStore::new(&paths).unwrap();
    let second_info = crate::state::create_session_detached(&paths, Some("second")).unwrap();
    let second = crate::state::StateStore::for_session(&paths, &second_info.id).unwrap();
    let mut runtime = runtime();
    runtime.bind_session_browser(&paths, &first);
    runtime.fullscreen = Some(FullscreenSession::enter().unwrap());
    let sender = runtime.session_event_sender();
    runtime.switch_session_workspace(&paths, &second).unwrap();
    runtime.transcript.push_meta("VISIBLE-SECOND".into());
    runtime.fullscreen.as_mut().unwrap().state.scroll = 4;
    runtime.fullscreen.as_mut().unwrap().state.follow = false;
    sender
        .send((
            first.session_id().into(),
            crate::runner::RunnerEvent::Agent(crate::agent::AgentEvent::Chunk(
                crate::llm::ChatStreamChunk {
                    kind: crate::llm::ChatStreamKind::Content,
                    text: "HIDDEN-FIRST-ANSWER".into(),
                },
            )),
        ))
        .unwrap();
    runtime.tick_session_events().unwrap();
    assert_eq!(runtime.fullscreen.as_ref().unwrap().state.scroll, 4);
    assert!(!runtime.fullscreen.as_ref().unwrap().state.follow);
    let visible = runtime
        .transcript
        .render_fullscreen(80, &runtime.options, &Default::default());
    assert!(!visible
        .lines
        .iter()
        .any(|line| line.as_str().contains("HIDDEN-FIRST-ANSWER")));
    runtime.switch_session_workspace(&paths, &first).unwrap();
    let first_doc = runtime
        .transcript
        .render_fullscreen(80, &runtime.options, &Default::default());
    assert!(first_doc
        .lines
        .iter()
        .any(|line| line.as_str().contains("HIDDEN-FIRST-ANSWER")));
}

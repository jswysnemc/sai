use super::*;
use crate::cli::repl_input::ReplInputSubmission;
use crate::paths::SaiPaths;
use crate::tools::subagent_state;

/// 【终端】【接管测试】隔离子代理状态与输入运行期
struct Fixture {
    _root: tempfile::TempDir,
    _cancel: tokio::sync::oneshot::Receiver<()>,
    owner: String,
    id: String,
    runtime: ReplRuntime,
    ctx: StreamCommandContext,
}

impl Fixture {
    /// 【终端】【接管测试】创建当前查看子代理的输入环境；无参数，返回隔离环境
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let owner = root.path().to_string_lossy().into_owned();
        let (snapshot, cancel) = subagent_state::create_subagent_for_owner(
            &owner,
            "takeover".into(),
            "general".into(),
            4,
        );
        let mut runtime = ReplRuntime::new(
            100,
            crate::render::transcript::TranscriptRenderOptions {
                reasoning_mode: crate::render::ReasoningDisplayMode::Summary,
                tool_call_mode: crate::render::ToolCallDisplayMode::Summary,
            },
        );
        runtime
            .transcript
            .push_tool_call("subagent".into(), r#"{"description":"takeover"}"#.into());
        runtime.transcript.push_tool_result(
            "subagent".into(),
            true,
            serde_json::json!({"subagent": {"id": snapshot.id, "status": "running"}}).to_string(),
        );
        assert!(runtime.transcript.enter_subagent_view(0));
        let ctx = StreamCommandContext::capture(
            &SaiPaths::for_tests(root.path()),
            owner.clone(),
            AgentMode::Yolo,
        );
        Self {
            _root: root,
            _cancel: cancel,
            owner,
            id: snapshot.id,
            runtime,
            ctx,
        }
    }

    /// 【终端】【接管测试】投递草稿；无参数，返回子代理路由结果
    fn submit(&mut self) -> Option<Result<()>> {
        let draft = self.runtime.stream_draft();
        let submission =
            ReplInputSubmission::from_input(AgentMode::Yolo, draft.text.clone(), &draft.clipboard);
        crate::cli::repl::subagent_input::deliver_viewed_submission(
            &mut self.runtime,
            &self.owner,
            &submission,
        )
    }
}

impl Drop for Fixture {
    /// 【终端】【接管测试】清理此测试的子代理记录；无参数，无返回值
    fn drop(&mut self) {
        subagent_state::clear_subagents_for_owner(&self.owner);
    }
}

/// 【终端】【接管输入】主会话运行时消息只进入当前子代理；无参数，无返回值
#[test]
fn takeover_regression_stream_input_targets_viewed_subagent() {
    for key in [KeyCode::Enter, KeyCode::Tab] {
        let mut fixture = Fixture::new();
        fixture.runtime.stream_draft_mut().text = "follow up to child".into();
        handle_stream_key(&mut fixture.runtime, &fixture.ctx, key, KeyModifiers::NONE).unwrap();
        let inbox = subagent_state::drain_subagent_inbox(&fixture.id);
        assert!(
            fixture.runtime.queued_items().is_empty(),
            "child message leaked into main queue"
        );
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].from, "user");
        assert_eq!(inbox[0].text, "follow up to child");
        assert!(fixture.runtime.stream_draft().text.is_empty());
    }
}

/// 【终端】【接管输入】空闲提交展开粘贴原子块，主视图保持原有路由；无参数，无返回值
#[test]
fn idle_submission_expands_pasted_text_and_main_view_is_not_routed() {
    let mut fixture = Fixture::new();
    let pasted = "a long line of follow-up text\n".repeat(100);
    let draft = fixture.runtime.stream_draft_mut();
    draft
        .clipboard
        .paste_text_into_input(&mut draft.text, &mut draft.cursor, pasted.clone());
    assert!(fixture.submit().unwrap().is_ok());
    let inbox = subagent_state::drain_subagent_inbox(&fixture.id);
    assert_eq!(inbox[0].text, pasted.trim());
    assert!(fixture.runtime.queued_items().is_empty());
    fixture.runtime.transcript.exit_subagent_view();
    assert!(fixture.submit().is_none());
    assert!(subagent_state::drain_subagent_inbox(&fixture.id).is_empty());
}

/// 【终端】【接管失败】终态子代理拒收时保留草稿并显示原因，不回退到主队列；无参数，无返回值
#[test]
fn completed_subagent_keeps_stream_draft_and_shows_error() {
    let mut fixture = Fixture::new();
    subagent_state::finish_subagent(&fixture.id, "completed", Some("done".into()), None, None);
    fixture.runtime.stream_draft_mut().text = "retain this follow-up".into();
    handle_stream_key(
        &mut fixture.runtime,
        &fixture.ctx,
        KeyCode::Enter,
        KeyModifiers::NONE,
    )
    .unwrap();
    assert_eq!(fixture.runtime.stream_draft().text, "retain this follow-up");
    assert!(fixture.runtime.queued_items().is_empty());
    assert!(fixture
        .runtime
        .subagent_input_error
        .as_ref()
        .unwrap()
        .1
        .contains("not accepting messages"));
    assert!(subagent_state::drain_subagent_inbox(&fixture.id).is_empty());
}

/// 【终端】【接管附件】图片提交整体拒收，不静默丢弃附件或回退到主会话；无参数，无返回值
#[test]
fn image_submission_is_rejected_before_text_delivery() {
    let mut fixture = Fixture::new();
    let mut submission = ReplInputSubmission::from_input(
        AgentMode::Yolo,
        "describe image".into(),
        &Default::default(),
    );
    submission.chat_input.image_url = Some("data:image/png;base64,AA==".into());
    let result = crate::cli::repl::subagent_input::deliver_viewed_submission(
        &mut fixture.runtime,
        &fixture.owner,
        &submission,
    );
    assert!(result.unwrap().is_err());
    assert!(fixture.runtime.subagent_input_error.is_some());
    assert!(subagent_state::drain_subagent_inbox(&fixture.id).is_empty());
    assert!(fixture.runtime.queued_items().is_empty());
}

/// 【终端】【接管归属】错误父会话不允许本地写入目标收件箱；无参数，无返回值
#[test]
fn wrong_owner_never_delivers_to_main_or_child() {
    let mut fixture = Fixture::new();
    fixture.runtime.stream_draft_mut().text = "retain this message".into();
    fixture.owner.push_str("-wrong-owner");
    assert!(fixture.submit().unwrap().is_err());
    fixture
        .owner
        .truncate(fixture.owner.len() - "-wrong-owner".len());
    assert!(fixture.runtime.queued_items().is_empty());
    assert!(subagent_state::drain_subagent_inbox(&fixture.id).is_empty());
}

/// 【终端】【接管边界】切回主视图后正常入队，既有子代理消息不改变投递目标；无参数，无返回值
#[test]
fn switching_back_restores_main_queue_routing() {
    let mut fixture = Fixture::new();
    fixture.runtime.transcript.exit_subagent_view();
    fixture.runtime.stream_draft_mut().text = "message to main".into();
    handle_stream_key(
        &mut fixture.runtime,
        &fixture.ctx,
        KeyCode::Enter,
        KeyModifiers::NONE,
    )
    .unwrap();
    assert_eq!(fixture.runtime.queued_items().len(), 1);
    assert!(subagent_state::drain_subagent_inbox(&fixture.id).is_empty());
}

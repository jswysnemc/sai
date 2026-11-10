use super::*;
use crate::agent::AgentMode;
use crate::config::AppConfig;
use crate::llm::OpenAiCompatibleClient;
use crate::paths::SaiPaths;
use crate::state::StateStore;
use crate::tools::ToolRegistry;

/// 【上下文】【预计算夹具】构造旧历史和当前任务，不连接外部服务。
/// 参数：隔离路径；返回执行器及旧轮次压缩范围。
fn fixture(paths: &SaiPaths) -> (Agent, CompactionRequest) {
    let mut config = AppConfig::default();
    config.load_instruction_files = false;
    config.skills.enabled = false;
    config.memory.enabled = false;
    let state = StateStore::new(paths).unwrap();
    state.start_turn("old", "old task").unwrap();
    state
        .complete_turn("old", &"old facts ".repeat(200), None)
        .unwrap();
    let request = CompactionRequest::new(state.load_turns().unwrap(), None);
    state.start_turn("current", "latest user guidance").unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, paths).unwrap();
    let agent = Agent::new(
        config,
        paths,
        state,
        client,
        ToolRegistry::new(),
        AgentMode::Yolo,
    )
    .unwrap();
    (agent, request)
}

/// 【上下文】【来源回归】追加新指导不改变旧前缀，改写旧历史和模型配置必须使摘要失效。
#[test]
fn source_identity_tracks_history_and_model_but_preserves_new_guidance() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let (mut agent, request) = fixture(&paths);
    let (_, original) = agent
        .background_compaction_source(&request)
        .unwrap()
        .unwrap();
    agent
        .state
        .start_turn(
            "new-guidance",
            "Do Windows first without changing objective",
        )
        .unwrap();
    assert_eq!(
        agent
            .background_compaction_source(&request)
            .unwrap()
            .unwrap()
            .1,
        original
    );
    agent.config.providers[0].default_model.push_str("-changed");
    assert_ne!(
        agent
            .background_compaction_source(&request)
            .unwrap()
            .unwrap()
            .1,
        original
    );
    agent.config.providers[0].default_model = agent.client.model().to_string();
    agent
        .state
        .complete_turn("old", "changed evidence", None)
        .unwrap();
    assert_ne!(
        agent
            .background_compaction_source(&request)
            .unwrap()
            .unwrap()
            .1,
        original
    );
    agent
        .state
        .apply_compaction(&request, "other authoritative summary")
        .unwrap();
    assert!(agent
        .background_compaction_source(&request)
        .unwrap()
        .is_none());
}

/// 【上下文】【应用回归】完成的预计算在间隙应用，当前任务和新增指导仍存在于会话投影。
#[tokio::test]
async fn ready_background_summary_preserves_current_task() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let (mut agent, request) = fixture(&paths);
    let (_, source) = agent
        .background_compaction_source(&request)
        .unwrap()
        .unwrap();
    let handle = tokio::spawn(async {
        Ok(ChatResult {
            content: "Old findings preserved; current task remains pending.".into(),
            reasoning: None,
            provider_content: None,
            usage: None,
            tool_calls: Vec::new(),
            duration_ms: 0,
            ttft_ms: 0,
        })
    });
    while !handle.is_finished() {
        tokio::task::yield_now().await;
    }
    agent.background_compaction = Some(BackgroundCompaction {
        request,
        source,
        handle: Some(handle),
    });
    let messages = vec![
        ChatMessage::system("system"),
        ChatMessage::plain("user", "latest user guidance"),
    ];
    let mut applied_event = false;
    let applied = agent
        .maintain_background_compaction("current", &messages, 100, &mut |event| {
            if matches!(event, AgentEvent::CompactionFinished { applied: true, .. }) {
                applied_event = true;
            }
            Ok(())
        })
        .await
        .unwrap();
    assert!(applied && applied_event);
    assert!(agent
        .state
        .load_turns()
        .unwrap()
        .iter()
        .any(|turn| turn.turn_id == "current" && turn.user_content == "latest user guidance"));
    assert!(agent
        .state
        .load_authoritative_compaction_summary()
        .unwrap()
        .unwrap()
        .summary
        .contains("Old findings preserved"));
}

/// 【上下文】【取消回归】丢弃预计算必须取消其模型 future，不留下后台工作。
#[tokio::test]
async fn dropping_pending_summary_aborts_its_task() {
    let handle =
        tokio::spawn(async { futures_util::future::pending::<Result<ChatResult>>().await });
    let abort = handle.abort_handle();
    drop(BackgroundCompaction {
        request: CompactionRequest::new(Vec::new(), None),
        source: String::new(),
        handle: Some(handle),
    });
    tokio::task::yield_now().await;
    assert!(abort.is_finished());
}

/// 【上下文】【等待取消回归】取消正在等待摘要的调用也必须终止请求；无参数、无返回值。
#[tokio::test]
async fn cancelling_summary_wait_aborts_its_task() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let (mut agent, request) = fixture(&paths);
    let (_, source) = agent
        .background_compaction_source(&request)
        .unwrap()
        .unwrap();
    let handle =
        tokio::spawn(async { futures_util::future::pending::<Result<ChatResult>>().await });
    let abort = handle.abort_handle();
    agent.background_compaction = Some(BackgroundCompaction {
        request,
        source,
        handle: Some(handle),
    });
    let messages = vec![ChatMessage::plain("user", "latest user guidance")];
    let mut on_event = |_| Ok(());
    let wait =
        agent.maintain_background_compaction("current", &messages, usize::MAX, &mut on_event);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), wait)
            .await
            .is_err()
    );
    tokio::task::yield_now().await;
    assert!(
        abort.is_finished(),
        "cancelled wait must not detach the summary request"
    );
}

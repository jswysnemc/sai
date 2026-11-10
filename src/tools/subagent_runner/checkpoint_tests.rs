use super::*;
use crate::tools::subagent_runner::checkpoint::{SavedMessage, SubagentCheckpoint};

/// 【子任务】【恢复回归】检查点跨 JSON 保存签名、图片和已完成结果，只补齐未知调用。
#[test]
fn checkpoint_preserves_signatures_and_repairs_only_unconfirmed_calls() {
    let calls = ["done", "unknown"].map(|id| crate::llm::ToolCall {
        id: id.into(),
        kind: "function".into(),
        function: crate::llm::ToolCallFunction {
            name: "run_command".into(),
            arguments: "{}".into(),
        },
    });
    let signature = crate::llm::ProviderAssistantContent::Anthropic(vec![json!({
        "type":"thinking","thinking":"check first","signature":"original-signature"
    })]);
    let assistant = ChatMessage::assistant("", Some(calls.to_vec()))
        .with_provider_content(Some(signature.clone()));
    let checkpoint = SubagentCheckpoint {
        messages: [
            ChatMessage::system("system"),
            ChatMessage::plain("user", "keep original task"),
            assistant,
            ChatMessage::tool("done", "written"),
        ]
        .iter()
        .map(SavedMessage::new)
        .collect(),
        ..Default::default()
    };
    let stored = serde_json::to_vec(&checkpoint).unwrap();
    let loaded: SubagentCheckpoint = serde_json::from_slice(&stored).unwrap();
    let messages = loaded.restore_messages();
    assert_eq!(messages[2].provider_content, Some(signature));
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.tool_call_id.as_deref() == Some("done"))
            .count(),
        1
    );
    assert_eq!(
        messages.last().unwrap().tool_call_id.as_deref(),
        Some("unknown")
    );
    assert!(serde_json::to_string(messages.last().unwrap())
        .unwrap()
        .contains("Inspect the actual state"));
}

/// 【子任务】【恢复回归】从含中断调用的检查点继续请求模型，不直接重放工具副作用。
#[tokio::test]
async fn resume_never_reexecutes_persisted_or_unconfirmed_tools() {
    let mut harness = Harness::new(|_| Vec::new(), unchanged_output).await;
    let owner = harness._root.path().join("session");
    let (snapshot, _cancel) = crate::tools::subagent_state::create_subagent_for_owner(
        &owner.display().to_string(),
        "recovery".into(),
        "general".into(),
        5,
    );
    harness.runner.checkpoint_id = Some(snapshot.id.clone());
    let call = crate::llm::ToolCall {
        id: "uncertain".into(),
        kind: "function".into(),
        function: crate::llm::ToolCallFunction {
            name: "run_command".into(),
            arguments: "{}".into(),
        },
    };
    let mut messages = harness.runner.initial_messages("original task");
    messages.push(ChatMessage::assistant("", Some(vec![call])));
    harness
        .runner
        .save_checkpoint(
            &messages,
            &SubagentStats::default(),
            &harness.runner.fresh_tool_visibility(),
            1,
            false,
            &[],
        )
        .unwrap();
    harness.run().await;
    assert_eq!(harness.executions.load(Ordering::SeqCst), 0);
    assert!(harness.requests.lock().unwrap()[0]
        .to_string()
        .contains("Inspect the actual state"));
    // 1. 【子任务】【恢复回归】段末已落盘时直接返回结果，不再请求模型
    let count = harness.requests.lock().unwrap().len();
    harness.run().await;
    assert_eq!(harness.requests.lock().unwrap().len(), count);
}

/// 【子任务】【压缩回归】使用实际模型请求压缩长历史，保留任务、最新指导和完整原文留档。
#[tokio::test]
async fn long_context_is_compacted_without_losing_task_or_latest_guidance() {
    let mut harness = Harness::new(|_| Vec::new(), unchanged_output).await;
    let mut config = harness.config.clone();
    config.context.default_max_chars = 8192;
    let paths = SaiPaths::for_tests(harness._root.path());
    harness.runner.progressive_context = Some((config, paths));
    let owner = harness._root.path().join("session");
    let (snapshot, _cancel) = crate::tools::subagent_state::create_subagent_for_owner(
        &owner.display().to_string(),
        "long context".into(),
        "general".into(),
        0,
    );
    harness.runner.checkpoint_id = Some(snapshot.id);
    let mut messages = harness.runner.initial_messages("original overall task");
    let history = (0..4000)
        .map(|i| format!("file-{i}: inspect result\n"))
        .collect::<String>();
    messages.push(ChatMessage::assistant(history.clone(), None));
    messages.push(ChatMessage::plain("user", "latest guidance: preserve API"));
    let mut stats = SubagentStats::default();
    let mut visibility = harness.runner.fresh_tool_visibility();
    assert!(harness
        .runner
        .compact_context(&mut messages, &mut stats, &mut visibility, true)
        .await
        .unwrap());
    let text = serde_json::to_string(&messages).unwrap();
    assert!(text.contains("original overall task"));
    assert!(text.contains("latest guidance: preserve API"));
    assert!(text.contains("Full prior context"));
    assert!(stats.total_tokens > 0);
    let archives = std::fs::read_dir(owner.join("subagent-context"))
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(archives.len(), 1);
    let archived = std::fs::read_to_string(archives[0].as_ref().unwrap().path()).unwrap();
    assert!(archived.contains("file-3999"));
    for request in harness.requests.lock().unwrap().iter() {
        assert!(!request.to_string().contains("image/png;base64"));
    }
}

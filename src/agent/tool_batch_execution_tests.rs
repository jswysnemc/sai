use super::*;
use crate::agent::AgentMode;
use crate::config::AppConfig;
use crate::llm::{OpenAiCompatibleClient, ToolCallFunction};
use crate::paths::SaiPaths;
use crate::state::StateStore;
use crate::tools::{ToolRegistry, ToolSpec};
use std::sync::Arc;
use tokio::sync::Notify;

/// 【工具并发】【真实边界回归】快工具必须先展示进度并落盘，慢工具才允许完成；模型消息保持原顺序。
#[tokio::test]
async fn fast_results_are_persisted_while_slow_tools_are_still_running() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig {
        load_instruction_files: false,
        ..Default::default()
    };
    let state = StateStore::new(&paths).unwrap();
    let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
    let progress_seen = Arc::new(Notify::new());
    let fast_committed = Arc::new(Notify::new());
    let wait_progress = progress_seen.clone();
    let wait_commit = fast_committed.clone();
    let mut tools = ToolRegistry::new();
    tools.register(ToolSpec::new_with_progress(
        "probe",
        "Read test",
        serde_json::json!({
            "type":"object","properties":{"slow":{"type":"boolean"}},"required":["slow"]
        }),
        move |args, progress| {
            let wait_progress = wait_progress.clone();
            let wait_commit = wait_commit.clone();
            async move {
                if args["slow"] == true {
                    wait_commit.notified().await;
                    Ok("slow result".into())
                } else {
                    progress.report("fast progress");
                    wait_progress.notified().await;
                    Ok("fast result".into())
                }
            }
        },
    ));
    let agent = Agent::new(config, &paths, state, client, tools, AgentMode::Yolo).unwrap();
    agent.state.start_turn("batch", "inspect").unwrap();
    let group = [("slow", true), ("fast", false)]
        .into_iter()
        .enumerate()
        .map(|(sequence, (id, slow))| {
            let call = ToolCall {
                id: id.into(),
                kind: "function".into(),
                function: ToolCallFunction {
                    name: "probe".into(),
                    arguments: serde_json::json!({"slow":slow}).to_string(),
                },
            };
            SequencedToolCall {
                sequence: sequence + 1,
                provider_call: call.clone(),
                execution_call: Ok(call),
            }
        })
        .collect();
    let mut order = Vec::new();
    let mut messages = Vec::new();
    let mut used = Vec::new();
    let mut guard = RepeatGuard::default();
    let mut perf = PerfTrace::new("concurrent-test");
    let mut sink = |event| {
        match event {
            AgentEvent::ToolProgressIdentified { id, message, .. } => {
                assert_eq!(id, "fast");
                assert_eq!(message, "fast progress");
                progress_seen.notify_one();
            }
            AgentEvent::ToolResultIdentified { id, .. } => {
                if id == "fast" {
                    let persisted = agent.state.project_running_turn_tool_messages("batch")?;
                    assert!(persisted
                        .iter()
                        .any(|message| message.tool_call_id.as_deref() == Some("fast")
                            && serde_json::to_string(message)
                                .unwrap()
                                .contains("fast result")));
                    fast_committed.notify_one();
                }
                order.push(id);
            }
            _ => {}
        }
        Ok(())
    };
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        agent.execute_concurrent_read_only_group(
            "batch",
            1,
            None,
            group,
            &mut used,
            &mut messages,
            &mut guard,
            &mut sink,
            &mut perf,
            &HookContext {
                session_id: agent.state.session_id().into(),
                workdir: root.path().display().to_string(),
                tool_name: None,
                extra: Default::default(),
            },
        ),
    )
    .await
    .expect("progress and fast completion must not wait for slow tool")
    .unwrap();
    assert_eq!(order, ["fast", "slow"]);
    assert_eq!(
        messages
            .iter()
            .filter_map(|message| message.tool_call_id.as_deref())
            .collect::<Vec<_>>(),
        ["slow", "fast"]
    );
}

/// 【目标】【并发暂停回归】前批工具暂停目标后，等待容量的工具必须拒绝启动并保留对应结果。
/// 参数：无；返回断言结果。
#[tokio::test]
async fn paused_goal_blocks_tools_waiting_for_concurrency_capacity() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig {
        load_instruction_files: false,
        ..Default::default()
    };
    let state = StateStore::new(&paths).unwrap();
    let goals = crate::goal::GoalStore::new(state.goal_file());
    let goal = goals.replace("inspect", None, false).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let control = goals.clone();
    let mut tools = ToolRegistry::new();
    tools.register(ToolSpec::new(
        "probe",
        "read",
        serde_json::json!({"type":"object"}),
        move |_| {
            let observed = observed.clone();
            let control = control.clone();
            async move {
                observed.fetch_add(1, Ordering::SeqCst);
                control.set_status(crate::goal::GoalStatus::Paused)?;
                Ok("first read".into())
            }
        },
    ));
    let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
    let mut agent = Agent::new(config, &paths, state, client, tools, AgentMode::Yolo).unwrap();
    agent.goal_accounting = Some(
        crate::goal::GoalAccounting::new(goals, "turn:pause-test".into(), Some(goal.id)).unwrap(),
    );
    agent.state.start_turn("batch", "inspect").unwrap();
    let group = (0..12)
        .map(|index| {
            let call = ToolCall {
                id: format!("read-{index}"),
                kind: "function".into(),
                function: ToolCallFunction {
                    name: "probe".into(),
                    arguments: format!("{{\"index\":{index}}}"),
                },
            };
            SequencedToolCall {
                sequence: index + 1,
                provider_call: call.clone(),
                execution_call: Ok(call),
            }
        })
        .collect();
    let mut messages = Vec::new();
    agent
        .execute_concurrent_read_only_group(
            "batch",
            1,
            None,
            group,
            &mut Vec::new(),
            &mut messages,
            &mut RepeatGuard::default(),
            &mut |_| Ok(()),
            &mut PerfTrace::new("pause-test"),
            &HookContext {
                session_id: agent.state.session_id().into(),
                workdir: root.path().display().to_string(),
                tool_name: None,
                extra: Default::default(),
            },
        )
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(messages.len(), 12);
    assert!(messages
        .iter()
        .skip(1)
        .all(|message| serde_json::to_string(message).unwrap().contains("paused")));
}

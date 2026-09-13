use super::lifecycle_support::{kinds, LifecycleFixture, PROBE};
use super::services_support::{tool_call, ModelFixture, ModelReply};
use crate::agent::AgentEvent;
use crate::tools::{ProgressMode, SubagentProgress, SubagentRunner, ToolProgress};
use serde_json::json;
use std::collections::BTreeSet;
use std::time::Duration;

/// 【生命周期验收】【主请求组合】普通监听器与命令包经过真实模型、工具和否决流程
/// @returns 无，宿主参数和业务结果保持原值，全部事件共享一次操作标识
#[tokio::test]
async fn installed_observer_tracks_real_agent_rounds_and_vetoes_without_rewriting_arguments() {
    let arguments = json!({"value":"original", "session_id":"forged-session",
        "operation_id":"forged-operation", "workdir":"/forged", "allow_writes":true});
    let mut veto = tool_call(0, PROBE, json!({"value":"denied","deny":true}));
    veto["id"] = json!("call-veto");
    let model = ModelFixture::start(vec![
        ModelReply::delta(json!({"tool_calls":[tool_call(0, PROBE, arguments.clone())]})),
        ModelReply::delta(json!({"tool_calls":[veto]})),
        ModelReply::text("final response stays outside lifecycle data"),
    ])
    .await;
    let fixture = LifecycleFixture::new(&model);
    let mut agent = fixture.agent(&model);
    let result = crate::runtime_cwd::scope(
        fixture.root.path().to_path_buf(),
        agent.chat_stream_with_images(
            "user message stays outside lifecycle data",
            vec![],
            Some("observer-main".into()),
            |_| Ok(()),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        result.content,
        "final response stays outside lifecycle data"
    );
    assert_eq!(*fixture.probes.lock().unwrap(), [arguments.clone()]);
    let report = fixture.report_agent(&agent).await;
    fixture.assert_contexts(&report, agent.session_id());
    assert_eq!(
        kinds(&report),
        [
            "agent_start",
            "turn_start",
            "message_start",
            "message_end",
            "turn_end",
            "tool_call",
            "tool_result",
            "turn_start",
            "message_start",
            "message_end",
            "turn_end",
            "tool_call",
            "tool_result",
            "turn_start",
            "message_start",
            "message_end",
            "turn_end",
            "agent_end",
        ]
    );
    let rows = report["rows"].as_array().unwrap();
    assert!(rows
        .iter()
        .all(|row| row["operation_id"] == rows[0]["operation_id"]));
    assert_eq!(rows[5]["data"]["arguments"], arguments);
    assert_eq!(
        rows[6]["data"],
        json!({"name":PROBE,"ok":true,"output":"probe:original"})
    );
    assert_eq!(rows[12]["data"]["ok"], false);
    assert!(rows[12]["data"]["output"]
        .as_str()
        .unwrap()
        .contains("observer denied probe"));
    assert_eq!(
        rows[17]["data"],
        json!({"turn_id":"observer-main","ok":true})
    );
    let rounds = rows
        .iter()
        .filter(|row| row["kind"] == "turn_start")
        .map(|row| row["data"]["round"].as_u64().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(rounds.len(), 3);
    for group in [&rows[1..5], &rows[7..11], &rows[13..17]] {
        assert!(group
            .iter()
            .all(|row| row["data"]["round"] == group[0]["data"]["round"]));
    }
    let requests = model.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests
        .iter()
        .all(|request| request["model"] == "lifecycle-main"));
    assert!(requests[1]["messages"]
        .to_string()
        .contains("probe:original"));
    assert!(requests[2]["messages"]
        .to_string()
        .contains("observer denied probe"));
    for request in requests {
        let names = request["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, [PROBE]);
    }
    assert!(!report.to_string().contains("stays outside lifecycle data"));
}

/// 【生命周期验收】【子任务隔离】空工具白名单仍保留监听器，子任务使用独立实例和会话
/// @returns 无，真实子任务模型触发配对事件，父任务记录不变
#[tokio::test]
async fn command_only_observer_survives_empty_subagent_tool_filter_and_isolates_state() {
    let model = ModelFixture::start(vec![ModelReply::text("child response")]).await;
    let fixture = LifecycleFixture::new(&model);
    let mut parent = fixture.registry();
    parent.start_plugin_session("parent-session").unwrap();
    let mut child = parent.clone_filtered(&[]);
    child
        .start_plugin_session("parent-session/subagent/fixture")
        .unwrap();
    assert!(child.definitions().is_empty());
    let runner = SubagentRunner::new(
        model.client("lifecycle-child", &fixture.paths),
        "Return the fixture response.",
        child.clone(),
        SubagentProgress::new(ToolProgress::default(), ProgressMode::Full, true),
    );
    let (result, _) = crate::runtime_cwd::scope(
        fixture.root.path().to_path_buf(),
        runner.run("child fixture"),
    )
    .await
    .unwrap();
    assert_eq!(result.content, "child response");
    let report = fixture.report_registry(&child).await;
    fixture.assert_contexts(&report, "parent-session/subagent/fixture");
    assert_eq!(
        kinds(&report),
        [
            "agent_start",
            "turn_start",
            "message_start",
            "message_end",
            "turn_end",
            "agent_end"
        ]
    );
    let rows = report["rows"].as_array().unwrap();
    assert!(rows.iter().all(|row| row["data"]["kind"] == "subagent"));
    assert!(rows
        .iter()
        .all(|row| row["operation_id"] == rows[0]["operation_id"]));
    assert_eq!(rows[5]["data"]["ok"], true);
    assert!(kinds(&fixture.report_registry(&parent).await).is_empty());
    let requests = model.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["model"], "lifecycle-child");
    assert!(requests[0]
        .get("tools")
        .is_none_or(|tools| tools.as_array().unwrap().is_empty()));
}

/// 【生命周期验收】【取消恢复】在真实模型流等待期间回收 Future，不补发结束事件
/// @returns 无，同一 Agent 的后续请求仍能执行，使用新的操作标识
#[tokio::test]
async fn cancelled_agent_stream_does_not_forge_end_events_and_next_turn_can_continue() {
    let model = ModelFixture::start(vec![
        ModelReply::text("partial response").hold_open(),
        ModelReply::text("recovered response"),
    ])
    .await;
    let fixture = LifecycleFixture::new(&model);
    let mut agent = fixture.agent(&model);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut sender = Some(sender);
    let mut operation = Box::pin(crate::runtime_cwd::scope(
        fixture.root.path().to_path_buf(),
        agent.chat_stream_with_images(
            "cancel fixture",
            vec![],
            Some("cancelled-turn".into()),
            |event| {
                if matches!(event, AgentEvent::Chunk(_)) {
                    if let Some(sender) = sender.take() {
                        let _ = sender.send(());
                    }
                }
                Ok(())
            },
        ),
    ));
    tokio::select! {
        result = &mut operation => panic!("held response completed before cancellation: {result:?}"),
        result = tokio::time::timeout(Duration::from_secs(3), receiver) => result.unwrap().unwrap(),
    }
    drop(operation);
    let cancelled = fixture.report_agent(&agent).await;
    fixture.assert_contexts(&cancelled, agent.session_id());
    assert_eq!(
        kinds(&cancelled),
        ["agent_start", "turn_start", "message_start"]
    );
    assert_eq!(model.requests().len(), 1);
    let result = crate::runtime_cwd::scope(
        fixture.root.path().to_path_buf(),
        agent.chat_stream_with_images(
            "continue fixture",
            vec![],
            Some("continued-turn".into()),
            |_| Ok(()),
        ),
    )
    .await
    .unwrap();
    assert_eq!(result.content, "recovered response");
    let resumed = fixture.report_agent(&agent).await;
    fixture.assert_contexts(&resumed, agent.session_id());
    assert_eq!(
        kinds(&resumed),
        [
            "agent_start",
            "turn_start",
            "message_start",
            "agent_start",
            "turn_start",
            "message_start",
            "message_end",
            "turn_end",
            "agent_end"
        ]
    );
    let rows = resumed["rows"].as_array().unwrap();
    assert_ne!(rows[0]["operation_id"], rows[3]["operation_id"]);
    assert!(rows[..3]
        .iter()
        .all(|row| row["operation_id"] == rows[0]["operation_id"]));
    assert!(rows[3..]
        .iter()
        .all(|row| row["operation_id"] == rows[3]["operation_id"]));
    assert_eq!(
        rows[8]["data"],
        json!({"turn_id":"continued-turn","ok":true})
    );
    assert_eq!(model.requests().len(), 2);
}

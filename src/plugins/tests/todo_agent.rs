use super::{
    services_support::{ModelFixture, ModelReply},
    todo_agent_support::*,
};
use crate::{agent::AgentMode, state::TurnStatus};
use serde_json::json;

/// 【待办 Agent 测试】【完整提醒链路】成功更新重置计数，失败更新参与计数，每循环只提醒一次
/// @returns 无；下一次用户对话重新计数，主回复和工具结果完整保留
#[tokio::test]
async fn todo_agent_reminds_once_per_loop_and_resets_after_updates() {
    let mut replies = vec![
        tool(1, "step", json!({"n":1})),
        tool(2, "step", json!({"n":2})),
        tool(
            3,
            "todo",
            json!({"action":"update","id":"a","text":"changed"}),
        ),
        tool(4, "step", json!({"n":3})),
        tool(5, "step", json!({"n":4})),
        tool(
            6,
            "todo",
            json!({"action":"update","id":"missing","text":"failed"}),
        ),
        tool(7, "step", json!({"n":5})),
        tool(8, "step", json!({"n":6})),
        tool(9, "step", json!({"n":7})),
        ModelReply::text("first answer"),
    ];
    for n in 8..=10 {
        replies.push(tool(n + 2, "step", json!({"n":n})));
    }
    replies.push(ModelReply::text("second answer"));
    let model = ModelFixture::start(replies).await;
    let root = tempfile::tempdir().unwrap();
    crate::runtime_cwd::scope(root.path().to_path_buf(), async {
        let (mut agent, state) = agent(root.path(), &model, AgentMode::Yolo, false);
        seed(root.path(), &state);
        for (id, answer) in [("first", "first answer"), ("second", "second answer")] {
            assert_eq!(
                agent
                    .chat_stream_with_images("Continue the plan", vec![], Some(id.into()), |_| Ok(
                        ()
                    ))
                    .await
                    .unwrap()
                    .content,
                answer
            );
        }
        assert!(state
            .load_turns()
            .unwrap()
            .iter()
            .all(|turn| turn.status == TurnStatus::Completed));
        let requests = model.requests();
        assert_eq!(requests.len(), 14);
        for (number, request) in requests.iter().enumerate() {
            let expected = usize::from((6..=9).contains(&number) || number == 13);
            assert_eq!(reminder_count(request), expected, "request {number}");
        }
        assert!(requests[6]["messages"]
            .to_string()
            .contains("todo item not found: missing"));
        let record: serde_json::Value = serde_json::from_slice(
            &std::fs::read(super::todo_support::record_path(
                &crate::paths::SaiPaths::for_tests(root.path()),
                &state.state_dir().display().to_string(),
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(record["items"][0]["text"], "changed");
    })
    .await;
}

/// 【待办 Agent 测试】【回调错误隔离】待办存储损坏不能中断普通工具或主模型回复
/// @returns 无；错误文件保持原样且不产生提醒
#[tokio::test]
async fn todo_agent_policy_failure_keeps_tools_and_reply_completed() {
    let model = ModelFixture::start(vec![
        tool(1, "step", json!({"n":1})),
        tool(2, "step", json!({"n":2})),
        tool(3, "step", json!({"n":3})),
        ModelReply::text("valid answer"),
    ])
    .await;
    let root = tempfile::tempdir().unwrap();
    crate::runtime_cwd::scope(root.path().to_path_buf(), async {
        let (mut agent, state) = agent(root.path(), &model, AgentMode::Yolo, false);
        let record = super::todo_support::record_path(
            &crate::paths::SaiPaths::for_tests(root.path()),
            &state.state_dir().display().to_string(),
        );
        std::fs::write(&record, b"{broken").unwrap();
        assert_eq!(
            agent
                .chat_stream_with_images("Continue", vec![], Some("broken-policy".into()), |_| Ok(
                    ()
                ))
                .await
                .unwrap()
                .content,
            "valid answer"
        );
        assert_eq!(state.load_turns().unwrap()[0].status, TurnStatus::Completed);
        assert_eq!(std::fs::read(record).unwrap(), b"{broken");
        let requests = model.requests();
        assert_eq!(requests.len(), 4);
        assert!(requests.iter().all(|request| reminder_count(request) == 0));
        assert!(requests[3]["messages"].to_string().contains("step 3"));
    })
    .await;
}

/// 【待办 Agent 测试】【不可用工具】计划模式及白名单排除待办时不能提示调用不可用工具
/// @returns 无；旧待办文件不因被过滤的策略发生导入
#[tokio::test]
async fn todo_agent_respects_plan_mode_and_filtered_tool_catalogs() {
    for (mode, excluded) in [(AgentMode::Plan, false), (AgentMode::Yolo, true)] {
        let model = ModelFixture::start(vec![
            tool(1, "step", json!({"n":1})),
            tool(2, "step", json!({"n":2})),
            tool(3, "step", json!({"n":3})),
            ModelReply::text("readonly answer"),
        ])
        .await;
        let root = tempfile::tempdir().unwrap();
        crate::runtime_cwd::scope(root.path().to_path_buf(), async {
            let (mut agent, state) = agent(root.path(), &model, mode, excluded);
            seed(root.path(), &state);
            assert_eq!(
                agent
                    .chat_stream_with_images("Inspect", vec![], Some("filtered".into()), |_| Ok(()))
                    .await
                    .unwrap()
                    .content,
                "readonly answer"
            );
            assert!(super::todo_support::record_path(
                &crate::paths::SaiPaths::for_tests(root.path()),
                &state.state_dir().display().to_string()
            )
            .exists());
            let requests = model.requests();
            assert!(requests.iter().all(|request| reminder_count(request) == 0));
            assert!(requests.iter().all(|request| !request["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["function"]["name"] == "lua__todo__todo")));
        })
        .await;
    }
}

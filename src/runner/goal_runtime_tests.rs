use super::automatic_test_support::{AutomaticTestHarness, TestResponse};
use super::*;
use crate::agent::{AgentEvent, AgentMode};
use crate::goal::GoalStatus;

/// 【目标】【预算测试】达到预算后不能执行响应中的工具，收尾用量仍需计入；无参数、无返回值
#[tokio::test]
async fn budget_stops_tools_and_accounts_final_summary() {
    let mut harness = AutomaticTestHarness::new().await;
    harness
        .agent
        .state()
        .replace_goal("finish original objective", Some(18), false)
        .unwrap();
    harness.respond_with([TestResponse::ToolCall, TestResponse::Reply]);
    harness
        .submit(UserInputSubmission::new("continue", AgentMode::Yolo))
        .await
        .unwrap();
    let goal = harness.agent.state().goal().unwrap().unwrap();
    assert_eq!(goal.status, GoalStatus::BudgetLimited);
    assert_eq!(goal.tokens_used, 36);
    assert!(!harness.events.iter().any(|event| matches!(event,
        RunnerEvent::Agent(AgentEvent::ToolCall { name, .. }) if name == "probe")));
    assert!(harness.last_request()["tools"].is_null());
}

/// 【目标】【指导测试】追加要求保持目标和预算，单轮回复结束不代表目标完成；无参数、无返回值
#[tokio::test]
async fn guidance_preserves_objective_and_goal_continues_between_turns() {
    let mut harness = AutomaticTestHarness::new().await;
    let initial = harness
        .agent
        .state()
        .replace_goal("finish original objective", Some(1000), false)
        .unwrap();
    harness
        .submit(UserInputSubmission::new(
            "优先检查 Windows",
            AgentMode::Yolo,
        ))
        .await
        .unwrap();
    let goal = harness.agent.state().goal().unwrap().unwrap();
    assert_eq!(goal.id, initial.id);
    assert_eq!(goal.objective, initial.objective);
    assert_eq!(goal.status, GoalStatus::Active);
    assert_eq!(goal.tokens_used, 18);
    assert_eq!(harness.request_count(), 1);
    let request = harness.last_request().to_string();
    assert!(request.contains("finish original objective"));
    assert!(request.contains("优先检查 Windows"));
    assert!(request.contains("User follow-up messages provide guidance"));
    harness
        .submit(UserInputSubmission::new("", AgentMode::Yolo).with_goal_continuation())
        .await
        .unwrap();
    assert_eq!(harness.request_count(), 2);
    assert_eq!(
        harness.agent.state().goal().unwrap().unwrap().tokens_used,
        36
    );
}

/// 【目标】【暂停测试】用户请求暂停后停止工具并保留累计用量；无参数、无返回值
#[tokio::test]
async fn explicit_pause_stops_work_and_keeps_usage() {
    let mut harness = AutomaticTestHarness::new().await;
    harness
        .agent
        .state()
        .replace_goal("finish original objective", None, false)
        .unwrap();
    harness.respond_with([TestResponse::PauseGoal, TestResponse::Reply]);
    harness
        .submit(UserInputSubmission::new("暂停当前目标", AgentMode::Yolo))
        .await
        .unwrap();
    let goal = harness.agent.state().goal().unwrap().unwrap();
    assert_eq!(goal.status, GoalStatus::Paused);
    assert_eq!(goal.tokens_used, 18);
    let before = harness.request_count();
    harness
        .submit(UserInputSubmission::new("", AgentMode::Yolo).with_goal_continuation())
        .await
        .unwrap();
    assert_eq!(harness.request_count(), before);
}

/// 【目标】【错误核算】后续请求失败仍保留此前已消耗用量；无参数、无返回值
#[tokio::test]
async fn failed_followup_keeps_consumed_tokens() {
    let mut harness = AutomaticTestHarness::new().await;
    harness
        .agent
        .state()
        .replace_goal("finish original objective", None, false)
        .unwrap();
    harness.respond_with([TestResponse::ToolCall, TestResponse::Error]);
    assert!(harness
        .submit(UserInputSubmission::new("continue", AgentMode::Yolo))
        .await
        .is_err());
    let goal = harness.agent.state().goal().unwrap().unwrap();
    assert_eq!(goal.tokens_used, 18);
    assert_eq!(goal.status, GoalStatus::Blocked);
}

/// 【目标】【空响应测试】连续三次自动空响应必须停止续轮；无参数、无返回值
#[tokio::test]
async fn three_empty_automatic_turns_block_the_goal() {
    let mut harness = AutomaticTestHarness::new().await;
    harness
        .agent
        .state()
        .replace_goal("finish original objective", None, false)
        .unwrap();
    for index in 1..=3 {
        harness.respond_with([TestResponse::Empty]);
        harness
            .submit(UserInputSubmission::new("", AgentMode::Yolo).with_goal_continuation())
            .await
            .unwrap();
        assert_eq!(
            harness.agent.state().goal().unwrap().unwrap().status,
            if index == 3 {
                GoalStatus::Blocked
            } else {
                GoalStatus::Active
            }
        );
    }
}

/// 【目标】【规划模式】自动续轮在规划模式不发送请求，也不改变目标状态或用量。
#[tokio::test]
async fn plan_mode_defers_goal_continuation_without_spending_tokens() {
    let mut harness = AutomaticTestHarness::new().await;
    let goal = harness
        .agent
        .state()
        .replace_goal("finish implementation", Some(100), false)
        .unwrap();
    harness.agent.apply_live_mode(AgentMode::Plan);
    harness.agent.resume_external_goal();
    assert!(tokio::time::timeout(
        std::time::Duration::from_millis(30),
        harness.agent.external_event_monitor().wait_for_wake()
    )
    .await
    .is_err());
    harness
        .submit(UserInputSubmission::new("", AgentMode::Plan).with_goal_continuation())
        .await
        .unwrap();
    assert_eq!(harness.request_count(), 0);
    let saved = harness.agent.state().goal().unwrap().unwrap();
    assert_eq!(saved.id, goal.id);
    assert_eq!(saved.status, GoalStatus::Active);
    assert_eq!(saved.tokens_used, 0);
}

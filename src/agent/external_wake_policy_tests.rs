use super::*;
use crate::agent::AgentMode;
use crate::llm::OpenAiCompatibleClient;
use crate::runner::UserInputSubmission;
use crate::tools::command::BackgroundCommandStore;
use crate::tools::ToolRegistry;

#[path = "../runner/automatic_test_support.rs"]
#[allow(dead_code)]
mod support;
use support::AutomaticTestHarness;

#[path = "external_wake_sources_tests.rs"]
mod sources;

/// 【会话重开】【测试环境】销毁旧 Agent 并从同一份持久化记录重新打开会话。
/// 参数: harness 为带本地模拟接口的真实会话
/// 返回: 无；重新打开失败时终止测试
fn reopen(harness: &mut AutomaticTestHarness) {
    let state = StateStore::for_session(&harness.paths, harness.agent.session_id()).unwrap();
    let client = OpenAiCompatibleClient::from_config(&harness.config, &harness.paths).unwrap();
    let mut tools = ToolRegistry::new();
    crate::tools::command::register_session_background(
        &mut tools,
        &harness.config,
        &harness.paths,
        state.session_id(),
    );
    harness.agent = Agent::new(
        harness.config.clone(),
        &harness.paths,
        state,
        client,
        tools,
        AgentMode::Yolo,
    )
    .unwrap();
}

/// 【会话重开】【旧回执】重新打开不能把未确认的旧回执转换为新模型请求。
/// 参数: 无
/// 返回: 无；模型请求数增加或回执结果丢失时断言失败
#[tokio::test]
async fn reopening_does_not_start_from_old_completion() {
    let mut harness = AutomaticTestHarness::new().await;
    harness.prime().await;
    let _batch = harness.background_notice().await;
    let before = harness.request_count();
    reopen(&mut harness);
    if let ExternalEventPoll::Ready(ExternalEventWake::Completion(batch)) = harness
        .agent
        .external_event_monitor()
        .poll_once()
        .await
        .unwrap()
    {
        harness
            .submit(UserInputSubmission::new("", AgentMode::Yolo).with_external_event_batch(batch))
            .await
            .unwrap();
    }
    assert_eq!(
        harness.request_count(),
        before,
        "重新打开只应恢复历史，不能发起后台回执对话"
    );
    let tasks = BackgroundCommandStore::new(harness.paths.state_dir.clone())
        .load()
        .unwrap();
    assert_eq!(tasks.len(), 1, "后台任务记录必须保留供查看");
}

/// 【会话重开】【迟到快照】旧监听已经排队的回执在新 Agent 上执行前必须失效。
/// 参数: 无
/// 返回: 无；旧自动输入调用模型时断言失败
#[tokio::test]
async fn reopening_rejects_previously_queued_completion() {
    let mut harness = AutomaticTestHarness::new().await;
    harness.prime().await;
    let batch = harness.background_notice().await;
    let before = harness.request_count();
    reopen(&mut harness);
    harness
        .submit(UserInputSubmission::new("", AgentMode::Yolo).with_external_event_batch(batch))
        .await
        .unwrap();
    assert_eq!(
        harness.request_count(),
        before,
        "旧回执不能在执行入口绕过重新打开的边界"
    );
}

/// 【会话重开】【迟到完成】关闭前的任务在重新打开后结束，也不能恢复自动对话。
/// 参数: 无
/// 返回: 无；旧任务结束后产生自动唤醒时断言失败
#[tokio::test]
async fn reopening_does_not_adopt_preexisting_running_task() {
    let mut harness = AutomaticTestHarness::new().await;
    harness.prime().await;
    let _batch = harness.background_notice().await;
    let store = BackgroundCommandStore::new(harness.paths.state_dir.clone());
    store
        .update(|tasks| {
            tasks[0].status = "running".to_string();
            tasks[0].pid = std::process::id();
            Ok(())
        })
        .unwrap();
    reopen(&mut harness);
    store
        .update(|tasks| {
            tasks[0].status = "exited".to_string();
            Ok(())
        })
        .unwrap();
    assert!(
        !matches!(
            harness
                .agent
                .external_event_monitor()
                .poll_once()
                .await
                .unwrap(),
            ExternalEventPoll::Ready(_)
        ),
        "旧任务的完成时间不能重新授权自动对话"
    );
}

/// 【会话重开】【旧监听】关闭后保留的监听句柄不能读取或确认任何回执。
/// 参数: 无
/// 返回: 无；旧监听仍然有效或修改确认状态时断言失败
#[tokio::test]
async fn reopening_revokes_old_monitor_without_acknowledging_results() {
    let mut harness = AutomaticTestHarness::new().await;
    let batch = harness.background_notice().await;
    let old_monitor = harness.agent.external_event_monitor();
    reopen(&mut harness);
    assert!(matches!(
        old_monitor.poll_once().await.unwrap(),
        ExternalEventPoll::Idle
    ));
    assert!(old_monitor.wait_for_wake().await.unwrap().is_none());
    assert!(!old_monitor.is_pending(&batch).unwrap());
    old_monitor.acknowledge(&batch).unwrap();
    let tasks = BackgroundCommandStore::new(harness.paths.state_dir.clone())
        .load()
        .unwrap();
    assert!(!tasks[0].completion_notified);
    assert_eq!(
        std::fs::read_to_string(&tasks[0].stdout_log).unwrap(),
        "command output\n"
    );
}

/// 【会话切换】【回执隔离】切换离开再返回时，旧监听和已排队快照均失效。
/// 参数: 无
/// 返回: 无；切换后旧通知触发请求时断言失败
#[tokio::test]
async fn switching_away_and_back_rejects_old_completion() {
    let mut harness = AutomaticTestHarness::new().await;
    let batch = harness.background_notice().await;
    let original = harness.agent.state().clone();
    let monitor = harness.agent.external_event_monitor();
    let next_session = crate::state::create_session(&harness.paths, Some("切换测试")).unwrap();
    let next = StateStore::for_session(&harness.paths, &next_session.id).unwrap();
    next.init_files().unwrap();
    harness.agent.replace_state(next).unwrap();
    assert!(matches!(
        monitor.poll_once().await.unwrap(),
        ExternalEventPoll::Idle
    ));
    harness.agent.replace_state(original).unwrap();
    harness
        .submit(UserInputSubmission::new("", AgentMode::Yolo).with_external_event_batch(batch))
        .await
        .unwrap();
    assert_eq!(harness.request_count(), 0);
    assert!(matches!(
        harness
            .agent
            .external_event_monitor()
            .poll_once()
            .await
            .unwrap(),
        ExternalEventPoll::Idle
    ));
}

/// 【会话重开】【新任务】重新打开后创建的任务仍按正常流程投递并确认。
/// 参数: 无
/// 返回: 无；新任务未投递、重复投递或错误消费旧任务时断言失败
#[tokio::test]
async fn new_completion_after_reopen_is_delivered_once() {
    let mut harness = AutomaticTestHarness::new().await;
    harness.prime().await;
    harness.background_notice().await;
    let store = BackgroundCommandStore::new(harness.paths.state_dir.clone());
    store
        .update(|tasks| {
            tasks[0].id = "old-task".into();
            Ok(())
        })
        .unwrap();
    reopen(&mut harness);
    let batch = harness.background_notice().await;
    let before = harness.request_count();
    let input = UserInputSubmission::new("", AgentMode::Yolo).with_external_event_batch(batch);
    harness.submit(input.clone()).await.unwrap();
    harness.submit(input).await.unwrap();
    assert_eq!(harness.request_count(), before + 1);
    let tasks = store.load().unwrap();
    assert!(
        !tasks
            .iter()
            .find(|task| task.id == "old-task")
            .unwrap()
            .completion_notified
    );
    assert!(
        tasks
            .iter()
            .find(|task| task.id == "completed-task")
            .unwrap()
            .completion_notified
    );
}

/// 【会话重开】【Web 运行】Web 重建 Agent 后只执行本次输入，不注入旧后台回执。
/// 参数: 无
/// 返回: 无；Web 运行额外请求模型或消费旧通知时断言失败
#[tokio::test]
async fn web_runner_reopen_skips_old_completions_and_queued_snapshots() {
    use crate::runner::{RunnerSubmission, SessionRunner, SubmissionSource};
    let mut harness = AutomaticTestHarness::new().await;
    harness.prime().await;
    let batch = harness.background_notice().await;
    let before = harness.request_count();
    let mut tools = ToolRegistry::new();
    crate::tools::command::register_session_background(
        &mut tools,
        &harness.config,
        &harness.paths,
        harness.agent.session_id(),
    );
    let runner = SessionRunner::new(&harness.paths)
        .with_config(harness.config.clone())
        .with_tool_registry(tools);
    let mut sink = |_| Ok(());
    for input in [
        UserInputSubmission::new("回答本次问题", AgentMode::Yolo),
        UserInputSubmission::new("", AgentMode::Yolo).with_external_event_batch(batch),
    ] {
        tokio::time::timeout(
            Duration::from_secs(10),
            runner.run_submission(
                RunnerSubmission::user_input(SubmissionSource::Web, input)
                    .with_session_id(harness.agent.session_id()),
                &mut sink,
            ),
        )
        .await
        .unwrap()
        .unwrap();
    }
    assert_eq!(harness.request_count(), before + 1);
    let tasks = BackgroundCommandStore::new(harness.paths.state_dir.clone())
        .load()
        .unwrap();
    assert!(!tasks[0].completion_notified);
}

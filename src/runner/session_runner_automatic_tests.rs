use super::*;
use crate::agent::{AgentEvent, ExternalEventBatch};

#[path = "automatic_test_support.rs"]
mod support;
use support::{AutomaticTestHarness, TestResponse};

const COMPLETION_PROMPT: &str =
    "<external-completion-events>后台命令 task-1 已退出，请处理结果</external-completion-events>";

/// 【自动续聊】【测试输入】把完成通知转换为真实 REPL 使用的自动输入。
///
/// 参数: batch 为已排队通知
/// 返回: 携带消费确认标识的自动输入
fn completion_input(batch: &ExternalEventBatch) -> UserInputSubmission {
    UserInputSubmission::new("", AgentMode::Yolo).with_external_event_batch(batch.clone())
}

/// 【自动续聊】【请求验证】验证通知正文实际进入上一轮助手回复之后的请求。
///
/// 参数: 无
/// 返回: 无；遗漏通知提示或请求以助手消息结尾时断言失败
#[tokio::test]
async fn automatic_completion_reaches_provider_after_assistant_reply() {
    let mut harness = AutomaticTestHarness::new().await;
    harness.prime().await;
    let before = harness.request_count();
    harness
        .submit(
            UserInputSubmission::new("", AgentMode::Yolo)
                .with_external_event(COMPLETION_PROMPT, "后台命令已完成"),
        )
        .await
        .unwrap();
    assert_eq!(harness.request_count(), before + 1);
    let request = harness.last_request();
    let last = request["messages"].as_array().unwrap().last().unwrap();
    assert_eq!(last["role"], "user");
    assert_eq!(last["content"], COMPLETION_PROMPT);
}

/// 【自动续聊】【单次消费】验证外层唤醒与请求间隙不会重复投递同一后台通知。
///
/// 参数: 无
/// 返回: 无；请求、界面或持久化消费出现重复时断言失败
#[tokio::test]
async fn automatic_background_completion_is_delivered_once() {
    let mut harness = AutomaticTestHarness::new().await;
    harness.prime().await;
    let batch = harness.background_notice().await;
    let before = harness.request_count();
    harness.submit(completion_input(&batch)).await.unwrap();

    assert_eq!(harness.request_count(), before + 1);
    assert_eq!(harness.prompt_occurrences(batch.prompt()), 1);
    assert_eq!(
        harness.events.iter().filter(|event| matches!(
            event,
            RunnerEvent::Agent(AgentEvent::InterMessage(message)) if message.id == batch.event_id()
        )).count(),
        1
    );
    assert!(!harness
        .agent
        .external_event_monitor()
        .is_pending(&batch)
        .unwrap());
}

/// 【自动续聊】【迟到通知】验证显式清理后的唤醒不会发出空请求。
///
/// 参数: 无
/// 返回: 无；清理后仍调用模型时断言失败
#[tokio::test]
async fn automatic_cleaned_completion_skips_provider() {
    let mut harness = AutomaticTestHarness::new().await;
    harness.prime().await;
    let batch = harness.background_notice().await;
    crate::tools::command::cleanup_background_tasks_for_user(
        &harness.paths,
        &harness.config,
        false,
    )
    .await
    .unwrap();
    let before = harness.request_count();
    harness.submit(completion_input(&batch)).await.unwrap();
    assert_eq!(harness.request_count(), before);
}

/// 【自动续聊】【迟到通知】验证通知排队后显式读取输出不会再触发自动消费。
///
/// 参数: 无
/// 返回: 无；结果已读取后仍调用模型时断言失败
#[tokio::test]
async fn automatic_read_completion_skips_provider() {
    let mut harness = AutomaticTestHarness::new().await;
    harness.prime().await;
    let batch = harness.background_notice().await;
    crate::tools::command::read_background_task_output_for_user(
        &harness.paths,
        &harness.config,
        batch.event_id(),
        "all",
        50,
    )
    .await
    .unwrap();
    let before = harness.request_count();
    harness.submit(completion_input(&batch)).await.unwrap();
    assert_eq!(harness.request_count(), before);
}

/// 【自动续聊】【失败重试】验证失败请求保留通知，重试请求只携带一份正文。
///
/// 参数: 无
/// 返回: 无；请求失败导致通知丢失或重试正文重复时断言失败
#[tokio::test]
async fn automatic_failed_request_keeps_completion_for_retry() {
    let mut harness = AutomaticTestHarness::new().await;
    harness.prime().await;
    let batch = harness.background_notice().await;
    harness.respond_with([TestResponse::Error]);
    assert!(harness.submit(completion_input(&batch)).await.is_err());
    assert!(harness
        .agent
        .external_event_monitor()
        .is_pending(&batch)
        .unwrap());

    harness.submit(completion_input(&batch)).await.unwrap();
    assert_eq!(harness.prompt_occurrences(batch.prompt()), 1);
    assert!(!harness
        .agent
        .external_event_monitor()
        .is_pending(&batch)
        .unwrap());
}

/// 【自动续聊】【确认边界】验证后续工具轮次失败不会撤销已成功请求的通知确认。
///
/// 参数: 无
/// 返回: 无；整轮失败导致已交付通知再次排队时断言失败
#[tokio::test]
async fn automatic_acknowledgement_survives_later_request_failure() {
    let mut harness = AutomaticTestHarness::new().await;
    harness.prime().await;
    let batch = harness.background_notice().await;
    harness.respond_with([TestResponse::ToolCall, TestResponse::Error]);
    let before = harness.request_count();
    assert!(harness.submit(completion_input(&batch)).await.is_err());
    assert_eq!(harness.request_count(), before + 2);
    assert!(!harness
        .agent
        .external_event_monitor()
        .is_pending(&batch)
        .unwrap());
    assert_eq!(harness.prompt_occurrences(batch.prompt()), 1);
    let before_retry = harness.request_count();
    harness.submit(completion_input(&batch)).await.unwrap();
    assert_eq!(harness.request_count(), before_retry);
}

/// 【自动续聊】【目标续作】验证活动目标的续作提示进入请求，结束后不再发送空轮次。
///
/// 参数: 无
/// 返回: 无；缺少目标提示或目标结束后仍请求模型时断言失败
#[tokio::test]
async fn automatic_goal_continuation_requires_active_goal() {
    let mut harness = AutomaticTestHarness::new().await;
    harness.prime().await;
    // 1. 【自动续聊】【目标续作】关闭后台轮询，隔离单次续作输入的请求边界
    harness
        .agent
        .switch_mode(AgentMode::Yolo, ToolRegistry::new())
        .unwrap();
    let goal = harness
        .agent
        .state()
        .replace_goal("完成回归验证", None, false)
        .unwrap();
    harness
        .submit(UserInputSubmission::new("", AgentMode::Yolo).with_goal_continuation())
        .await
        .unwrap();
    assert_eq!(
        harness.prompt_occurrences(&crate::goal::continuation_prompt(&goal)),
        1
    );
    harness
        .agent
        .state()
        .set_goal_status(crate::goal::GoalStatus::Complete)
        .unwrap();
    let before = harness.request_count();
    harness
        .submit(UserInputSubmission::new("", AgentMode::Yolo).with_goal_continuation())
        .await
        .unwrap();
    assert_eq!(harness.request_count(), before);
}

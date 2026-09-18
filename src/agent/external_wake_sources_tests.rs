use super::*;
use crate::tools::subagent_state::{
    create_subagent_for_owner_goal, drain_subagent_inbox, finish_subagent, park_subagent,
    pending_finished_notices, queue_subagent_message, resume_subagent,
};
use std::io::Write;

/// 【会话重开】【子任务夹具】登记隔离的子任务记录，不启动实际工作进程。
/// 参数: harness 为会话，persistent 控制任务是否可接收后续消息
/// 返回: 子任务标识
fn child_task(harness: &AutomaticTestHarness, persistent: bool) -> String {
    let owner = harness.agent.state().state_dir().display().to_string();
    create_subagent_for_owner_goal(
        &owner,
        None,
        "验证回执".into(),
        "general".into(),
        1,
        persistent,
    )
    .0
    .id
}

/// 【会话重开】【子任务回执】旧任务迟到完成不能唤醒，新任务完成仍可投递。
/// 参数: 无
/// 返回: 无；旧回执触发运行或新回执丢失时断言失败
#[tokio::test]
async fn old_child_completion_is_retained_and_new_child_is_delivered() {
    let mut harness = AutomaticTestHarness::new().await;
    let old = child_task(&harness, false);
    reopen(&mut harness);
    finish_subagent(&old, "completed", Some("旧结果".into()), None, None);
    assert!(matches!(
        harness
            .agent
            .external_event_monitor()
            .poll_once()
            .await
            .unwrap(),
        ExternalEventPoll::Idle
    ));
    let new = child_task(&harness, false);
    finish_subagent(&new, "completed", Some("新结果".into()), None, None);
    let ExternalEventPoll::Ready(ExternalEventWake::Completion(batch)) = harness
        .agent
        .external_event_monitor()
        .poll_once()
        .await
        .unwrap()
    else {
        panic!("本次打开后创建的子任务应投递回执");
    };
    assert_eq!(batch.event_id(), new);
    harness
        .submit(UserInputSubmission::new("", AgentMode::Yolo).with_external_event_batch(batch))
        .await
        .unwrap();
    assert_eq!(harness.request_count(), 1);
    let owner = harness.agent.state().state_dir().display().to_string();
    let pending = pending_finished_notices(&owner);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, old);
}

/// 【会话重开】【旧追加消息】旧任务与关闭前排队的追加消息都不能恢复自动对话。
/// 参数: 无
/// 返回: 无；旧追加消息完成后出现自动唤醒时断言失败
#[tokio::test]
async fn persistent_child_old_queued_followup_cannot_wake_reopened_session() {
    let mut harness = AutomaticTestHarness::new().await;
    let child = child_task(&harness, true);
    queue_subagent_message(&child, "parent", "关闭前追加的任务").unwrap();
    reopen(&mut harness);
    // 1. 【会话重开】【旧任务完成】原任务结束后，工作进程才读取此前排队的消息
    assert!(park_subagent(&child, Some("原任务完成".into()), None));
    assert_eq!(drain_subagent_inbox(&child).len(), 1);
    assert!(resume_subagent(&child));
    assert!(park_subagent(&child, Some("旧追加任务完成".into()), None));
    assert!(
        matches!(
            harness
                .agent
                .external_event_monitor()
                .poll_once()
                .await
                .unwrap(),
            ExternalEventPoll::Idle
        ),
        "旧消息对应的第二次完成不能当作新工作"
    );
}

/// 【会话重开】【新追加消息】已有持久子任务收到新指令后，成功和失败回执都应保留。
/// 参数: 无
/// 返回: 无；新任务失败后没有通知，或成功通知丢失时断言失败
#[tokio::test]
async fn persistent_child_new_followup_delivers_success_and_failure() {
    for failed in [false, true] {
        let mut harness = AutomaticTestHarness::new().await;
        let child = child_task(&harness, true);
        assert!(park_subagent(&child, Some("旧任务完成".into()), None));
        reopen(&mut harness);
        queue_subagent_message(&child, "user", "重新打开后主动追加的任务").unwrap();
        assert!(
            matches!(
                harness
                    .agent
                    .external_event_monitor()
                    .poll_once()
                    .await
                    .unwrap(),
                ExternalEventPoll::Idle
            ),
            "新消息尚未执行时不能重发旧结果"
        );
        assert!(resume_subagent(&child));
        assert_eq!(drain_subagent_inbox(&child).len(), 1);
        if failed {
            finish_subagent(&child, "failed", None, Some("测试失败".into()), None);
        } else {
            assert!(park_subagent(&child, Some("新任务完成".into()), None));
        }
        let ExternalEventPoll::Ready(ExternalEventWake::Completion(batch)) = harness
            .agent
            .external_event_monitor()
            .poll_once()
            .await
            .unwrap()
        else {
            panic!("新追加任务的结果必须投递，failed={failed}");
        };
        assert_eq!(batch.event_id(), child);
        assert!(harness
            .agent
            .external_event_monitor()
            .is_pending(&batch)
            .unwrap());
    }
}

/// 【会话重开】【信箱夹具】把入站消息写入临时会话信箱，模拟跨进程投递。
/// 参数: harness 为隔离会话，id 为消息标识
/// 返回: 无；写入失败时终止测试
fn incoming_message(harness: &AutomaticTestHarness, id: &str) {
    let dir = harness.agent.state().state_dir().join("inbox");
    std::fs::create_dir_all(&dir).unwrap();
    let envelope = crate::tools::mesh::MeshEnvelope {
        id: id.into(),
        correlation_id: None,
        reply_to: None,
        from: "session:sender".into(),
        to: format!("session:{}", harness.agent.session_id()),
        kind: "message".into(),
        text: format!("回执 {id}"),
        queued_at_ms: 1,
        pid: std::process::id(),
        heartbeat_at: 1,
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("mesh.jsonl"))
        .unwrap();
    writeln!(file, "{}", serde_json::to_string(&envelope).unwrap()).unwrap();
}

/// 【会话重开】【信箱回执】保留旧信箱消息并跳过它们，避免阻塞新消息。
/// 参数: 无
/// 返回: 无；旧消息被消费或新消息无法投递时断言失败
#[tokio::test]
async fn old_mailbox_messages_do_not_wake_or_block_new_messages() {
    let mut harness = AutomaticTestHarness::new().await;
    incoming_message(&harness, "old-message");
    reopen(&mut harness);
    let monitor = harness.agent.external_event_monitor();
    assert!(matches!(
        monitor.poll_once().await.unwrap(),
        ExternalEventPoll::Idle
    ));
    incoming_message(&harness, "new-message");
    let ExternalEventPoll::Ready(ExternalEventWake::Completion(batch)) =
        monitor.poll_once().await.unwrap()
    else {
        panic!("新入站消息应投递");
    };
    assert_eq!(batch.event_id(), "new-message");
    harness
        .submit(UserInputSubmission::new("", AgentMode::Yolo).with_external_event_batch(batch))
        .await
        .unwrap();
    assert_eq!(harness.request_count(), 1);
    let pending = crate::tools::mesh::pending_messages(
        harness.agent.state().state_dir(),
        harness.agent.session_id(),
    );
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "old-message");
}

/// 【会话重开】【目标恢复】旧目标只有在用户主动提交后才允许继续消费回执。
/// 参数: 无
/// 返回: 无；重开激活目标或主动恢复后遗漏回执时断言失败
#[tokio::test]
async fn reopened_goal_waits_for_user_before_accepting_old_completion() {
    let mut harness = AutomaticTestHarness::new().await;
    harness.background_notice().await;
    let goal = harness
        .agent
        .state()
        .replace_goal("完成已有目标", None, false)
        .unwrap();
    let store = BackgroundCommandStore::new(harness.paths.state_dir.clone());
    store
        .update(|tasks| {
            tasks[0].goal_id = Some(goal.id.clone());
            Ok(())
        })
        .unwrap();
    harness
        .agent
        .state()
        .set_goal_status(crate::goal::GoalStatus::Blocked)
        .unwrap();
    reopen(&mut harness);
    let monitor = harness.agent.external_event_monitor();
    assert!(matches!(
        monitor.poll_once().await.unwrap(),
        ExternalEventPoll::Idle
    ));
    assert_eq!(
        harness.agent.state().goal().unwrap().unwrap().status,
        crate::goal::GoalStatus::Blocked
    );
    // 1. 【会话重开】【主动恢复】隔离单次用户输入，随后检查真实监听器是否重新接收目标回执
    harness
        .agent
        .switch_mode(AgentMode::Yolo, ToolRegistry::new())
        .unwrap();
    harness
        .submit(UserInputSubmission::new("继续当前目标", AgentMode::Yolo))
        .await
        .unwrap();
    let ExternalEventPoll::Ready(ExternalEventWake::Completion(batch)) =
        monitor.poll_once().await.unwrap()
    else {
        panic!("用户主动继续后应恢复目标回执");
    };
    assert_eq!(batch.event_id(), "completed-task");
    assert_eq!(
        harness.agent.state().goal().unwrap().unwrap().status,
        crate::goal::GoalStatus::Active
    );
}

/// 【会话重开】【目标续作】仅打开已有活动目标不会续作，本次新建目标仍可正常运行。
/// 参数: 无
/// 返回: 无；旧目标自行续作或新目标无法续作时断言失败
#[tokio::test]
async fn reopening_active_goal_does_not_continue_but_new_goal_can() {
    let mut harness = AutomaticTestHarness::new().await;
    harness
        .agent
        .state()
        .replace_goal("旧目标", None, false)
        .unwrap();
    reopen(&mut harness);
    let monitor = harness.agent.external_event_monitor();
    assert!(matches!(
        monitor.poll_once().await.unwrap(),
        ExternalEventPoll::Idle
    ));
    harness
        .agent
        .state()
        .set_goal_status(crate::goal::GoalStatus::Complete)
        .unwrap();
    harness
        .agent
        .state()
        .replace_goal("新目标", None, false)
        .unwrap();
    assert!(matches!(
        monitor.poll_once().await.unwrap(),
        ExternalEventPoll::Ready(ExternalEventWake::GoalContinuation)
    ));
}

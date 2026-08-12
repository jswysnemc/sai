use super::*;

/// 验证新建子智能体可以按 ID 读取初始快照。
///
/// 返回:
/// - 无
#[test]
fn creates_and_reads_subagent_snapshot() {
    let (subagent, _cancel) =
        create_subagent_for_owner("default", "demo".to_string(), "explore".to_string(), 3);
    let loaded = subagent_snapshot(&subagent.id).unwrap();

    assert_eq!(loaded.description, "demo");
    assert_eq!(loaded.status, "running");
    assert_eq!(loaded.max_steps, 3);
    assert_eq!(loaded.step, 0);
    assert_eq!(loaded.phase, None);
}

/// 验证运行中进度会写回子智能体快照。
///
/// 返回:
/// - 无
#[test]
fn progress_update_writes_back_to_running_snapshot() {
    let (subagent, _cancel) = create_subagent("progress".to_string(), "explore".to_string(), 5);
    update_subagent_progress(
        &subagent.id,
        SubagentProgressUpdate {
            step: Some(2),
            phase: Some("工具 #2：Search 运行中".to_string()),
            last_tool: Some("Search".to_string()),
        },
    );
    let loaded = subagent_snapshot(&subagent.id).unwrap();

    assert_eq!(loaded.step, 2);
    assert_eq!(loaded.phase.as_deref(), Some("工具 #2：Search 运行中"));
    assert_eq!(loaded.last_tool.as_deref(), Some("Search"));
}

/// 验证终态子智能体不会再接受进度更新。
///
/// 返回:
/// - 无
#[test]
fn progress_update_ignored_after_finish() {
    let (subagent, _cancel) = create_subagent("done".to_string(), "general".to_string(), 4);
    finish_subagent(
        &subagent.id,
        "completed",
        Some("ok".to_string()),
        None,
        None,
    );
    update_subagent_progress(
        &subagent.id,
        SubagentProgressUpdate {
            step: Some(9),
            phase: Some("不应写入".to_string()),
            last_tool: None,
        },
    );
    let loaded = subagent_snapshot(&subagent.id).unwrap();

    assert_eq!(loaded.status, "completed");
    assert_eq!(loaded.step, 0);
    assert_eq!(loaded.phase, None);
}

/// 验证取消操作会把运行中子智能体标记为已取消。
///
/// 返回:
/// - 无
#[test]
fn cancel_marks_running_subagent_cancelled() {
    let (subagent, _cancel) = create_subagent("cancel".to_string(), "general".to_string(), 5);
    let cancelled = cancel_subagent(&subagent.id).unwrap();

    assert_eq!(cancelled.status, "cancelled");
}

/// 验证完成通知在主智能体确认前不会因一次读取而丢失。
///
/// 返回:
/// - 无
#[test]
fn finished_notice_remains_available_until_acknowledged() {
    let (subagent, _cancel) = create_subagent("delivery".to_string(), "general".to_string(), 5);
    finish_subagent(
        &subagent.id,
        "completed",
        Some("result".to_string()),
        None,
        None,
    );

    let first = take_finished_notices();
    let second = take_finished_notices();

    assert!(first.iter().any(|notice| notice.id == subagent.id));
    assert!(second.iter().any(|notice| notice.id == subagent.id));
}

/// 构造持久子智能体测试记录。
fn create_persistent(owner: &str, description: &str) -> (SubagentSnapshot, oneshot::Receiver<()>) {
    create_subagent_for_owner_goal(
        owner,
        None,
        description.to_string(),
        "general".to_string(),
        0,
        true,
    )
}

/// 【持久生命周期】验证 park 进入待命并重置完成通知，resume 唤醒回运行态。
#[test]
fn persistent_subagent_parks_and_resumes() {
    let owner = "persistent-park-owner";
    let (subagent, _cancel) = create_persistent(owner, "park target");

    assert!(park_subagent(
        &subagent.id,
        Some("segment one".to_string()),
        None
    ));
    let idle = subagent_snapshot(&subagent.id).unwrap();
    assert_eq!(idle.status, "idle");
    assert!(idle.persistent);
    assert_eq!(idle.turns_completed, 1);
    assert_eq!(idle.result.as_deref(), Some("segment one"));
    // 每个任务段完成都会重新投递完成通知
    let notices = pending_finished_notices(owner);
    assert!(notices
        .iter()
        .any(|notice| notice.id == subagent.id && notice.status == "idle"));

    // park 只对运行中的记录生效;resume 只对待命中的记录生效
    assert!(!park_subagent(&subagent.id, None, None));
    assert!(resume_subagent(&subagent.id));
    assert_eq!(subagent_snapshot(&subagent.id).unwrap().status, "running");
    assert!(!resume_subagent(&subagent.id));
}

/// 【消息队列】验证留言入队、计数与按序取出归档。
#[test]
fn queue_and_drain_inbox_in_order() {
    let owner = "inbox-order-owner";
    let (subagent, _cancel) = create_persistent(owner, "inbox target");

    let snapshot =
        queue_subagent_message_for_owner(owner, &subagent.id, "parent", "先做 A").unwrap();
    assert_eq!(snapshot.pending_messages, 1);
    let snapshot = queue_subagent_message(&subagent.id, "user", "再做 B").unwrap();
    assert_eq!(snapshot.pending_messages, 2);
    assert_eq!(subagent_inbox_len(&subagent.id), 2);

    let drained = drain_subagent_inbox(&subagent.id);
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].from, "parent");
    assert_eq!(drained[0].text, "先做 A");
    assert_eq!(drained[1].from, "user");
    assert_eq!(drained[1].text, "再做 B");
    assert_eq!(subagent_inbox_len(&subagent.id), 0);
    assert_eq!(subagent_snapshot(&subagent.id).unwrap().pending_messages, 0);
    // 已注入消息归档进历史,详情查询可回放
    let history = subagent_messages(&subagent.id);
    assert_eq!(history.len(), 2);
    // 队列为空时重复取出不产生状态变化
    assert!(drain_subagent_inbox(&subagent.id).is_empty());
}

/// 【消息队列】验证终态子智能体拒绝留言，空消息被拒绝。
#[test]
fn queue_rejects_finished_subagent_and_empty_message() {
    let owner = "inbox-reject-owner";
    let (subagent, _cancel) = create_persistent(owner, "reject target");
    assert!(queue_subagent_message(&subagent.id, "user", "   ").is_err());
    finish_subagent(&subagent.id, "completed", Some("done".to_string()), None, None);

    assert!(queue_subagent_message(&subagent.id, "user", "hello").is_err());
    assert!(queue_subagent_message("missing-subagent", "user", "hello").is_err());
}

/// 【优雅结束】验证 stop 请求只接受持久子智能体并可被 worker 查询。
#[test]
fn stop_request_requires_persistent_subagent() {
    let owner = "stop-request-owner";
    let (one_shot, _one_shot_cancel) = create_subagent_for_owner(
        owner,
        "one shot".to_string(),
        "general".to_string(),
        5,
    );
    assert!(request_subagent_stop_for_owner(owner, &one_shot.id, true).is_err());

    let (persistent, _cancel) = create_persistent(owner, "stop target");
    assert!(subagent_stop_requested(&persistent.id).is_none());
    let snapshot = request_subagent_stop_for_owner(owner, &persistent.id, false).unwrap();
    assert_eq!(snapshot.id, persistent.id);
    let stop = subagent_stop_requested(&persistent.id).unwrap();
    assert!(!stop.apply);
    // 其他会话不能操作该子智能体
    assert!(request_subagent_stop_for_owner("other-owner", &persistent.id, true).is_err());
}

/// 【取消】验证待命中的持久子智能体可以被取消。
#[test]
fn cancel_idle_persistent_subagent() {
    let owner = "cancel-idle-owner";
    let (subagent, mut cancel_rx) = create_persistent(owner, "cancel idle target");
    assert!(park_subagent(&subagent.id, Some("segment".to_string()), None));

    let cancelled = cancel_subagent(&subagent.id).unwrap();

    assert_eq!(cancelled.status, "cancelled");
    assert!(cancel_rx.try_recv().is_ok());
}

/// 【状态机】验证终态记录不会被后续 finish 覆盖，idle 转终态重新触发通知。
#[test]
fn finish_guards_terminal_state_and_renotifies_from_idle() {
    let owner = "finish-guard-owner";
    let (subagent, _cancel) = create_persistent(owner, "finish guard target");
    assert!(park_subagent(&subagent.id, Some("segment".to_string()), None));
    // 主 Agent 已消费 idle 段完成通知
    acknowledge_finished_notices(owner, std::slice::from_ref(&subagent.id));
    assert!(pending_finished_notices(owner)
        .iter()
        .all(|notice| notice.id != subagent.id));

    finish_subagent(&subagent.id, "completed", Some("final".to_string()), None, None);
    let finished = subagent_snapshot(&subagent.id).unwrap();
    assert_eq!(finished.status, "completed");
    // idle → completed 是新事件,通知重新投递
    assert!(pending_finished_notices(owner)
        .iter()
        .any(|notice| notice.id == subagent.id && notice.status == "completed"));

    // 已是终态,后续 finish 不再覆盖
    finish_subagent(&subagent.id, "failed", None, Some("late".to_string()), None);
    assert_eq!(subagent_snapshot(&subagent.id).unwrap().status, "completed");
}

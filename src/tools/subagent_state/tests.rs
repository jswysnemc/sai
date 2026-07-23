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

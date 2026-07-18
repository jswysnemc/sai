use super::subagent_state::{
    list_subagents_for_owner, pending_finished_notices, FinishedSubagentNotice, SubagentSnapshot,
};

/// 读取指定父会话和持续目标尚未确认的完成通知。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `goal_id`: 持续目标标识
///
/// 返回:
/// - 仅属于指定目标的完成通知
pub(crate) fn pending_finished_notices_for_goal(
    owner_key: &str,
    goal_id: &str,
) -> Vec<FinishedSubagentNotice> {
    pending_finished_notices(owner_key)
        .into_iter()
        .filter(|notice| notice.goal_id.as_deref() == Some(goal_id))
        .collect()
}

/// 列出指定父会话和持续目标的子智能体快照。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `goal_id`: 持续目标标识
///
/// 返回:
/// - 仅属于指定目标的子智能体快照
pub(crate) fn list_subagents_for_goal(owner_key: &str, goal_id: &str) -> Vec<SubagentSnapshot> {
    list_subagents_for_owner(owner_key)
        .into_iter()
        .filter(|snapshot| snapshot.goal_id.as_deref() == Some(goal_id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::subagent_state::{create_subagent_for_owner_goal, finish_subagent};

    /// 验证 Goal 只会收到绑定到自身的子智能体完成通知。
    #[test]
    fn finished_notices_are_filtered_by_goal() {
        let temp = tempfile::tempdir().unwrap();
        let owner = temp.path().display().to_string();
        let (goal_one, _cancel) = create_subagent_for_owner_goal(
            &owner,
            Some("goal-1".to_string()),
            "one".to_string(),
            "general".to_string(),
            2,
        );
        let (goal_two, _cancel) = create_subagent_for_owner_goal(
            &owner,
            Some("goal-2".to_string()),
            "two".to_string(),
            "general".to_string(),
            2,
        );
        finish_subagent(
            &goal_one.id,
            "completed",
            Some("one".to_string()),
            None,
            None,
        );
        finish_subagent(
            &goal_two.id,
            "completed",
            Some("two".to_string()),
            None,
            None,
        );

        let notices = pending_finished_notices_for_goal(&owner, "goal-1");
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].id, goal_one.id);
    }
}

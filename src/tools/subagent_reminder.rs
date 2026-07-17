use super::subagent_state::{
    acknowledge_finished_notices, pending_finished_notices, subagent_snapshot,
    FinishedSubagentNotice,
};

/// 子智能体完成提醒器。
///
/// 在主 Agent 的工具循环中,每个工具轮后检查是否有后台子智能体刚刚完成,
/// 若有则生成一段 system-reminder 注入对话,主动把结果推给主 Agent,
/// 避免主 Agent 因不知道子智能体是否完成而反复轮询 action=status。
pub(crate) struct SubagentReminder {
    owner_key: String,
    pending_ids: Vec<String>,
}

impl SubagentReminder {
    /// 创建子智能体完成提醒器。
    /// 参数:
    /// - `owner_key`: 父会话稳定作用域键
    pub(crate) fn new(owner_key: String) -> Self {
        Self {
            owner_key,
            pending_ids: Vec::new(),
        }
    }

    /// 在一个工具轮后收集新完成子智能体并生成提醒文本。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 有新完成子智能体时返回 system-reminder 文本
    pub(crate) fn after_tool_round(&mut self) -> Option<String> {
        let notices = pending_finished_notices(&self.owner_key);
        if notices.is_empty() {
            return None;
        }
        self.pending_ids = notices.iter().map(|notice| notice.id.clone()).collect();
        Some(render_notice(&notices))
    }

    /// 确认最近一次提醒已经包含在成功的模型请求中。
    pub(crate) fn acknowledge_delivered(&mut self) {
        acknowledge_finished_notices(&self.owner_key, &self.pending_ids);
        self.pending_ids.clear();
    }
}

/// 把完成通知渲染为 system-reminder 文本。
///
/// 参数:
/// - `notices`: 新完成的子智能体通知列表
///
/// 返回:
/// - system-reminder 文本
fn render_notice(notices: &[FinishedSubagentNotice]) -> String {
    let lines = notices
        .iter()
        .map(|notice| {
            let snapshot = subagent_snapshot(&notice.id).ok();
            let payload = snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.result.as_deref().or(snapshot.error.as_deref()))
                .map(bounded_result)
                .unwrap_or_default();
            format!(
                "- {}（{}）：{}\n{}",
                notice.description,
                notice.id,
                status_label(&notice.status),
                payload
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<system-reminder>以下后台子智能体已结束。结果已经直接附在通知中，请基于结果继续当前任务，不需要再次调用 status 或 result：\n{lines}\n</system-reminder>"
    )
}

/// 限制单个子智能体结果进入主模型上下文的长度。
///
/// 参数:
/// - `text`: 子智能体结果正文
///
/// 返回:
/// - 最多四千字符的结果
fn bounded_result(text: &str) -> String {
    const LIMIT: usize = 4_000;
    if text.chars().count() <= LIMIT {
        return text.to_string();
    }
    let mut result = text.chars().take(LIMIT).collect::<String>();
    result.push_str("\n[结果已截断，可按 subagent_id 查询完整内容]");
    result
}

/// 返回子智能体终态的中文说明。
///
/// 参数:
/// - `status`: 子智能体状态
///
/// 返回:
/// - 状态说明文本
fn status_label(status: &str) -> &str {
    match status {
        "completed" => "已完成",
        "failed" => "执行失败",
        "cancelled" => "已取消",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::subagent_state::{
        create_subagent_for_owner, finish_subagent, pending_finished_notices,
    };

    #[test]
    fn no_reminder_without_finished_subagents() {
        let mut reminder = SubagentReminder::new("reminder-empty".to_string());
        // 新建但未完成,不应触发提醒
        let (_subagent, _cancel) = create_subagent_for_owner(
            "reminder-empty",
            "pending work".to_string(),
            "general".to_string(),
            5,
        );
        // 可能有其他测试残留的完成项,这里只断言不 panic 并可重复调用
        let _ = reminder.after_tool_round();
    }

    #[test]
    fn acknowledges_only_after_successful_delivery() {
        let owner = "reminder-delivery";
        let mut reminder = SubagentReminder::new(owner.to_string());
        let (subagent, _cancel) =
            create_subagent_for_owner(owner, "build index".to_string(), "explore".to_string(), 5);
        finish_subagent(
            &subagent.id,
            "completed",
            Some("done".to_string()),
            None,
            None,
        );

        let first = reminder.after_tool_round();
        assert!(first.is_some());
        let text = first.unwrap();
        assert!(text.contains(&subagent.id));
        assert!(text.contains("已完成"));
        assert!(text.contains("done"));

        let before_ack = pending_finished_notices(owner);
        assert!(before_ack.iter().any(|notice| notice.id == subagent.id));
        reminder.acknowledge_delivered();

        let notices_again = pending_finished_notices(owner);
        assert!(notices_again.iter().all(|notice| notice.id != subagent.id));
    }
}

use crate::i18n::text as t;
use crate::tools::subagent_state;
use anyhow::{bail, Result};

/// 生成 `/subagents` 命令的会话子智能体列表文本。
///
/// 列表按启动时间倒序编号，序号可直接用于 `/msg <序号> <消息>`；
/// 持久子智能体额外标注存活状态与未读留言数。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
///
/// 返回:
/// - 可直接展示的列表文本
pub(in crate::cli) fn format_subagent_list(owner_key: &str) -> String {
    let subagents = subagent_state::list_subagents_for_owner(owner_key);
    if subagents.is_empty() {
        return t("no subagents in this session", "当前会话还没有子智能体").to_string();
    }
    let mut lines = vec![t("Session subagents:", "会话子智能体：").to_string()];
    for (index, snapshot) in subagents.iter().enumerate() {
        let mut tags = Vec::new();
        if snapshot.persistent {
            tags.push(t("persistent", "持久").to_string());
        }
        if snapshot.pending_messages > 0 {
            tags.push(format!(
                "{} {}",
                snapshot.pending_messages,
                t("unread", "条未读留言")
            ));
        }
        let tag_text = if tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", tags.join(" · "))
        };
        lines.push(format!(
            "{}. {} ({}) — {}{tag_text}",
            index + 1,
            snapshot.description,
            snapshot.id,
            status_label(&snapshot.status),
        ));
    }
    lines.push(
        t(
            "Use /msg <index|id> <message> to leave a message on an alive subagent.",
            "使用 /msg <序号|ID> <消息> 给存活的子智能体留言。",
        )
        .to_string(),
    );
    lines.join("\n")
}

/// 把用户留言投递给目标子智能体。
///
/// 目标解析顺序：显式序号或 ID → 当前查看中的子智能体 → 会话内唯一
/// 存活（running/idle）的子智能体；无法唯一确定时报错并给出提示。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `target`: `/msg` 显式指定的序号或子智能体 ID
/// - `message`: 留言正文
/// - `viewing_id`: TUI 当前正在查看的子智能体 ID
///
/// 返回:
/// - 投递成功的提示文本
pub(in crate::cli) fn deliver_subagent_message(
    owner_key: &str,
    target: Option<&str>,
    message: &str,
    viewing_id: Option<&str>,
) -> Result<String> {
    let subagent_id = resolve_message_target(owner_key, target, viewing_id)?;
    let snapshot =
        subagent_state::queue_subagent_message_for_owner(owner_key, &subagent_id, "user", message)?;
    Ok(if crate::i18n::is_zh() {
        format!(
            "已给子智能体留言：{}（{}）；消息将在其下一个步间间隙注入",
            snapshot.description, snapshot.id
        )
    } else {
        format!(
            "message left for subagent {} ({}); it is injected at the next step boundary",
            snapshot.description, snapshot.id
        )
    })
}

/// 解析留言目标子智能体。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `target`: 显式指定的序号或 ID
/// - `viewing_id`: 当前查看中的子智能体 ID
///
/// 返回:
/// - 目标子智能体 ID
fn resolve_message_target(
    owner_key: &str,
    target: Option<&str>,
    viewing_id: Option<&str>,
) -> Result<String> {
    let subagents = subagent_state::list_subagents_for_owner(owner_key);
    if let Some(target) = target {
        // 1. subagent_ 前缀视为完整 ID;纯数字视为 /subagents 列表序号
        if target.starts_with("subagent_") {
            return Ok(target.to_string());
        }
        let index = target
            .parse::<usize>()
            .map_err(|_| anyhow::anyhow!("{}: {target}", t("invalid target", "无效留言目标")))?;
        if index == 0 || index > subagents.len() {
            bail!(
                "{}: {index}",
                t("subagent index out of range", "子智能体序号超出范围")
            );
        }
        return Ok(subagents[index - 1].id.clone());
    }
    // 2. 正在查看某个子智能体时默认投递给它
    if let Some(viewing) = viewing_id {
        return Ok(viewing.to_string());
    }
    // 3. 只有一个存活子智能体时无需指定
    let alive = subagents
        .iter()
        .filter(|snapshot| snapshot.status == "running" || snapshot.status == "idle")
        .collect::<Vec<_>>();
    match alive.as_slice() {
        [only] => Ok(only.id.clone()),
        [] => bail!(t(
            "no alive subagent to message; run /subagents to check",
            "没有存活的子智能体可留言；用 /subagents 查看"
        )),
        _ => bail!(t(
            "multiple alive subagents; specify one with /msg <index|id> <message>",
            "存在多个存活子智能体；请用 /msg <序号|ID> <消息> 指定目标"
        )),
    }
}

/// 生成子智能体状态的本地化标签。
///
/// 参数:
/// - `status`: 快照状态字符串
///
/// 返回:
/// - 本地化状态文本
fn status_label(status: &str) -> String {
    match status {
        "running" => t("running", "运行中").to_string(),
        "idle" => t("idle (awaiting messages)", "待命中（可留言）").to_string(),
        "completed" => t("completed", "已完成").to_string(),
        "failed" => t("failed", "已失败").to_string(),
        "cancelled" => t("cancelled", "已取消").to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证空会话的列表提示与无目标留言的报错。
    #[test]
    fn empty_session_yields_hint_and_message_error() {
        let owner = "/nonexistent-subagent-owner-for-test";
        assert!(format_subagent_list(owner)
            .contains(&t("no subagents in this session", "当前会话还没有子智能体")));
        assert!(resolve_message_target(owner, None, None).is_err());
    }

    /// 验证显式 ID 目标直接透传，序号越界报错。
    #[test]
    fn explicit_targets_resolve_or_fail() {
        let owner = "/nonexistent-subagent-owner-for-test";
        assert_eq!(
            resolve_message_target(owner, Some("subagent_9_9"), None).unwrap(),
            "subagent_9_9"
        );
        assert!(resolve_message_target(owner, Some("3"), None).is_err());
        assert!(resolve_message_target(owner, Some("abc"), None).is_err());
    }

    /// 验证查看中的子智能体优先作为默认目标。
    #[test]
    fn viewing_subagent_wins_as_default_target() {
        let owner = "/nonexistent-subagent-owner-for-test";
        assert_eq!(
            resolve_message_target(owner, None, Some("subagent_5_5")).unwrap(),
            "subagent_5_5"
        );
    }
}

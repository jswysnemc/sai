use super::*;

/// 取出已完成但尚未通知主 Agent 的子智能体,并标记为已通知。
///
/// 供主 Agent 循环在工具轮后调用,把后台子智能体的完成事件推给主 Agent,
/// 避免主 Agent 反复轮询 action=status。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 本次新完成子智能体的通知列表
#[cfg(test)]
pub(crate) fn take_finished_notices() -> Vec<FinishedSubagentNotice> {
    pending_finished_notices("default")
}

/// 读取父会话尚未确认的完成通知，不修改投递状态。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
///
/// 返回:
/// - 待投递完成通知
pub(crate) fn pending_finished_notices(owner_key: &str) -> Vec<FinishedSubagentNotice> {
    ensure_owner_loaded(owner_key);
    let subagents = subagents().lock().expect("subagent state lock");
    let mut notices = Vec::new();
    for record in subagents.values() {
        // 1. 仅挑出当前父会话中已进入终态且未确认的记录
        if record.owner_key != owner_key
            || record.finish_notified
            || record.snapshot.status == "running"
        {
            continue;
        }
        notices.push(FinishedSubagentNotice {
            id: record.snapshot.id.clone(),
            goal_id: record.snapshot.goal_id.clone(),
            description: record.snapshot.description.clone(),
            status: record.snapshot.status.clone(),
        });
    }
    notices
}

/// 确认父会话已经把完成结果交给模型。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `ids`: 已成功投递的子智能体 ID
pub(crate) fn acknowledge_finished_notices(owner_key: &str, ids: &[String]) {
    ensure_owner_loaded(owner_key);
    let ids = ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    let mut subagents = subagents().lock().expect("subagent state lock");
    for record in subagents.values_mut() {
        if record.owner_key == owner_key
            && ids.contains(record.snapshot.id.as_str())
            && record.snapshot.status != "running"
        {
            record.finish_notified = true;
        }
    }
    persist_owner_locked(&subagents, owner_key);
}

/// 列出指定父会话创建的子智能体。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
///
/// 返回:
/// - 子智能体快照列表
pub(crate) fn list_subagents_for_owner(owner_key: &str) -> Vec<SubagentSnapshot> {
    ensure_owner_loaded(owner_key);
    let mut items = subagents()
        .lock()
        .expect("subagent state lock")
        .values()
        .filter(|record| record.owner_key == owner_key)
        .map(|record| record.snapshot.clone())
        .collect::<Vec<_>>();
    items.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    items
}

/// 读取后台子智能体快照。
///
/// 参数:
/// - `id`: 任务 ID
///
/// 返回:
/// - 子智能体快照
pub(crate) fn subagent_snapshot(id: &str) -> Result<SubagentSnapshot> {
    subagents()
        .lock()
        .expect("subagent state lock")
        .get(id)
        .map(|record| record.snapshot.clone())
        .ok_or_else(|| anyhow::anyhow!("subagent not found: {id}"))
}

/// 读取指定父会话中的子智能体快照。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `id`: 子智能体 ID
///
/// 返回:
/// - 子智能体快照
pub(crate) fn subagent_snapshot_for_owner(owner_key: &str, id: &str) -> Result<SubagentSnapshot> {
    ensure_owner_loaded(owner_key);
    subagents()
        .lock()
        .expect("subagent state lock")
        .get(id)
        .filter(|record| record.owner_key == owner_key)
        .map(|record| record.snapshot.clone())
        .ok_or_else(|| anyhow::anyhow!("subagent not found in current session: {id}"))
}

/// 列出后台子智能体快照。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 子智能体快照列表
pub(crate) fn list_subagents() -> Vec<SubagentSnapshot> {
    let mut subagents = subagents()
        .lock()
        .expect("subagent state lock")
        .values()
        .map(|record| record.snapshot.clone())
        .collect::<Vec<_>>();
    subagents.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    subagents
}

/// 取消后台子智能体。
///
/// 参数:
/// - `id`: 任务 ID
///
/// 返回:
/// - 取消后的子智能体快照
pub(crate) fn cancel_subagent(id: &str) -> Result<SubagentSnapshot> {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let record = subagents
        .get_mut(id)
        .ok_or_else(|| anyhow::anyhow!("subagent not found: {id}"))?;
    if record.snapshot.status != "running" {
        return Ok(record.snapshot.clone());
    }
    if let Some(cancel) = record.cancel.take() {
        let _ = cancel.send(());
    } else {
        bail!("subagent is not cancellable: {id}");
    }
    record.snapshot.status = "cancelled".to_string();
    record.snapshot.updated_at = unix_seconds();
    record.snapshot.error = Some("cancel requested".to_string());
    publish_record(record);
    let result = record.snapshot.clone();
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
    Ok(result)
}

/// 取消指定父会话中的子智能体。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `id`: 子智能体 ID
///
/// 返回:
/// - 取消后的子智能体快照
pub(crate) fn cancel_subagent_for_owner(owner_key: &str, id: &str) -> Result<SubagentSnapshot> {
    ensure_owner_loaded(owner_key);
    let belongs_to_owner = subagents()
        .lock()
        .expect("subagent state lock")
        .get(id)
        .is_some_and(|record| record.owner_key == owner_key);
    if !belongs_to_owner {
        bail!("subagent not found in current session: {id}");
    }
    cancel_subagent(id)
}

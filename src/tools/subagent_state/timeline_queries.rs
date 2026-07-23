use super::*;

/// 读取子智能体的执行时间线快照。
///
/// 参数:
/// - `id`: 任务 ID
///
/// 返回:
/// - 时间线条目列表
pub(crate) fn subagent_timeline(id: &str) -> Result<Vec<SubagentTimelineEntry>> {
    subagents()
        .lock()
        .expect("subagent state lock")
        .get(id)
        .map(|record| record.timeline.entries())
        .ok_or_else(|| anyhow::anyhow!("subagent not found: {id}"))
}

/// 读取并订阅子智能体详情流。
///
/// 参数:
/// - `id`: 子智能体 ID
/// - `after`: 已接收的最后事件序号
///
/// 返回:
/// - 历史事件和实时广播接收器
pub(crate) fn subagent_event_stream(
    id: &str,
    after: u64,
) -> Result<(
    Vec<SubagentStreamEvent>,
    tokio::sync::broadcast::Receiver<SubagentStreamEvent>,
)> {
    let subagents = subagents().lock().expect("subagent state lock");
    let record = subagents
        .get(id)
        .ok_or_else(|| anyhow::anyhow!("subagent not found: {id}"))?;
    Ok((
        record.event_journal.events_after(after),
        record.event_journal.subscribe(),
    ))
}

use super::readable_tool_name;
use super::subagent_event::{SubagentEventJournal, SubagentStreamEvent};
use super::subagent_persistence::{self, PersistedSubagent};
use super::subagent_timeline::{SubagentTimeline, SubagentTimelineEntry};
use crate::i18n::is_zh;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use tokio::sync::oneshot;

static SUBAGENTS: OnceLock<Mutex<HashMap<String, SubagentRecord>>> = OnceLock::new();
static LOADED_OWNERS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct SubagentSnapshot {
    pub(crate) id: String,
    pub(crate) description: String,
    pub(crate) subagent_type: String,
    pub(crate) status: String,
    pub(crate) max_steps: usize,
    pub(crate) started_at: u64,
    pub(crate) updated_at: u64,
    pub(crate) step: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stats: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) worktree_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) worktree_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) parent_workdir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) worktree_merge: Option<Value>,
}

/// 子智能体运行过程中的一次进度更新。
#[derive(Debug, Clone, Default)]
pub(crate) struct SubagentProgressUpdate {
    pub(crate) step: Option<usize>,
    pub(crate) phase: Option<String>,
    pub(crate) last_tool: Option<String>,
}

struct SubagentRecord {
    owner_key: String,
    snapshot: SubagentSnapshot,
    cancel: Option<oneshot::Sender<()>>,
    /// 完成事件是否已通知主 Agent,避免重复提醒
    finish_notified: bool,
    /// 执行时间线,供详情页实时流式渲染
    timeline: SubagentTimeline,
    event_journal: SubagentEventJournal,
}

/// 已完成但尚未通知主 Agent 的子智能体摘要。
#[derive(Debug, Clone)]
pub(crate) struct FinishedSubagentNotice {
    pub(crate) id: String,
    pub(crate) description: String,
    pub(crate) status: String,
}

/// 创建后台子智能体记录。
///
/// 参数:
/// - `description`: 任务描述
/// - `subagent_type`: 子代理类型
/// - `max_steps`: 最大工具调用次数
///
/// 返回:
/// - 子智能体快照和取消接收器
#[cfg(test)]
pub(crate) fn create_subagent(
    description: String,
    subagent_type: String,
    max_steps: usize,
) -> (SubagentSnapshot, oneshot::Receiver<()>) {
    create_subagent_for_owner("default", description, subagent_type, max_steps)
}

/// 创建绑定到父会话的后台子智能体记录。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `description`: 任务描述
/// - `subagent_type`: 子代理类型
/// - `max_steps`: 最大工具调用次数
///
/// 返回:
/// - 子智能体快照和取消接收器
pub(crate) fn create_subagent_for_owner(
    owner_key: &str,
    description: String,
    subagent_type: String,
    max_steps: usize,
) -> (SubagentSnapshot, oneshot::Receiver<()>) {
    ensure_owner_loaded(owner_key);
    let now = unix_seconds();
    let id = format!("subagent_{now}_{}", rand::random::<u16>());
    let (cancel_tx, cancel_rx) = oneshot::channel();
    let snapshot = SubagentSnapshot {
        id: id.clone(),
        description,
        subagent_type,
        status: "running".to_string(),
        max_steps,
        started_at: now,
        updated_at: now,
        step: 0,
        phase: None,
        last_tool: None,
        result: None,
        error: None,
        stats: None,
        worktree_root: None,
        worktree_branch: None,
        parent_workdir: None,
        worktree_merge: None,
    };
    let mut record = SubagentRecord {
        owner_key: owner_key.to_string(),
        snapshot: snapshot.clone(),
        cancel: Some(cancel_tx),
        finish_notified: false,
        timeline: SubagentTimeline::default(),
        event_journal: SubagentEventJournal::new(),
    };
    publish_record(&mut record);
    let mut subagents = subagents().lock().expect("subagent state lock");
    subagents.insert(id, record);
    persist_owner_locked(&subagents, owner_key);
    (snapshot, cancel_rx)
}


/// Attach worktree isolation metadata to a running subagent.
pub(crate) fn set_subagent_worktree(
    id: &str,
    worktree_root: Option<String>,
    worktree_branch: Option<String>,
    parent_workdir: Option<String>,
) {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = subagents.get_mut(id) else {
        return;
    };
    record.snapshot.worktree_root = worktree_root;
    record.snapshot.worktree_branch = worktree_branch;
    record.snapshot.parent_workdir = parent_workdir;
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

/// Attach worktree merge result metadata to a finished or finishing subagent.
pub(crate) fn set_subagent_worktree_merge(id: &str, merge: Value) {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = subagents.get_mut(id) else {
        return;
    };
    record.snapshot.worktree_merge = Some(merge);
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

/// 完成后台子智能体记录。
///
/// 参数:
/// - `id`: 任务 ID
/// - `status`: 完成状态
/// - `result`: 子代理结果
/// - `error`: 错误信息
/// - `stats`: 统计信息
///
/// 返回:
/// - 无
pub(crate) fn finish_subagent(
    id: &str,
    status: &str,
    result: Option<String>,
    error: Option<String>,
    stats: Option<Value>,
) {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = subagents.get_mut(id) else {
        return;
    };
    record.snapshot.status = status.to_string();
    record.snapshot.updated_at = unix_seconds();
    record.snapshot.result = result;
    record.snapshot.error = error;
    record.snapshot.stats = stats;
    record.cancel = None;
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

/// 更新运行中子智能体的中间进度。
///
/// 参数:
/// - `id`: 任务 ID
/// - `update`: 本次进度更新(步数、阶段、最近工具)
///
/// 返回:
/// - 无
pub(crate) fn update_subagent_progress(id: &str, update: SubagentProgressUpdate) {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = subagents.get_mut(id) else {
        return;
    };
    // 1. 只更新运行中的子智能体，避免覆盖已写入的终态
    if record.snapshot.status != "running" {
        return;
    }
    if let Some(step) = update.step {
        record.snapshot.step = step;
    }
    if let Some(phase) = update.phase {
        record.snapshot.phase = Some(phase);
    }
    if let Some(last_tool) = update.last_tool {
        record.snapshot.last_tool = Some(last_tool);
    }
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

/// 记录子智能体的一次子工具调用开始,并同步快照进度。
///
/// 参数:
/// - `id`: 任务 ID
/// - `name`: 子工具名称
/// - `args`: 子工具参数 JSON 文本
///
/// 返回:
/// - 无
pub(crate) fn timeline_tool_started(id: &str, name: &str, args: &str) {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = running_record(&mut subagents, id) else {
        return;
    };
    let step = record.timeline.push_tool(name, args);
    record.snapshot.step = step;
    record.snapshot.last_tool = Some(name.to_string());
    record.snapshot.phase = Some(if is_zh() {
        format!("工具 #{step}：{} 运行中", readable_tool_name(name))
    } else {
        format!("tool #{step}: {name} running")
    });
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

/// 回填子智能体最近一次子工具调用的结果,并同步快照进度。
///
/// 参数:
/// - `id`: 任务 ID
/// - `name`: 子工具名称
/// - `ok`: 是否成功
/// - `output`: 子工具输出
///
/// 返回:
/// - 无
pub(crate) fn timeline_tool_finished(id: &str, name: &str, ok: bool, output: &str) {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = running_record(&mut subagents, id) else {
        return;
    };
    let Some(step) = record.timeline.complete_tool(name, ok, output) else {
        return;
    };
    let state_text = if ok {
        "ok"
    } else if is_zh() {
        "失败"
    } else {
        "failed"
    };
    record.snapshot.phase = Some(if is_zh() {
        format!("工具 #{step}：{} {state_text}", readable_tool_name(name))
    } else {
        format!("tool #{step}: {name} {state_text}")
    });
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

/// 追加子智能体的正文或推理片段到时间线。
///
/// 参数:
/// - `id`: 任务 ID
/// - `text`: 文本片段
/// - `reasoning`: 是否为推理片段
///
/// 返回:
/// - 无
pub(crate) fn timeline_streaming_text(id: &str, text: &str, reasoning: bool) {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = running_record(&mut subagents, id) else {
        return;
    };
    if reasoning {
        record.timeline.append_reasoning(text);
    } else {
        record.timeline.append_text(text);
    }
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

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

/// 取出仍在运行中的记录,终态记录返回空。
fn running_record<'map>(
    subagents: &'map mut HashMap<String, SubagentRecord>,
    id: &str,
) -> Option<&'map mut SubagentRecord> {
    subagents
        .get_mut(id)
        .filter(|record| record.snapshot.status == "running")
}

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

/// 将记录当前状态写入详情事件流。
///
/// 参数:
/// - `record`: 子智能体运行记录
fn publish_record(record: &mut SubagentRecord) {
    record
        .event_journal
        .publish(record.snapshot.clone(), record.timeline.entries());
}

/// 首次访问父会话时恢复其持久化子智能体记录。
fn ensure_owner_loaded(owner_key: &str) {
    let mut loaded = loaded_owners().lock().expect("subagent owner lock");
    if !loaded.insert(owner_key.to_string()) {
        return;
    }
    let Ok(records) = subagent_persistence::load(owner_key) else {
        return;
    };
    let mut subagents = subagents().lock().expect("subagent state lock");
    for persisted in records {
        if subagents.contains_key(&persisted.snapshot.id) {
            continue;
        }
        let mut snapshot = persisted.snapshot;
        if snapshot.status == "running" {
            snapshot.status = "failed".to_string();
            snapshot.error = Some("子智能体进程在完成前中断".to_string());
            snapshot.updated_at = unix_seconds();
        }
        let mut record = SubagentRecord {
            owner_key: persisted.owner_key,
            snapshot,
            cancel: None,
            finish_notified: persisted.finish_notified,
            timeline: SubagentTimeline::from_entries(persisted.timeline),
            event_journal: SubagentEventJournal::new(),
        };
        publish_record(&mut record);
        subagents.insert(record.snapshot.id.clone(), record);
    }
    persist_owner_locked(&subagents, owner_key);
}

/// 保存指定父会话当前全部子智能体记录。
fn persist_owner_locked(subagents: &HashMap<String, SubagentRecord>, owner_key: &str) {
    let records = subagents
        .values()
        .filter(|record| record.owner_key == owner_key)
        .map(|record| PersistedSubagent {
            owner_key: record.owner_key.clone(),
            snapshot: record.snapshot.clone(),
            timeline: record.timeline.entries(),
            finish_notified: record.finish_notified,
        })
        .collect::<Vec<_>>();
    let _ = subagent_persistence::save(owner_key, &records);
}

/// 获取当前 Unix 秒数。
///
/// 参数:
/// - 无
///
/// 返回:
/// - Unix 秒数
fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

/// 获取后台子智能体表。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 全局子智能体表
fn subagents() -> &'static Mutex<HashMap<String, SubagentRecord>> {
    SUBAGENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 获取已完成持久化恢复的父会话集合。
fn loaded_owners() -> &'static Mutex<HashSet<String>> {
    LOADED_OWNERS.get_or_init(|| Mutex::new(HashSet::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_reads_subagent_snapshot() {
        let (subagent, _cancel) = create_subagent("demo".to_string(), "explore".to_string(), 3);
        let loaded = subagent_snapshot(&subagent.id).unwrap();

        assert_eq!(loaded.description, "demo");
        assert_eq!(loaded.status, "running");
        assert_eq!(loaded.max_steps, 3);
        assert_eq!(loaded.step, 0);
        assert_eq!(loaded.phase, None);
    }

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

    #[test]
    fn cancel_marks_running_subagent_cancelled() {
        let (subagent, _cancel) = create_subagent("cancel".to_string(), "general".to_string(), 5);
        let cancelled = cancel_subagent(&subagent.id).unwrap();

        assert_eq!(cancelled.status, "cancelled");
    }

    /// 验证完成通知在主智能体确认前不会因一次读取而丢失。
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
}

use super::model::{GroupMember, GroupMessage, RunnerReport, SuperTask, TriggerEvent};
use super::store::SuperAgentStore;
use anyhow::{bail, Result};
use serde_json::{json, Value};

/// Inline 摘要默认字符预算（对应论文 3.3 节 Inline & Spill 上下文缓存策略）。
const INLINE_PREVIEW_CHARS: usize = 240;

/// 生成高密度的 Inline 截断摘要，超出预算时标记 spill 续读提示。
///
/// 参数:
/// - `text`: 原始正文
/// - `max_chars`: 内联字符上限
///
/// 返回:
/// - `(内联文本, 是否被截断)`
pub fn inline_or_spill(text: &str, max_chars: usize) -> (String, bool) {
    let limit = max_chars.max(32);
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        let total = text.chars().count();
        (
            format!("{head}\n...[spilled: total {total} chars; use super_recall to read full text]"),
            true,
        )
    } else {
        (head, false)
    }
}

/// 构造给 Leader 的 Trigger 紧凑卡片（携带下一跳引用 ID，支撑链式召回）。
///
/// 参数:
/// - `trigger`: 触发事件
///
/// 返回:
/// - 紧凑 JSON 卡片
pub fn trigger_card(trigger: &TriggerEvent) -> Value {
    let (preview, spilled) = inline_or_spill(&trigger.content, INLINE_PREVIEW_CHARS);
    json!({
        "trigger_id": trigger.id,
        "thread_id": trigger.thread_id,
        "source_channel": trigger.source_channel,
        "actor": trigger.actor,
        "disposition": trigger.disposition.as_str(),
        "route_path": trigger.route_path,
        "preview": preview,
        "spilled": spilled,
        "created_at": trigger.created_at,
    })
}

/// 构造 Runner 结构化报告的紧凑卡片。
///
/// 参数:
/// - `report`: Runner 报告
///
/// 返回:
/// - 紧凑 JSON 卡片
pub fn report_card(report: &RunnerReport) -> Value {
    let (details_preview, spilled) = inline_or_spill(&report.details, INLINE_PREVIEW_CHARS);
    json!({
        "report_id": report.id,
        "task_id": report.task_id,
        "thread_id": report.thread_id,
        "worker_id": report.worker_member_id,
        "worker_name": report.worker_name,
        "status": report.status.as_str(),
        "summary": report.summary,
        "details_preview": details_preview,
        "spilled": spilled,
        "artifacts": report.artifacts,
        "needs_human_question": report.needs_human_question,
        "synthesized_by_leader": report.synthesized_by_leader,
    })
}

/// 构造任务工单的紧凑卡片。
///
/// 参数:
/// - `task`: 任务记录
///
/// 返回:
/// - 紧凑 JSON 卡片
pub fn task_card(task: &SuperTask) -> Value {
    let (objective_preview, spilled) = inline_or_spill(&task.objective, INLINE_PREVIEW_CHARS);
    json!({
        "task_id": task.id,
        "thread_id": task.thread_id,
        "title": task.title,
        "objective_preview": objective_preview,
        "spilled": spilled,
        "status": task.status.as_str(),
        "assigned_member_id": task.assigned_member_id,
        "assigned_member_name": task.assigned_member_name,
        "origin_channel": task.origin_channel,
        "latest_report_status": task.latest_report_status.map(|status| status.as_str()),
        "latest_report_summary": task.latest_report_summary,
    })
}

/// 构造群组成员紧凑卡片。
///
/// 参数:
/// - `member`: 成员记录
///
/// 返回:
/// - 紧凑 JSON 卡片
pub fn member_card(member: &GroupMember) -> Value {
    json!({
        "member_id": member.id,
        "name": member.name,
        "role": member.role.as_str(),
        "status": member.status.as_str(),
        "specialty": member.specialty,
        "active_task_id": member.active_task_id,
        "workdir": member.workdir,
        "last_activity": member.last_activity,
    })
}

/// 构造群组消息紧凑卡片。
///
/// 参数:
/// - `message`: 群组消息
///
/// 返回:
/// - 紧凑 JSON 卡片
pub fn message_card(message: &GroupMessage) -> Value {
    let (preview, spilled) = inline_or_spill(&message.content, INLINE_PREVIEW_CHARS);
    json!({
        "message_id": message.id,
        "seq": message.seq,
        "thread_id": message.thread_id,
        "sender_kind": message.sender_kind.as_str(),
        "sender_name": message.sender_name,
        "recipient_name": message.recipient_name,
        "kind": message.kind.as_str(),
        "channel_source": message.channel_source,
        "preview": preview,
        "spilled": spilled,
        "related_task_id": message.related_task_id,
        "related_report_id": message.related_report_id,
    })
}

/// 【超级智能体】【链式召回】按对象类型与标识读取溢出正文及下一跳引用（论文 3.3 节 Chain of Recall）。
///
/// 参数:
/// - `store`: 持久化存储
/// - `group_id`: 当前群组边界
/// - `kind`: 对象类型（trigger / thread / task / report / message / member）
/// - `id`: 对象标识
/// - `offset`: 字符偏移量
/// - `limit`: 本次最多返回字符数
///
/// 返回:
/// - 带正文切片与下一跳关联 ID 的 JSON 对象
pub fn recall_object(
    store: &SuperAgentStore,
    group_id: &str,
    kind: &str,
    id: &str,
    offset: usize,
    limit: usize,
) -> Result<Value> {
    let max_chars = limit.clamp(200, 12_000);
    match kind.trim().to_ascii_lowercase().as_str() {
        "trigger" => {
            let Some(trigger) = store.get_trigger(id)? else {
                bail!("trigger not found: {id}");
            };
            if trigger.group_id != group_id {
                bail!("trigger {id} belongs to another group");
            }
            let (slice, next_offset, total_chars) = slice_chars(&trigger.content, offset, max_chars);
            Ok(json!({
                "kind": "trigger",
                "id": trigger.id,
                "thread_id": trigger.thread_id,
                "source_channel": trigger.source_channel,
                "actor": trigger.actor,
                "disposition": trigger.disposition.as_str(),
                "route_path": trigger.route_path,
                "content": slice,
                "offset": offset,
                "next_offset": next_offset,
                "total_chars": total_chars,
                "next_hops": {
                    "thread_id": trigger.thread_id,
                    "target_member_ids": trigger.target_member_ids,
                }
            }))
        }
        "thread" => {
            let Some(thread) = store.get_thread(id)? else {
                bail!("thread not found: {id}");
            };
            if thread.group_id != group_id {
                bail!("thread {id} belongs to another group");
            }
            let messages = store.list_thread_messages(id, 30)?;
            let subs = store.thread_subscriptions(id)?;
            Ok(json!({
                "kind": "thread",
                "id": thread.id,
                "title": thread.title,
                "channel": thread.channel,
                "external_ref": thread.external_ref,
                "subscriptions": subs,
                "messages": messages.iter().map(message_card).collect::<Vec<_>>(),
            }))
        }
        "task" => {
            let Some(task) = store.get_task(id)? else {
                bail!("task not found: {id}");
            };
            if task.group_id != group_id {
                bail!("task {id} belongs to another group");
            }
            let reports: Vec<Value> = store
                .list_reports(group_id, 50)?
                .into_iter()
                .filter(|rep| rep.task_id.as_deref() == Some(id))
                .map(|rep| report_card(&rep))
                .collect();
            let (objective, next_offset, total_chars) =
                slice_chars(&task.objective, offset, max_chars);
            Ok(json!({
                "kind": "task",
                "id": task.id,
                "title": task.title,
                "status": task.status.as_str(),
                "thread_id": task.thread_id,
                "assigned_member_id": task.assigned_member_id,
                "assigned_member_name": task.assigned_member_name,
                "origin_channel": task.origin_channel,
                "objective": objective,
                "deliverable": task.deliverable,
                "offset": offset,
                "next_offset": next_offset,
                "total_chars": total_chars,
                "reports": reports,
            }))
        }
        "report" => {
            let Some(report) = store.get_report(id)? else {
                bail!("report not found: {id}");
            };
            if report.group_id != group_id {
                bail!("report {id} belongs to another group");
            }
            let (details, next_offset, total_chars) = slice_chars(&report.details, offset, max_chars);
            Ok(json!({
                "kind": "report",
                "id": report.id,
                "task_id": report.task_id,
                "thread_id": report.thread_id,
                "worker_member_id": report.worker_member_id,
                "worker_name": report.worker_name,
                "status": report.status.as_str(),
                "summary": report.summary,
                "details": details,
                "artifacts": report.artifacts,
                "needs_human_question": report.needs_human_question,
                "offset": offset,
                "next_offset": next_offset,
                "total_chars": total_chars,
            }))
        }
        "message" => {
            let Some(message) = store.get_message(id)? else {
                bail!("message not found: {id}");
            };
            if message.group_id != group_id {
                bail!("message {id} belongs to another group");
            }
            let (content, next_offset, total_chars) = slice_chars(&message.content, offset, max_chars);
            Ok(json!({
                "kind": "message",
                "id": message.id,
                "thread_id": message.thread_id,
                "sender_name": message.sender_name,
                "sender_kind": message.sender_kind.as_str(),
                "content": content,
                "offset": offset,
                "next_offset": next_offset,
                "total_chars": total_chars,
                "related_task_id": message.related_task_id,
                "related_report_id": message.related_report_id,
            }))
        }
        "member" | "worker" | "runner" => {
            let Some(member) = store.get_member(id)? else {
                bail!("member not found: {id}");
            };
            if member.group_id != group_id {
                bail!("member {id} belongs to another group");
            }
            let reports: Vec<Value> = store
                .list_reports(group_id, 20)?
                .into_iter()
                .filter(|rep| rep.worker_member_id == id)
                .map(|rep| report_card(&rep))
                .collect();
            Ok(json!({
                "kind": "member",
                "member": member,
                "recent_reports": reports,
            }))
        }
        other => bail!("unsupported recall kind: {other}"),
    }
}

/// 按字符偏移切分长文本，供续读使用。
///
/// 参数:
/// - `text`: 原始正文
/// - `offset`: 起始字符下标
/// - `limit`: 返回最大字符数
///
/// 返回:
/// - `(切片文本, 下一页偏移, 总字符数)`
fn slice_chars(text: &str, offset: usize, limit: usize) -> (String, Option<usize>, usize) {
    let total = text.chars().count();
    let slice: String = text.chars().skip(offset).take(limit).collect();
    let consumed = offset.saturating_add(slice.chars().count());
    let next = (consumed < total).then_some(consumed);
    (slice, next, total)
}

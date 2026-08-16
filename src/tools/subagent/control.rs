use super::args::string_arg;
use crate::i18n::text as t;
use crate::tools::subagent_runner::SubagentStats;
use crate::tools::subagent_state;
use anyhow::Result;
use serde_json::{json, Value};

/// 生成子智能体统计 JSON。
///
/// 参数:
/// - `stats`: 子代理统计
///
/// 返回:
/// - 公开统计信息
pub(super) fn stats_json(stats: &SubagentStats) -> Value {
    let mut value = stats.public();
    if let Value::Object(map) = &mut value {
        map.insert("budget_reached".to_string(), json!(stats.budget_reached));
    }
    value
}

/// 查询单个后台子智能体状态。
///
/// 参数:
/// - `args`: 查询参数
/// - `owner_key`: 子智能体所属者标识
///
/// 返回:
/// - 子智能体快照
pub(super) fn subagent_status(args: Value, owner_key: &str) -> Result<String> {
    let subagent_id = string_arg(&args, "subagent_id")?;
    let subagent = subagent_state::subagent_snapshot_for_owner(owner_key, &subagent_id)?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "subagent": subagent
    }))?)
}

/// 查询后台子智能体结果。
///
/// 参数:
/// - `args`: 查询参数
/// - `owner_key`: 子智能体所属者标识
///
/// 返回:
/// - 子智能体结果或当前状态
pub(super) fn subagent_result(args: Value, owner_key: &str) -> Result<String> {
    let subagent_id = string_arg(&args, "subagent_id")?;
    // 默认返回前 50 行结果，可用 max_lines 调高
    let max_lines = args
        .get("max_lines")
        .or_else(|| args.get("head_lines"))
        .and_then(Value::as_u64)
        .unwrap_or(50)
        .clamp(1, 2000) as usize;
    let mut subagent = subagent_state::subagent_snapshot_for_owner(owner_key, &subagent_id)?;
    let mut truncated = false;
    if let Some(result) = subagent.result.as_mut() {
        let (clipped, did) = clip_lines(result, max_lines);
        *result = clipped;
        truncated |= did;
    }
    if let Some(error) = subagent.error.as_mut() {
        let (clipped, did) = clip_lines(error, max_lines);
        *error = clipped;
        truncated |= did;
    }
    if subagent.status != "running" {
        // 显式读取已经把终态结果交给主模型，后续不再发送重复完成回执
        subagent_state::acknowledge_finished_notices(owner_key, std::slice::from_ref(&subagent_id));
    }
    Ok(serde_json::to_string_pretty(&json!({
        // idle 表示持久子智能体的当前任务段已成功完成
        "ok": subagent.status == "completed" || subagent.status == "idle",
        "subagent": subagent,
        "max_lines": max_lines,
        "truncated": truncated,
    }))?)
}

/// 按行截断文本。
///
/// 参数:
/// - `text`: 原文
/// - `max_lines`: 最大行数
///
/// 返回:
/// - (截断后文本, 是否截断)
fn clip_lines(text: &str, max_lines: usize) -> (String, bool) {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max_lines {
        return (text.to_string(), false);
    }
    let mut clipped = lines[..max_lines].join("\n");
    clipped.push_str(&format!(
        "\n… +{} lines truncated (raise max_lines to read more)",
        lines.len() - max_lines
    ));
    (clipped, true)
}

/// 列出指定所属者的后台子智能体。
///
/// 参数:
/// - `owner_key`: 子智能体所属者标识
///
/// 返回:
/// - 子智能体列表
pub(super) fn subagent_list(owner_key: &str) -> Result<String> {
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "subagents": subagent_state::list_subagents_for_owner(owner_key)
    }))?)
}

/// 取消后台子智能体。
///
/// 参数:
/// - `args`: 取消参数
/// - `owner_key`: 子智能体所属者标识
///
/// 返回:
/// - 取消后的子智能体快照
pub(super) fn subagent_cancel(args: Value, owner_key: &str) -> Result<String> {
    let subagent_id = string_arg(&args, "subagent_id")?;
    let subagent = subagent_state::cancel_subagent_for_owner(owner_key, &subagent_id)?;
    subagent_state::acknowledge_finished_notices(owner_key, std::slice::from_ref(&subagent_id));
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "subagent": subagent
    }))?)
}

/// 向存活的子智能体投递追加消息。
///
/// 消息进入其消息队列，在下一个步间间隙注入对话；待命中的
/// 持久子智能体会被唤醒开始新任务段。
///
/// 参数:
/// - `args`: 发送参数（subagent_id 与 message）
/// - `owner_key`: 子智能体所属者标识
///
/// 返回:
/// - 入队后的子智能体快照
pub(super) fn subagent_send(args: Value, owner_key: &str) -> Result<String> {
    let subagent_id = string_arg(&args, "subagent_id")?;
    let message = string_arg(&args, "message")?;
    let subagent = subagent_state::queue_subagent_message_for_owner(
        owner_key,
        &subagent_id,
        "parent",
        &message,
    )?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "subagent": subagent,
        "message": t(
            "message queued; it is injected at the subagent's next step boundary. A system-reminder arrives when the segment finishes",
            "消息已入队,将在子智能体下一个步间间隙注入。该任务段完成时会收到系统提醒"
        )
    }))?)
}

/// 请求持久子智能体优雅结束。
///
/// idle 态立即收尾；running 态在当前任务段完成后收尾。收尾后按
/// `apply` 参数决定是否把 worktree 变更合并回主工作区（默认合并）。
///
/// 参数:
/// - `args`: 结束参数（subagent_id 与可选 apply）
/// - `owner_key`: 子智能体所属者标识
///
/// 返回:
/// - 登记结束请求后的子智能体快照
pub(super) fn subagent_stop(args: Value, owner_key: &str) -> Result<String> {
    let subagent_id = string_arg(&args, "subagent_id")?;
    let apply = args.get("apply").and_then(Value::as_bool).unwrap_or(true);
    let subagent = subagent_state::request_subagent_stop_for_owner(owner_key, &subagent_id, apply)?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "subagent": subagent,
        "apply": apply,
        "message": t(
            "stop requested; an idle subagent finishes immediately, a running one finishes after its current segment. A system-reminder arrives on completion",
            "已请求结束;待命中的子智能体立即收尾,运行中的在当前任务段完成后收尾。完成时会收到系统提醒"
        )
    }))?)
}

use super::args::string_arg;
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
    Ok(serde_json::to_string_pretty(&json!({
        "ok": subagent.status == "completed",
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
    clipped.push_str(&format!("\n… +{} lines truncated (raise max_lines to read more)", lines.len() - max_lines));
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
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "subagent": subagent
    }))?)
}

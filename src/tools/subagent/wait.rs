use super::args::optional_string_arg;
use crate::i18n::text as t;
use crate::tools::{subagent_state, ToolProgress};
use anyhow::Result;
use serde_json::{json, Value};
use std::time::Duration;

const WAIT_DEFAULT_SECONDS: u64 = 180;
const WAIT_MAX_SECONDS: u64 = 600;
const WAIT_POLL_MILLIS: u64 = 500;
const WAIT_REPORT_EVERY_SECONDS: u64 = 5;

/// 阻塞等待子智能体进入终态。
///
/// 指定 subagent_id 时等待该子智能体;未指定时等待任意一个新完成的子智能体。
/// 返回结果时同步确认完成通知，避免后台事件再次投递相同结果。
///
/// 参数:
/// - `args`: 等待参数
/// - `progress`: 主对话工具进度上报器
/// - `owner_key`: 父会话稳定作用域键
///
/// 返回:
/// - 完成子智能体的快照,或超时说明
pub(super) async fn wait_subagent(
    args: Value,
    progress: ToolProgress,
    owner_key: &str,
) -> Result<String> {
    let subagent_id = optional_string_arg(&args, "subagent_id")?.filter(|id| !id.is_empty());
    let timeout = args
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(WAIT_DEFAULT_SECONDS)
        .clamp(5, WAIT_MAX_SECONDS);
    let started = tokio::time::Instant::now();
    let mut last_report = 0u64;
    loop {
        // 1. 指定 id:该子智能体离开 running（进入终态或持久待命）即返回
        if let Some(id) = &subagent_id {
            let snapshot = subagent_state::subagent_snapshot_for_owner(owner_key, id)?;
            if snapshot.status != "running" {
                subagent_state::acknowledge_finished_notices(owner_key, std::slice::from_ref(id));
                return Ok(serde_json::to_string_pretty(&json!({
                    // idle 表示持久子智能体的当前任务段已成功完成
                    "ok": snapshot.status == "completed" || snapshot.status == "idle",
                    "subagent": snapshot
                }))?);
            }
        } else {
            // 2. 未指定 id:任意新完成的子智能体即返回,并消费其通知事件
            let notices = subagent_state::pending_finished_notices(owner_key);
            if !notices.is_empty() {
                let finished = notices
                    .iter()
                    .filter_map(|notice| subagent_state::subagent_snapshot(&notice.id).ok())
                    .collect::<Vec<_>>();
                let ids = notices
                    .iter()
                    .map(|notice| notice.id.clone())
                    .collect::<Vec<_>>();
                subagent_state::acknowledge_finished_notices(owner_key, &ids);
                return Ok(serde_json::to_string_pretty(&json!({
                    "ok": true,
                    "finished": finished
                }))?);
            }
            let subagents = subagent_state::list_subagents_for_owner(owner_key);
            if subagents
                .iter()
                .all(|snapshot| snapshot.status != "running")
            {
                return Ok(serde_json::to_string_pretty(&json!({
                    "ok": true,
                    "message": t(
                        "no running subagents to wait for; results were already delivered",
                        "没有运行中的子智能体可等待,结果此前已经送达"
                    ),
                    "subagents": subagents
                }))?);
            }
        }
        let elapsed = started.elapsed().as_secs();
        if elapsed >= timeout {
            return Ok(serde_json::to_string_pretty(&json!({
                "ok": false,
                "timeout": true,
                "message": t(
                    "wait timed out while subagents are still running; continue other work, a system-reminder arrives on finish",
                    "等待超时,子智能体仍在运行;请先继续其他工作,完成时会收到系统提醒"
                )
            }))?);
        }
        // 3. 周期性上报等待状态,避免前端看起来卡死
        if elapsed >= last_report + WAIT_REPORT_EVERY_SECONDS {
            last_report = elapsed;
            progress.report(if crate::i18n::is_zh() {
                format!("等待子智能体完成,已等待 {elapsed} 秒")
            } else {
                format!("waiting for subagent, {elapsed}s elapsed")
            });
        }
        tokio::time::sleep(Duration::from_millis(WAIT_POLL_MILLIS)).await;
    }
}

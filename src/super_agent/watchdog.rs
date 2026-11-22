use super::model::{
    now_millis, MemberStatus, MessageKind, MessageSenderKind, ReportStatus, WatchdogEvent,
    WatchdogSeverity,
};
use super::service::SuperAgentService;
use super::store::NewGroupMessage;
use crate::config::AppConfig;
use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

/// 待审报告在被 Watchdog 提醒前允许 Leader 自行处理的宽限时长（毫秒）。
const REPORT_SYNTHESIS_GRACE_MS: i64 = 20_000;

/// 【超级智能体】【Watchdog 巡检】执行一次外部 Control Loop 健康检查与异常自愈（论文第 6 节）。
///
/// 检查三类系统与任务进展异常：
/// 1. 失联/孤儿 Runner：数据库中为 `running` 但已不在本进程活跃表中且超过心跳宽限，自动标记失败并生成故障报告唤醒 Leader
/// 2. 进展停滞 Runner：心跳超过 `worker_stall_timeout_seconds` 未更新，发布 `action_required` 告警并在群组提示
/// 3. 未及时归纳交付的 Runner 报告：存在完成报告超过宽限时间仍未 `synthesized_by_leader`，主动重新唤醒 Leader 归纳交付
///
/// 参数:
/// - `service`: Super-Agent 核心服务句柄
/// - `record_healthy`: 无异常时是否写入一条 Healthy 巡检记录（手动触发巡检时为 true）
///
/// 返回:
/// - 本次巡检产生的新 Watchdog 事件列表
pub async fn run_watchdog_pass(
    service: &Arc<SuperAgentService>,
    record_healthy: bool,
) -> Result<Vec<WatchdogEvent>> {
    let config = AppConfig::load_or_default(service.paths()).unwrap_or_default();
    let stall_ms = (config.super_agent.worker_stall_timeout_seconds as i64) * 1000;
    let now = now_millis();
    let mut created = Vec::new();

    // 1. 扫描所有运行中 Worker 的存活与进展心跳
    let running_workers = service.store().list_running_workers()?;
    for worker in running_workers {
        let elapsed = now.saturating_sub(worker.last_heartbeat_at);
        let live_in_process = service.worker_executor().is_live(&worker.id);
        if !live_in_process && elapsed > 45_000 {
            let summary = format!(
                "Runner {} lost process liveness ({}s without heartbeat); marked failed and reported to Leader",
                worker.name,
                elapsed / 1000
            );
            let _ = service.store().update_member_state(
                &worker.id,
                MemberStatus::Failed,
                Some("interrupted: process exited unexpectedly"),
                Some(None),
            );
            let thread_id = service
                .store()
                .get_group(&worker.group_id)?
                .map(|g| g.default_thread_id)
                .unwrap_or_default();
            let _ = service
                .submit_runner_report(
                    &worker.group_id,
                    &thread_id,
                    worker.active_task_id.as_deref(),
                    &worker.id,
                    &worker.name,
                    ReportStatus::Failed,
                    &summary,
                    &summary,
                    Vec::new(),
                    None,
                )
                .await;
            let event = service.store().record_watchdog_event(
                Some(&worker.group_id),
                WatchdogSeverity::ActionRequired,
                "orphaned_worker",
                &summary,
                &format!("member_id={} task_id={:?}", worker.id, worker.active_task_id),
            )?;
            created.push(event);
            continue;
        }
        if elapsed > stall_ms {
            let summary = format!(
                "Runner {} appears stalled ({}s since last progress update)",
                worker.name,
                elapsed / 1000
            );
            let event = service.store().record_watchdog_event(
                Some(&worker.group_id),
                WatchdogSeverity::ActionRequired,
                "stalled_worker",
                &summary,
                &format!(
                    "member_id={} last_activity={}",
                    worker.id, worker.last_activity
                ),
            )?;
            if let Ok(Some(group)) = service.store().get_group(&worker.group_id) {
                let _ = service.record_group_message(NewGroupMessage {
                    group_id: worker.group_id.clone(),
                    thread_id: group.default_thread_id,
                    sender_kind: MessageSenderKind::Watchdog,
                    sender_id: "watchdog".to_string(),
                    sender_name: "Watchdog".to_string(),
                    recipient_id: Some(group.leader_member_id),
                    recipient_name: Some("Leader".to_string()),
                    kind: MessageKind::Alert,
                    channel_source: None,
                    content: summary,
                    summary: None,
                    related_task_id: worker.active_task_id.clone(),
                    related_report_id: None,
                });
            }
            created.push(event);
        }
    }

    // 2. 检测已提交但尚未被 Leader 归纳交付的 Runner 报告，重新唤醒 Leader
    let pending_reports = service.store().list_unsynthesized_reports(None)?;
    let mut nudged_groups = std::collections::HashSet::new();
    for report in pending_reports {
        let age_ms = now.saturating_sub(report.created_at);
        if age_ms >= REPORT_SYNTHESIS_GRACE_MS
            && !service.leader_coordinator().is_busy(&report.group_id)
            && nudged_groups.insert(report.group_id.clone())
        {
            let summary = format!(
                "Re-waking Leader in group {} to synthesize pending report {} from {}",
                report.group_id, report.id, report.worker_name
            );
            let event = service.store().record_watchdog_event(
                Some(&report.group_id),
                WatchdogSeverity::Notice,
                "undelivered_report_nudge",
                &summary,
                &format!("report_id={} age_ms={age_ms}", report.id),
            )?;
            service
                .leader_coordinator()
                .wake(Arc::clone(service), report.group_id.clone());
            created.push(event);
        }
    }

    // 3. 无异常时清理历史同类告警，手动巡检则追加一条健康记录
    if created.is_empty() && record_healthy {
        let overview = service.store().overview()?;
        let event = service.store().record_watchdog_event(
            None,
            WatchdogSeverity::Healthy,
            "health_check",
            &format!(
                "Watchdog check passed: {} group(s), {} running runner(s), {} active task(s)",
                overview.group_count, overview.running_workers, overview.active_tasks
            ),
            "",
        )?;
        created.push(event);
    }

    if !created.is_empty() {
        service.emit_event(
            "watchdog.updated",
            None,
            json!({ "count": created.len() }),
        );
    }
    Ok(created)
}

/// 启动后台定时 Watchdog 巡检任务（常驻 Web / 网关服务端运行）。
///
/// 参数:
/// - `service`: Super-Agent 核心服务句柄
///
/// 返回:
/// - 后台任务 JoinHandle
pub fn spawn_watchdog_loop(service: Arc<SuperAgentService>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let interval_secs = AppConfig::load_or_default(service.paths())
                .map(|cfg| {
                    if cfg.super_agent.watchdog_enabled {
                        cfg.super_agent.watchdog_interval_seconds.max(5)
                    } else {
                        60
                    }
                })
                .unwrap_or(30);
            tokio::time::sleep(Duration::from_secs(interval_secs)).await;
            let enabled = AppConfig::load_or_default(service.paths())
                .map(|cfg| cfg.super_agent.enabled && cfg.super_agent.watchdog_enabled)
                .unwrap_or(true);
            if !enabled {
                continue;
            }
            let _ = run_watchdog_pass(&service, false).await;
        }
    })
}

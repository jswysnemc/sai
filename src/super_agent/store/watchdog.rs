use super::SuperAgentStore;
use crate::super_agent::model::{
    new_id, now_millis, GroupDetailSnapshot, GroupSummary, MemberRole, MemberStatus,
    SuperAgentOverview, TaskStatus, WatchdogEvent, WatchdogSeverity,
};
use anyhow::{anyhow, Result};
use rusqlite::params;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

impl SuperAgentStore {
    /// 记录一条 Watchdog 巡检或异常自愈事件。
    ///
    /// 参数:
    /// - `group_id`: 关联群组（全局检查可为空）
    /// - `severity`: 严重级别
    /// - `kind`: 事件分类键
    /// - `summary`: 一句话摘要
    /// - `details`: 详细诊断内容
    ///
    /// 返回:
    /// - 已落盘 Watchdog 事件
    pub fn record_watchdog_event(
        &self,
        group_id: Option<&str>,
        severity: WatchdogSeverity,
        kind: &str,
        summary: &str,
        details: &str,
    ) -> Result<WatchdogEvent> {
        let now = now_millis();
        let id = new_id("wdg");
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO super_watchdog_events
             (id, group_id, severity, kind, summary, details, resolved, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
            params![
                id,
                group_id,
                severity.as_str(),
                kind,
                summary.trim(),
                details.trim(),
                now,
            ],
        )?;
        Ok(WatchdogEvent {
            id,
            group_id: group_id.map(ToOwned::to_owned),
            severity,
            kind: kind.to_string(),
            summary: summary.trim().to_string(),
            details: details.trim().to_string(),
            resolved: false,
            created_at: now,
        })
    }

    /// 列出最近的 Watchdog 巡检记录。
    ///
    /// 参数:
    /// - `limit`: 最大返回条数
    ///
    /// 返回:
    /// - Watchdog 事件列表
    pub fn list_watchdog_events(&self, limit: usize) -> Result<Vec<WatchdogEvent>> {
        let conn = self.conn()?;
        let cap = limit.clamp(1, 100) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, group_id, severity, kind, summary, details, resolved, created_at
             FROM super_watchdog_events
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![cap], map_watchdog_event)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 将同一群组与事件类型的旧告警标记为已恢复。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    /// - `kind`: 事件分类键
    ///
    /// 返回:
    /// - 更新条数
    pub fn resolve_watchdog_kind(&self, group_id: Option<&str>, kind: &str) -> Result<usize> {
        let conn = self.conn()?;
        let affected = match group_id {
            Some(gid) => conn.execute(
                "UPDATE super_watchdog_events SET resolved = 1
                 WHERE group_id = ?1 AND kind = ?2 AND resolved = 0",
                params![gid, kind],
            )?,
            None => conn.execute(
                "UPDATE super_watchdog_events SET resolved = 1
                 WHERE group_id IS NULL AND kind = ?1 AND resolved = 0",
                params![kind],
            )?,
        };
        Ok(affected)
    }

    /// 聚合单个群组的摘要信息。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    ///
    /// 返回:
    /// - 群组摘要
    pub fn group_summary(&self, group_id: &str) -> Result<GroupSummary> {
        let group = self
            .get_group(group_id)?
            .ok_or_else(|| anyhow!("group not found: {group_id}"))?;
        let members = self.list_members(group_id)?;
        let leader_status = members
            .iter()
            .find(|member| member.role == MemberRole::Leader)
            .map(|member| member.status)
            .unwrap_or(MemberStatus::Idle);
        let workers: Vec<_> = members
            .iter()
            .filter(|member| member.role == MemberRole::Worker)
            .collect();
        let running_worker_count = workers
            .iter()
            .filter(|member| member.status == MemberStatus::Running)
            .count();
        let tasks = self.list_tasks(group_id)?;
        let open_task_count = tasks
            .iter()
            .filter(|task| {
                matches!(
                    task.status,
                    TaskStatus::Pending | TaskStatus::Running | TaskStatus::Blocked
                )
            })
            .count();
        let pending_review_count = tasks
            .iter()
            .filter(|task| task.status == TaskStatus::WaitingReview)
            .count();
        let messages = self.list_messages(group_id, 1, None)?;
        let last_msg = messages.last();
        let conn = self.conn()?;
        let message_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM super_messages WHERE group_id = ?1",
            params![group_id],
            |row| row.get(0),
        )?;
        let bound_channels = self
            .list_channel_bindings(Some(group_id))?
            .into_iter()
            .filter(|binding| binding.enabled)
            .map(|binding| binding.channel)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(GroupSummary {
            group,
            leader_status,
            worker_count: workers.len(),
            running_worker_count,
            open_task_count,
            pending_review_count,
            message_count: message_count.max(0) as usize,
            bound_channels,
            last_message_preview: last_msg.map(|msg| truncate_preview(&msg.content, 96)),
            last_message_at: last_msg.map(|msg| msg.created_at),
        })
    }

    /// 组装单个群组的完整详情快照。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    /// - `message_limit`: 消息加载条数上限
    ///
    /// 返回:
    /// - 群组完整快照
    pub fn group_detail(&self, group_id: &str, message_limit: usize) -> Result<GroupDetailSnapshot> {
        let group = self
            .get_group(group_id)?
            .ok_or_else(|| anyhow!("group not found: {group_id}"))?;
        Ok(GroupDetailSnapshot {
            group,
            members: self.list_members(group_id)?,
            threads: self.list_threads(group_id)?,
            subscriptions: self.list_subscriptions(group_id)?,
            messages: self.list_messages(group_id, message_limit, None)?,
            tasks: self.list_tasks(group_id)?,
            reports: self.list_reports(group_id, 80)?,
            triggers: self.list_triggers(group_id, 100)?,
            channels: self.list_channel_bindings(Some(group_id))?,
            deliveries: self.list_delivery_receipts(group_id, 50)?,
        })
    }

    /// 汇总全局 Super-Agent 监控概览。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 全局概览快照
    pub fn overview(&self) -> Result<SuperAgentOverview> {
        let groups = self.list_groups()?;
        let mut summaries = Vec::with_capacity(groups.len());
        let mut running_workers = 0usize;
        let mut total_workers = 0usize;
        let mut active_tasks = 0usize;
        for group in &groups {
            let summary = self.group_summary(&group.id)?;
            running_workers += summary.running_worker_count;
            total_workers += summary.worker_count;
            active_tasks += summary.open_task_count;
            summaries.push(summary);
        }
        let since = now_millis().saturating_sub(DAY_MS);
        let conn = self.conn()?;
        let pending_synthesis_reports: i64 = conn.query_row(
            "SELECT COUNT(*) FROM super_reports
             WHERE synthesized_by_leader = 0 AND status IN ('ready', 'blocked', 'failed', 'waiting')",
            [],
            |row| row.get(0),
        )?;
        let delivered_tasks_24h: i64 = conn.query_row(
            "SELECT COUNT(*) FROM super_tasks WHERE status = 'delivered' AND updated_at >= ?1",
            params![since],
            |row| row.get(0),
        )?;
        let triggers_24h: i64 = conn.query_row(
            "SELECT COUNT(*) FROM super_triggers WHERE created_at >= ?1",
            params![since],
            |row| row.get(0),
        )?;
        let fast_path_triggers_24h: i64 = conn.query_row(
            "SELECT COUNT(*) FROM super_triggers WHERE created_at >= ?1 AND route_path = 'fast_path'",
            params![since],
            |row| row.get(0),
        )?;
        let slow_path_triggers_24h: i64 = conn.query_row(
            "SELECT COUNT(*) FROM super_triggers WHERE created_at >= ?1 AND route_path = 'slow_path'",
            params![since],
            |row| row.get(0),
        )?;
        let watchdog_alerts: i64 = conn.query_row(
            "SELECT COUNT(*) FROM super_watchdog_events
             WHERE resolved = 0 AND severity = 'action_required'",
            [],
            |row| row.get(0),
        )?;
        let bound_channel_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM super_channel_bindings WHERE enabled = 1",
            [],
            |row| row.get(0),
        )?;
        let recent_watchdog = self.list_watchdog_events(20)?;
        Ok(SuperAgentOverview {
            group_count: summaries.len(),
            running_workers,
            total_workers,
            active_tasks,
            pending_synthesis_reports: pending_synthesis_reports.max(0) as usize,
            delivered_tasks_24h: delivered_tasks_24h.max(0) as usize,
            triggers_24h: triggers_24h.max(0) as usize,
            fast_path_triggers_24h: fast_path_triggers_24h.max(0) as usize,
            slow_path_triggers_24h: slow_path_triggers_24h.max(0) as usize,
            watchdog_alerts: watchdog_alerts.max(0) as usize,
            bound_channel_count: bound_channel_count.max(0) as usize,
            groups: summaries,
            recent_watchdog,
        })
    }
}

/// 截断消息文本生成单行预览。
///
/// 参数:
/// - `text`: 原始文本
/// - `max_chars`: 最大字符数
///
/// 返回:
/// - 压缩空白后的单行预览
fn truncate_preview(text: &str, max_chars: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = compact.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{head}...")
    } else {
        head
    }
}

/// 映射 Watchdog 事件行。
fn map_watchdog_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<WatchdogEvent> {
    let severity_raw: String = row.get(2)?;
    let resolved: i64 = row.get(6)?;
    Ok(WatchdogEvent {
        id: row.get(0)?,
        group_id: row.get(1)?,
        severity: WatchdogSeverity::parse(&severity_raw),
        kind: row.get(3)?,
        summary: row.get(4)?,
        details: row.get(5)?,
        resolved: resolved != 0,
        created_at: row.get(7)?,
    })
}

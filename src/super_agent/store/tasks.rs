use super::SuperAgentStore;
use crate::super_agent::model::{
    new_id, now_millis, ReportStatus, RunnerReport, SubscriptionMode, SuperTask, TaskStatus,
    ThreadSubscription,
};
use anyhow::{anyhow, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::Value;

/// 创建新任务的输入载荷。
#[derive(Debug, Clone)]
pub struct NewSuperTask {
    pub group_id: String,
    pub thread_id: String,
    pub title: String,
    pub objective: String,
    pub assigned_member_id: Option<String>,
    pub assigned_member_name: Option<String>,
    pub origin_channel: String,
    pub origin_target: Value,
}

/// 提交 Runner 结构化报告的输入载荷。
#[derive(Debug, Clone)]
pub struct NewRunnerReport {
    pub group_id: String,
    pub thread_id: String,
    pub task_id: Option<String>,
    pub worker_member_id: String,
    pub worker_name: String,
    pub status: ReportStatus,
    pub summary: String,
    pub details: String,
    pub artifacts: Vec<String>,
    pub needs_human_question: Option<String>,
}

impl SuperAgentStore {
    /// 创建任务工单（Ticket）。
    ///
    /// 参数:
    /// - `input`: 新任务载荷
    ///
    /// 返回:
    /// - 已落盘任务记录
    pub fn create_task(&self, input: NewSuperTask) -> Result<SuperTask> {
        let now = now_millis();
        let id = new_id("tsk");
        let status = if input.assigned_member_id.is_some() {
            TaskStatus::Running
        } else {
            TaskStatus::Pending
        };
        let target_json = serde_json::to_string(&input.origin_target)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO super_tasks
             (id, group_id, thread_id, title, objective, assigned_member_id, assigned_member_name,
              origin_channel, origin_target_json, status, latest_report_status,
              latest_report_summary, deliverable, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, NULL, NULL, ?11, ?12)",
            params![
                id,
                input.group_id,
                input.thread_id,
                input.title.trim(),
                input.objective.trim(),
                input.assigned_member_id,
                input.assigned_member_name,
                input.origin_channel,
                target_json,
                status.as_str(),
                now,
                now,
            ],
        )?;
        self.touch_group(&input.group_id)?;
        Ok(SuperTask {
            id,
            group_id: input.group_id,
            thread_id: input.thread_id,
            title: input.title.trim().to_string(),
            objective: input.objective.trim().to_string(),
            assigned_member_id: input.assigned_member_id,
            assigned_member_name: input.assigned_member_name,
            origin_channel: input.origin_channel,
            origin_target: input.origin_target,
            status,
            latest_report_status: None,
            latest_report_summary: None,
            deliverable: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// 按任务 ID 查询任务。
    ///
    /// 参数:
    /// - `task_id`: 任务标识
    ///
    /// 返回:
    /// - 任务记录
    pub fn get_task(&self, task_id: &str) -> Result<Option<SuperTask>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, group_id, thread_id, title, objective, assigned_member_id,
                    assigned_member_name, origin_channel, origin_target_json, status,
                    latest_report_status, latest_report_summary, deliverable, created_at, updated_at
             FROM super_tasks WHERE id = ?1",
            params![task_id],
            map_task,
        )
        .optional()
        .map_err(Into::into)
    }

    /// 列出群组内全部任务（按最近更新倒序）。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    ///
    /// 返回:
    /// - 任务列表
    pub fn list_tasks(&self, group_id: &str) -> Result<Vec<SuperTask>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, group_id, thread_id, title, objective, assigned_member_id,
                    assigned_member_name, origin_channel, origin_target_json, status,
                    latest_report_status, latest_report_summary, deliverable, created_at, updated_at
             FROM super_tasks
             WHERE group_id = ?1
             ORDER BY updated_at DESC, created_at DESC",
        )?;
        let rows = stmt.query_map(params![group_id], map_task)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 更新任务分配、状态、最新报告摘要或 Leader 最终交付物。
    ///
    /// 参数:
    /// - `task_id`: 任务标识
    /// - `status`: 可选新状态
    /// - `assigned_member`: 可选新执行成员 `(id, name)`
    /// - `report_status`: 可选最新报告状态
    /// - `report_summary`: 可选最新报告摘要
    /// - `deliverable`: 可选最终交付正文
    ///
    /// 返回:
    /// - 更新后的任务
    pub fn update_task(
        &self,
        task_id: &str,
        status: Option<TaskStatus>,
        assigned_member: Option<(String, String)>,
        report_status: Option<ReportStatus>,
        report_summary: Option<String>,
        deliverable: Option<String>,
    ) -> Result<SuperTask> {
        let mut task = self
            .get_task(task_id)?
            .ok_or_else(|| anyhow!("task not found: {task_id}"))?;
        if let Some(next_status) = status {
            task.status = next_status;
        }
        if let Some((member_id, member_name)) = assigned_member {
            task.assigned_member_id = Some(member_id);
            task.assigned_member_name = Some(member_name);
        }
        if let Some(rep_status) = report_status {
            task.latest_report_status = Some(rep_status);
        }
        if let Some(summary) = report_summary {
            task.latest_report_summary = Some(summary);
        }
        if let Some(body) = deliverable {
            task.deliverable = Some(body);
        }
        task.updated_at = now_millis();
        let conn = self.conn()?;
        conn.execute(
            "UPDATE super_tasks
             SET assigned_member_id = ?2, assigned_member_name = ?3, status = ?4,
                 latest_report_status = ?5, latest_report_summary = ?6,
                 deliverable = ?7, updated_at = ?8
             WHERE id = ?1",
            params![
                task.id,
                task.assigned_member_id,
                task.assigned_member_name,
                task.status.as_str(),
                task.latest_report_status.map(ReportStatus::as_str),
                task.latest_report_summary,
                task.deliverable,
                task.updated_at,
            ],
        )?;
        Ok(task)
    }

    /// 持久化 Runner 提交给 Leader 的结构化报告，并同步推进对应任务状态。
    ///
    /// 参数:
    /// - `input`: 结构化报告载荷
    ///
    /// 返回:
    /// - 已落盘的报告记录
    pub fn create_report(&self, input: NewRunnerReport) -> Result<RunnerReport> {
        let now = now_millis();
        let id = new_id("rep");
        let artifacts_json = serde_json::to_string(&input.artifacts)?;
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO super_reports
             (id, group_id, thread_id, task_id, worker_member_id, worker_name, status,
              summary, details, artifacts_json, needs_human_question, synthesized_by_leader, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, ?12)",
            params![
                id,
                input.group_id,
                input.thread_id,
                input.task_id,
                input.worker_member_id,
                input.worker_name,
                input.status.as_str(),
                input.summary.trim(),
                input.details.trim(),
                artifacts_json,
                input.needs_human_question,
                now,
            ],
        )?;
        // 1. 联动更新任务状态，使 waiting_review / blocked / failed 实时反映在看板中
        if let Some(task_id) = input.task_id.as_deref() {
            let next_task_status = match input.status {
                ReportStatus::Ready => TaskStatus::WaitingReview,
                ReportStatus::Blocked => TaskStatus::Blocked,
                ReportStatus::Failed => TaskStatus::Failed,
                ReportStatus::Dropped => TaskStatus::Stopped,
                ReportStatus::Progress | ReportStatus::Waiting => TaskStatus::Running,
            };
            let _ = self.update_task(
                task_id,
                Some(next_task_status),
                None,
                Some(input.status),
                Some(input.summary.trim().to_string()),
                None,
            );
        }
        self.touch_group(&input.group_id)?;
        Ok(RunnerReport {
            id,
            group_id: input.group_id,
            thread_id: input.thread_id,
            task_id: input.task_id,
            worker_member_id: input.worker_member_id,
            worker_name: input.worker_name,
            status: input.status,
            summary: input.summary.trim().to_string(),
            details: input.details.trim().to_string(),
            artifacts: input.artifacts,
            needs_human_question: input.needs_human_question,
            synthesized_by_leader: false,
            created_at: now,
        })
    }

    /// 按 ID 读取单条报告详情（供 Chain of Recall 使用）。
    ///
    /// 参数:
    /// - `report_id`: 报告标识
    ///
    /// 返回:
    /// - 报告记录
    pub fn get_report(&self, report_id: &str) -> Result<Option<RunnerReport>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, group_id, thread_id, task_id, worker_member_id, worker_name, status,
                    summary, details, artifacts_json, needs_human_question, synthesized_by_leader, created_at
             FROM super_reports WHERE id = ?1",
            params![report_id],
            map_report,
        )
        .optional()
        .map_err(Into::into)
    }

    /// 列出群组内全部 Runner 报告。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    /// - `limit`: 最大返回条数
    ///
    /// 返回:
    /// - 按时间倒序排列的报告列表
    pub fn list_reports(&self, group_id: &str, limit: usize) -> Result<Vec<RunnerReport>> {
        let conn = self.conn()?;
        let cap = limit.clamp(1, 200) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, group_id, thread_id, task_id, worker_member_id, worker_name, status,
                    summary, details, artifacts_json, needs_human_question, synthesized_by_leader, created_at
             FROM super_reports
             WHERE group_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![group_id, cap], map_report)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 列出全部尚未被 Leader 归纳交付的待审报告（供 Watchdog 与 Leader 批处理使用）。
    ///
    /// 参数:
    /// - `group_id`: 可选群组过滤
    ///
    /// 返回:
    /// - 待归纳的报告列表
    pub fn list_unsynthesized_reports(&self, group_id: Option<&str>) -> Result<Vec<RunnerReport>> {
        let conn = self.conn()?;
        let sql = if group_id.is_some() {
            "SELECT id, group_id, thread_id, task_id, worker_member_id, worker_name, status,
                    summary, details, artifacts_json, needs_human_question, synthesized_by_leader, created_at
             FROM super_reports
             WHERE group_id = ?1 AND synthesized_by_leader = 0
               AND status IN ('ready', 'blocked', 'failed', 'waiting')
             ORDER BY created_at ASC"
        } else {
            "SELECT id, group_id, thread_id, task_id, worker_member_id, worker_name, status,
                    summary, details, artifacts_json, needs_human_question, synthesized_by_leader, created_at
             FROM super_reports
             WHERE synthesized_by_leader = 0
               AND status IN ('ready', 'blocked', 'failed', 'waiting')
             ORDER BY created_at ASC"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = match group_id {
            Some(gid) => stmt.query_map(params![gid], map_report)?,
            None => stmt.query_map([], map_report)?,
        };
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 将指定报告或关联任务下的全部待审报告标记为已由 Leader 归纳交付。
    ///
    /// 参数:
    /// - `report_ids`: 报告 ID 集合
    /// - `task_id`: 可选任务 ID（将关联报告一并标记）
    ///
    /// 返回:
    /// - 更新条数
    pub fn mark_reports_synthesized(
        &self,
        report_ids: &[String],
        task_id: Option<&str>,
    ) -> Result<usize> {
        let conn = self.conn()?;
        let mut updated = 0usize;
        for id in report_ids {
            updated += conn.execute(
                "UPDATE super_reports SET synthesized_by_leader = 1 WHERE id = ?1",
                params![id],
            )?;
        }
        if let Some(tid) = task_id {
            updated += conn.execute(
                "UPDATE super_reports SET synthesized_by_leader = 1 WHERE task_id = ?1",
                params![tid],
            )?;
        }
        Ok(updated)
    }

    /// 绑定或更新 Worker 对 Thread 的订阅关系（支持多对多订阅直通）。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    /// - `thread_id`: 话题线程标识
    /// - `member_id`: 成员标识
    /// - `mode`: 订阅模式（owner 或 context）
    ///
    /// 返回:
    /// - 生效的订阅记录
    pub fn upsert_subscription(
        &self,
        group_id: &str,
        thread_id: &str,
        member_id: &str,
        mode: SubscriptionMode,
    ) -> Result<ThreadSubscription> {
        let now = now_millis();
        let id = new_id("sub");
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO super_subscriptions (id, group_id, thread_id, member_id, mode, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(thread_id, member_id) DO UPDATE SET mode = excluded.mode",
            params![id, group_id, thread_id, member_id, mode.as_str(), now],
        )?;
        conn.query_row(
            "SELECT id, group_id, thread_id, member_id, mode, created_at
             FROM super_subscriptions WHERE thread_id = ?1 AND member_id = ?2",
            params![thread_id, member_id],
            map_subscription,
        )
        .map_err(Into::into)
    }

    /// 取消 Worker 对 Thread 的订阅。
    ///
    /// 参数:
    /// - `thread_id`: 话题线程标识
    /// - `member_id`: 成员标识
    ///
    /// 返回:
    /// - 是否移除了订阅
    pub fn remove_subscription(&self, thread_id: &str, member_id: &str) -> Result<bool> {
        let conn = self.conn()?;
        let affected = conn.execute(
            "DELETE FROM super_subscriptions WHERE thread_id = ?1 AND member_id = ?2",
            params![thread_id, member_id],
        )?;
        Ok(affected > 0)
    }

    /// 列出群组内全部订阅关系。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    ///
    /// 返回:
    /// - 订阅关系列表
    pub fn list_subscriptions(&self, group_id: &str) -> Result<Vec<ThreadSubscription>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, group_id, thread_id, member_id, mode, created_at
             FROM super_subscriptions
             WHERE group_id = ?1
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![group_id], map_subscription)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 查询某个 Thread 的全部订阅者。
    ///
    /// 参数:
    /// - `thread_id`: 话题线程标识
    ///
    /// 返回:
    /// - 订阅者记录列表
    pub fn thread_subscriptions(&self, thread_id: &str) -> Result<Vec<ThreadSubscription>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, group_id, thread_id, member_id, mode, created_at
             FROM super_subscriptions
             WHERE thread_id = ?1
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![thread_id], map_subscription)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

/// 映射任务行。
fn map_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<SuperTask> {
    let target_raw: String = row.get(8)?;
    let status_raw: String = row.get(9)?;
    let report_status_raw: Option<String> = row.get(10)?;
    Ok(SuperTask {
        id: row.get(0)?,
        group_id: row.get(1)?,
        thread_id: row.get(2)?,
        title: row.get(3)?,
        objective: row.get(4)?,
        assigned_member_id: row.get(5)?,
        assigned_member_name: row.get(6)?,
        origin_channel: row.get(7)?,
        origin_target: serde_json::from_str(&target_raw).unwrap_or_else(|_| Value::Object(Default::default())),
        status: TaskStatus::parse(&status_raw),
        latest_report_status: report_status_raw.as_deref().map(ReportStatus::parse),
        latest_report_summary: row.get(11)?,
        deliverable: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

/// 映射报告行。
fn map_report(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunnerReport> {
    let status_raw: String = row.get(6)?;
    let artifacts_raw: String = row.get(9)?;
    let synthesized: i64 = row.get(11)?;
    Ok(RunnerReport {
        id: row.get(0)?,
        group_id: row.get(1)?,
        thread_id: row.get(2)?,
        task_id: row.get(3)?,
        worker_member_id: row.get(4)?,
        worker_name: row.get(5)?,
        status: ReportStatus::parse(&status_raw),
        summary: row.get(7)?,
        details: row.get(8)?,
        artifacts: serde_json::from_str(&artifacts_raw).unwrap_or_default(),
        needs_human_question: row.get(10)?,
        synthesized_by_leader: synthesized != 0,
        created_at: row.get(12)?,
    })
}

/// 映射订阅行。
fn map_subscription(row: &rusqlite::Row<'_>) -> rusqlite::Result<ThreadSubscription> {
    let mode_raw: String = row.get(4)?;
    Ok(ThreadSubscription {
        id: row.get(0)?,
        group_id: row.get(1)?,
        thread_id: row.get(2)?,
        member_id: row.get(3)?,
        mode: SubscriptionMode::parse(&mode_raw),
        created_at: row.get(5)?,
    })
}

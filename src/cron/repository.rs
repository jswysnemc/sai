use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::PathBuf;

const DISABLE_AFTER_FAILURES: i64 = 3;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CronJob {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) prompt: String,
    pub(crate) session_id: String,
    pub(crate) interval_seconds: Option<i64>,
    pub(crate) next_run_at: i64,
    pub(crate) enabled: bool,
    pub(crate) failure_count: i64,
    pub(crate) last_error: Option<String>,
}

/// Gateway 定时任务持久化仓库。
pub(crate) struct CronRepository {
    file: PathBuf,
}

impl CronRepository {
    /// 创建仓库并初始化数据库结构。
    ///
    /// 参数:
    /// - `paths`: Sai 路径集合
    ///
    /// 返回:
    /// - 已初始化的仓库
    pub(crate) fn new(paths: &SaiPaths) -> Result<Self> {
        let directory = paths.state_dir.join("cron");
        std::fs::create_dir_all(&directory)?;
        let repository = Self {
            file: directory.join("jobs.db"),
        };
        repository.initialize()?;
        Ok(repository)
    }

    /// 新增一次性或固定间隔任务。
    pub(crate) fn create(
        &self,
        name: &str,
        prompt: &str,
        session_id: &str,
        run_at: i64,
        interval_seconds: Option<i64>,
    ) -> Result<CronJob> {
        if interval_seconds.is_some_and(|value| value <= 0) {
            bail!("interval_seconds must be greater than zero");
        }
        let job = CronJob {
            id: format!("cron_{}", uuid::Uuid::new_v4().simple()),
            name: name.to_string(),
            prompt: prompt.to_string(),
            session_id: session_id.to_string(),
            interval_seconds,
            next_run_at: run_at,
            enabled: true,
            failure_count: 0,
            last_error: None,
        };
        self.connection()?.execute(
            "INSERT INTO jobs (id,name,prompt,session_id,interval_seconds,next_run_at,enabled,failure_count,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,1,0,?7,?7)",
            params![job.id, job.name, job.prompt, job.session_id, job.interval_seconds, job.next_run_at, Utc::now().timestamp()],
        )?;
        Ok(job)
    }

    /// 列出全部任务。
    pub(crate) fn list(&self) -> Result<Vec<CronJob>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT id,name,prompt,session_id,interval_seconds,next_run_at,enabled,failure_count,last_error FROM jobs ORDER BY next_run_at,id")?;
        let jobs = statement
            .query_map([], map_job)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(jobs)
    }

    /// 删除指定任务。
    pub(crate) fn remove(&self, id: &str) -> Result<bool> {
        Ok(self
            .connection()?
            .execute("DELETE FROM jobs WHERE id=?1", [id])?
            > 0)
    }

    /// 启用或停用指定任务。
    ///
    /// 参数:
    /// - `id`: 定时任务标识
    /// - `enabled`: 新启用状态
    ///
    /// 返回:
    /// - 更新后的任务
    pub(crate) fn set_enabled(&self, id: &str, enabled: bool) -> Result<CronJob> {
        let changed = self.connection()?.execute(
            "UPDATE jobs SET enabled=?2,updated_at=?3 WHERE id=?1",
            params![id, enabled, Utc::now().timestamp()],
        )?;
        if changed == 0 {
            bail!("cron job not found: {id}");
        }
        self.list()?
            .into_iter()
            .find(|job| job.id == id)
            .ok_or_else(|| anyhow::anyhow!("cron job not found after update: {id}"))
    }

    /// 读取最早到期的启用任务。
    pub(crate) fn next_due(&self, now: i64) -> Result<Option<CronJob>> {
        Ok(self.connection()?.query_row(
            "SELECT id,name,prompt,session_id,interval_seconds,next_run_at,enabled,failure_count,last_error FROM jobs WHERE enabled=1 AND next_run_at<=?1 ORDER BY next_run_at,id LIMIT 1",
            [now], map_job,
        ).optional()?)
    }

    /// 记录任务成功，并推进间隔任务或删除一次性任务。
    pub(crate) fn complete(&self, job: &CronJob, finished_at: i64) -> Result<()> {
        let connection = self.connection()?;
        match job.interval_seconds {
            Some(interval) => {
                let next = job.next_run_at.max(finished_at).saturating_add(interval);
                connection.execute("UPDATE jobs SET next_run_at=?2,failure_count=0,last_error=NULL,updated_at=?3 WHERE id=?1", params![job.id,next,finished_at])?;
            }
            None => {
                connection.execute("DELETE FROM jobs WHERE id=?1", [&job.id])?;
            }
        }
        Ok(())
    }

    /// 记录失败，连续三次失败后自动禁用。
    pub(crate) fn fail(&self, job: &CronJob, error: &str, finished_at: i64) -> Result<()> {
        let failures = job.failure_count + 1;
        let enabled = failures < DISABLE_AFTER_FAILURES;
        let next = finished_at.saturating_add(job.interval_seconds.unwrap_or(60));
        self.connection()?.execute(
            "UPDATE jobs SET enabled=?2,failure_count=?3,last_error=?4,next_run_at=?5,updated_at=?6 WHERE id=?1",
            params![job.id, enabled, failures, error, next, finished_at],
        )?;
        Ok(())
    }

    fn initialize(&self) -> Result<()> {
        self.connection()?.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS jobs(
               id TEXT PRIMARY KEY,name TEXT NOT NULL,prompt TEXT NOT NULL,session_id TEXT NOT NULL,
               interval_seconds INTEGER,next_run_at INTEGER NOT NULL,enabled INTEGER NOT NULL DEFAULT 1,
               failure_count INTEGER NOT NULL DEFAULT 0,last_error TEXT,created_at INTEGER NOT NULL,updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_cron_due ON jobs(enabled,next_run_at);",
        )?;
        Ok(())
    }

    fn connection(&self) -> Result<Connection> {
        Ok(Connection::open(&self.file)?)
    }
}

fn map_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<CronJob> {
    Ok(CronJob {
        id: row.get(0)?,
        name: row.get(1)?,
        prompt: row.get(2)?,
        session_id: row.get(3)?,
        interval_seconds: row.get(4)?,
        next_run_at: row.get(5)?,
        enabled: row.get(6)?,
        failure_count: row.get(7)?,
        last_error: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(root: &std::path::Path) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("c"),
            config_file: root.join("c/f"),
            secrets_file: root.join("c/s"),
            skills_dir: root.join("c/k"),
            data_dir: root.join("d"),
            cache_dir: root.join("x"),
            state_dir: root.join("state"),
            pictures_dir: root.join("p"),
            fish_hook_file: root.join("fish"),
            bash_hook_file: root.join("bash"),
            zsh_hook_file: root.join("zsh"),
            powershell_hook_file: root.join("pwsh"),
        }
    }

    #[test]
    fn one_shot_is_removed_and_repeating_job_advances() {
        let temp = tempfile::tempdir().unwrap();
        let repository = CronRepository::new(&paths(temp.path())).unwrap();
        let once = repository.create("once", "prompt", "s", 10, None).unwrap();
        repository.complete(&once, 10).unwrap();
        assert!(repository.list().unwrap().is_empty());
        let repeating = repository
            .create("repeat", "prompt", "s", 10, Some(5))
            .unwrap();
        repository.complete(&repeating, 10).unwrap();
        assert_eq!(repository.list().unwrap()[0].next_run_at, 15);
    }

    #[test]
    fn three_failures_disable_job() {
        let temp = tempfile::tempdir().unwrap();
        let repository = CronRepository::new(&paths(temp.path())).unwrap();
        let mut job = repository
            .create("fail", "prompt", "s", 0, Some(1))
            .unwrap();
        for time in 1..=3 {
            repository.fail(&job, "error", time).unwrap();
            job = repository.list().unwrap()[0].clone();
        }
        assert!(!job.enabled);
        assert_eq!(job.failure_count, 3);
    }
}

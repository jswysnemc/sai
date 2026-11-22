mod groups;
mod messages;
mod tasks;
mod triggers;
mod watchdog;

pub use messages::NewGroupMessage;
pub use tasks::{NewRunnerReport, NewSuperTask};
pub use triggers::NewTriggerEvent;

use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 【超级智能体】【持久化存储】基于单文件 SQLite WAL 数据库，支持 TUI、WebUI 与网关跨进程共享状态。
#[derive(Debug, Clone)]
pub struct SuperAgentStore {
    db_path: PathBuf,
}

impl SuperAgentStore {
    /// 基于 Sai 路径构造存储并完成表结构初始化。
    ///
    /// 参数:
    /// - `paths`: Sai 运行路径集合
    ///
    /// 返回:
    /// - 已初始化的存储句柄
    pub fn open(paths: &SaiPaths) -> Result<Self> {
        let dir = paths.state_dir.join("super-agent");
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create super-agent state dir {}", dir.display()))?;
        let store = Self {
            db_path: dir.join("super_agent.db"),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// 返回底层数据库文件路径。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - SQLite 数据库路径
    #[allow(dead_code)]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// 打开配置了 WAL 与忙等待超时的数据库连接。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 可执行事务的 SQLite 连接
    pub(super) fn conn(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)
            .with_context(|| format!("failed to open {}", self.db_path.display()))?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;
        Ok(conn)
    }

    /// 初始化 Super-Agent 全部领域数据表与索引。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 迁移是否成功
    fn init_schema(&self) -> Result<()> {
        let conn = self.conn()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS super_groups (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                workspace_path TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'active',
                leader_member_id TEXT NOT NULL,
                default_thread_id TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS super_members (
                id TEXT PRIMARY KEY,
                group_id TEXT NOT NULL REFERENCES super_groups(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                role TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'idle',
                specialty TEXT NOT NULL DEFAULT '',
                subagent_type TEXT NOT NULL DEFAULT 'general',
                model_label TEXT NOT NULL DEFAULT '',
                workdir TEXT NOT NULL DEFAULT '',
                worktree_branch TEXT,
                active_task_id TEXT,
                last_activity TEXT NOT NULL DEFAULT '',
                last_heartbeat_at INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_super_members_group ON super_members(group_id);

            CREATE TABLE IF NOT EXISTS super_threads (
                id TEXT PRIMARY KEY,
                group_id TEXT NOT NULL REFERENCES super_groups(id) ON DELETE CASCADE,
                title TEXT NOT NULL,
                channel TEXT NOT NULL DEFAULT 'group',
                external_ref TEXT,
                status TEXT NOT NULL DEFAULT 'open',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_super_threads_group ON super_threads(group_id, updated_at DESC);

            CREATE TABLE IF NOT EXISTS super_subscriptions (
                id TEXT PRIMARY KEY,
                group_id TEXT NOT NULL REFERENCES super_groups(id) ON DELETE CASCADE,
                thread_id TEXT NOT NULL REFERENCES super_threads(id) ON DELETE CASCADE,
                member_id TEXT NOT NULL REFERENCES super_members(id) ON DELETE CASCADE,
                mode TEXT NOT NULL DEFAULT 'owner',
                created_at INTEGER NOT NULL,
                UNIQUE(thread_id, member_id)
            );
            CREATE INDEX IF NOT EXISTS idx_super_subs_thread ON super_subscriptions(thread_id);
            CREATE INDEX IF NOT EXISTS idx_super_subs_member ON super_subscriptions(member_id);

            CREATE TABLE IF NOT EXISTS super_messages (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                id TEXT NOT NULL UNIQUE,
                group_id TEXT NOT NULL REFERENCES super_groups(id) ON DELETE CASCADE,
                thread_id TEXT NOT NULL,
                sender_kind TEXT NOT NULL,
                sender_id TEXT NOT NULL,
                sender_name TEXT NOT NULL,
                recipient_id TEXT,
                recipient_name TEXT,
                kind TEXT NOT NULL,
                channel_source TEXT,
                content TEXT NOT NULL,
                summary TEXT,
                related_task_id TEXT,
                related_report_id TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_super_messages_group_seq ON super_messages(group_id, seq);

            CREATE TABLE IF NOT EXISTS super_triggers (
                id TEXT PRIMARY KEY,
                group_id TEXT NOT NULL REFERENCES super_groups(id) ON DELETE CASCADE,
                thread_id TEXT NOT NULL,
                source_channel TEXT NOT NULL,
                actor TEXT NOT NULL,
                content TEXT NOT NULL,
                disposition TEXT NOT NULL DEFAULT 'queued',
                route_path TEXT NOT NULL DEFAULT 'slow_path',
                reason TEXT NOT NULL DEFAULT '',
                target_member_ids_json TEXT NOT NULL DEFAULT '[]',
                metadata_json TEXT NOT NULL DEFAULT '{}',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_super_triggers_group ON super_triggers(group_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS super_tasks (
                id TEXT PRIMARY KEY,
                group_id TEXT NOT NULL REFERENCES super_groups(id) ON DELETE CASCADE,
                thread_id TEXT NOT NULL,
                title TEXT NOT NULL,
                objective TEXT NOT NULL,
                assigned_member_id TEXT,
                assigned_member_name TEXT,
                origin_channel TEXT NOT NULL DEFAULT 'group',
                origin_target_json TEXT NOT NULL DEFAULT '{}',
                status TEXT NOT NULL DEFAULT 'pending',
                latest_report_status TEXT,
                latest_report_summary TEXT,
                deliverable TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_super_tasks_group ON super_tasks(group_id, updated_at DESC);

            CREATE TABLE IF NOT EXISTS super_reports (
                id TEXT PRIMARY KEY,
                group_id TEXT NOT NULL REFERENCES super_groups(id) ON DELETE CASCADE,
                thread_id TEXT NOT NULL,
                task_id TEXT,
                worker_member_id TEXT NOT NULL,
                worker_name TEXT NOT NULL,
                status TEXT NOT NULL,
                summary TEXT NOT NULL,
                details TEXT NOT NULL,
                artifacts_json TEXT NOT NULL DEFAULT '[]',
                needs_human_question TEXT,
                synthesized_by_leader INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_super_reports_group ON super_reports(group_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS super_channel_bindings (
                id TEXT PRIMARY KEY,
                group_id TEXT NOT NULL REFERENCES super_groups(id) ON DELETE CASCADE,
                channel TEXT NOT NULL,
                external_chat_id TEXT NOT NULL DEFAULT '*',
                label TEXT NOT NULL DEFAULT '',
                admission_mode TEXT NOT NULL DEFAULT 'auto',
                enabled INTEGER NOT NULL DEFAULT 1,
                last_inbound_at INTEGER,
                last_outbound_at INTEGER,
                created_at INTEGER NOT NULL,
                UNIQUE(channel, external_chat_id)
            );

            CREATE TABLE IF NOT EXISTS super_deliveries (
                id TEXT PRIMARY KEY,
                group_id TEXT NOT NULL REFERENCES super_groups(id) ON DELETE CASCADE,
                task_id TEXT,
                channel TEXT NOT NULL,
                target TEXT NOT NULL,
                status TEXT NOT NULL,
                preview TEXT NOT NULL,
                error TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_super_deliveries_group ON super_deliveries(group_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS super_watchdog_events (
                id TEXT PRIMARY KEY,
                group_id TEXT,
                severity TEXT NOT NULL,
                kind TEXT NOT NULL,
                summary TEXT NOT NULL,
                details TEXT NOT NULL DEFAULT '',
                resolved INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_super_watchdog_created ON super_watchdog_events(created_at DESC);",
        )?;
        Ok(())
    }
}

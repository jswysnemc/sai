use super::{decay_table, now, rebuild_fts_table, write_memory_markdown, MemoryStore};
use anyhow::Result;
use rusqlite::{params, Connection};

impl MemoryStore {
    /// 为数据库记录补齐 Markdown 文件并修复空 FTS 索引。
    ///
    /// 参数:
    /// - `self`: 记忆存储
    ///
    /// 返回:
    /// - 成功时返回空值，失败时返回存储错误
    pub(super) fn ensure_markdown_and_fts(&self) -> Result<()> {
        let conn = self.data_conn()?;
        for table in ["facts", "episodes"] {
            let sql = if table == "facts" {
                "SELECT id, content, source, status, confidence, strength, created_at, updated_at, tags FROM facts"
            } else {
                "SELECT id, content, source, status, 1.0, strength, created_at, updated_at, '' FROM episodes"
            };
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, f64>(4).unwrap_or(1.0),
                    row.get::<_, f64>(5).unwrap_or(1.0),
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8).unwrap_or_default(),
                ))
            })?;
            for row in rows {
                let (
                    id,
                    content,
                    source,
                    status,
                    confidence,
                    strength,
                    created_at,
                    updated_at,
                    tags_raw,
                ) = row?;
                let path = self.files_dir.join(table).join(format!("{id}.md"));
                if !path.is_file() {
                    write_memory_markdown(
                        &self.files_dir,
                        table,
                        id,
                        &content,
                        &source,
                        &status,
                        if table == "facts" {
                            Some(confidence)
                        } else {
                            None
                        },
                        strength,
                        &created_at,
                        &updated_at,
                        &tags_raw,
                    )?;
                }
            }
            drop(stmt);
            let base_count: i64 =
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })?;
            let fts_count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}_fts"), [], |row| {
                    row.get(0)
                })
                .unwrap_or(0);
            if base_count > 0 && fts_count == 0 {
                rebuild_fts_table(&conn, table)?;
            }
        }
        Ok(())
    }

    /// 强化被命中的记忆并更新召回统计。
    ///
    /// 参数:
    /// - `self`: 记忆存储
    /// - `id`: 记忆记录标识
    /// - `source`: 记忆来源类型
    ///
    /// 返回:
    /// - 成功时返回空值，失败时返回数据库错误
    pub(super) fn reinforce(&self, id: i64, source: &str) -> Result<()> {
        let table = if source == "episode" {
            "episodes"
        } else {
            "facts"
        };
        let sql = format!(
            "UPDATE {table} SET recall_count=recall_count+1, strength=MIN(1.0, strength+?1), last_recalled_at=?2, updated_at=?2, status='active' WHERE id=?3"
        );
        self.data_conn()?.execute(
            &sql,
            params![self.config.forgetting_review_boost, now(), id],
        )?;
        Ok(())
    }

    /// 根据遗忘配置衰减事实与经历记忆。
    ///
    /// 参数:
    /// - `self`: 记忆存储
    ///
    /// 返回:
    /// - 成功时返回空值，失败时返回数据库错误
    pub(super) fn decay_memories(&self) -> Result<()> {
        if !self.config.enabled || !self.config.forgetting_enabled {
            return Ok(());
        }
        let conn = self.data_conn()?;
        decay_table(&conn, "facts", &self.config)?;
        decay_table(&conn, "episodes", &self.config)?;
        Ok(())
    }

    /// 打开主记忆数据库连接。
    ///
    /// 参数:
    /// - `self`: 记忆存储
    ///
    /// 返回:
    /// - SQLite 连接
    pub(super) fn data_conn(&self) -> Result<Connection> {
        Ok(Connection::open(&self.data_db)?)
    }

    /// 打开记忆状态数据库连接。
    ///
    /// 参数:
    /// - `self`: 记忆存储
    ///
    /// 返回:
    /// - SQLite 连接
    pub(super) fn state_conn(&self) -> Result<Connection> {
        Ok(Connection::open(&self.state_db)?)
    }
}

use super::ConversationDb;
use crate::llm::Usage;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

/// 创建每轮用量表。
///
/// 参数:
/// - `conn`: 对话数据库连接
///
/// 返回:
/// - 表结构创建是否成功
pub(super) fn create_turn_metrics_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS turn_metrics (
            turn_id                 TEXT PRIMARY KEY,
            prompt_tokens           INTEGER NOT NULL DEFAULT 0,
            completion_tokens       INTEGER NOT NULL DEFAULT 0,
            total_tokens            INTEGER NOT NULL DEFAULT 0,
            cache_read_tokens       INTEGER NOT NULL DEFAULT 0,
            cache_write_tokens      INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY(turn_id) REFERENCES turns(turn_id) ON DELETE CASCADE
        );",
    )?;
    Ok(())
}

impl ConversationDb {
    /// 保存指定轮次汇总后的模型用量。
    ///
    /// 参数:
    /// - `turn_id`: 轮次标识
    /// - `usage`: 同一轮全部模型请求的汇总用量
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn set_turn_usage(&self, turn_id: &str, usage: &Usage) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO turn_metrics (
                turn_id, prompt_tokens, completion_tokens, total_tokens,
                cache_read_tokens, cache_write_tokens
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(turn_id) DO UPDATE SET
                prompt_tokens = excluded.prompt_tokens,
                completion_tokens = excluded.completion_tokens,
                total_tokens = excluded.total_tokens,
                cache_read_tokens = excluded.cache_read_tokens,
                cache_write_tokens = excluded.cache_write_tokens",
            params![
                turn_id,
                usage.prompt_tokens as i64,
                usage.completion_tokens as i64,
                usage.total_tokens as i64,
                usage.cache_read_tokens as i64,
                usage.cache_write_tokens as i64,
            ],
        )?;
        Ok(())
    }

    /// 读取指定轮次的模型用量。
    ///
    /// 参数:
    /// - `turn_id`: 轮次标识
    ///
    /// 返回:
    /// - 已记录的完整轮次用量
    pub(crate) fn turn_usage(&self, turn_id: &str) -> Result<Option<Usage>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT prompt_tokens, completion_tokens, total_tokens,
                    cache_read_tokens, cache_write_tokens
             FROM turn_metrics WHERE turn_id = ?1",
            params![turn_id],
            |row| {
                Ok(Usage {
                    prompt_tokens: row.get::<_, i64>(0)?.max(0) as u64,
                    completion_tokens: row.get::<_, i64>(1)?.max(0) as u64,
                    total_tokens: row.get::<_, i64>(2)?.max(0) as u64,
                    cache_read_tokens: row.get::<_, i64>(3)?.max(0) as u64,
                    cache_write_tokens: row.get::<_, i64>(4)?.max(0) as u64,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证轮次用量可覆盖写入并完整恢复缓存字段。
    #[test]
    fn persists_turn_usage() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        db.start_turn("turn-1", "hello").unwrap();
        let usage = Usage {
            prompt_tokens: 100,
            completion_tokens: 20,
            total_tokens: 120,
            cache_read_tokens: 80,
            cache_write_tokens: 5,
        };

        db.set_turn_usage("turn-1", &usage).unwrap();

        let restored = db.turn_usage("turn-1").unwrap().unwrap();
        assert_eq!(restored.prompt_tokens, 100);
        assert_eq!(restored.completion_tokens, 20);
        assert_eq!(restored.cache_read_tokens, 80);
        assert_eq!(restored.cache_write_tokens, 5);
    }
}

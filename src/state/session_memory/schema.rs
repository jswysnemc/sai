use anyhow::Result;
use rusqlite::Connection;

/// 创建 session memory 相关数据表。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 表结构初始化是否成功
pub(in crate::state) fn create_session_memory_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS session_memory (
            session_id TEXT PRIMARY KEY,
            summary TEXT NOT NULL,
            last_summarized_turn_id TEXT,
            last_summarized_seq INTEGER NOT NULL DEFAULT 0,
            checkpoint_id TEXT,
            source_turn_count INTEGER NOT NULL DEFAULT 0,
            token_estimate INTEGER NOT NULL DEFAULT 0,
            consecutive_failures INTEGER NOT NULL DEFAULT 0,
            disabled_until TEXT,
            last_error TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_session_memory_checkpoint
        ON session_memory(checkpoint_id);",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_session_memory_table() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();

        create_session_memory_tables(&conn).unwrap();

        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'session_memory'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1);
    }
}

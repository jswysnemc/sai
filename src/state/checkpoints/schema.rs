use anyhow::Result;
use rusqlite::Connection;

/// 创建 checkpoint 相关数据表。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 表结构初始化是否成功
pub(in crate::state) fn create_checkpoint_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS compaction_checkpoints (
            id TEXT PRIMARY KEY,
            seq INTEGER NOT NULL UNIQUE,
            compacted_from_seq INTEGER NOT NULL,
            compacted_to_seq INTEGER NOT NULL,
            summary TEXT NOT NULL,
            recent TEXT NOT NULL,
            source_turn_count INTEGER NOT NULL,
            reason TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_compaction_checkpoints_to_seq
        ON compaction_checkpoints(compacted_to_seq);",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_checkpoint_table() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();

        create_checkpoint_tables(&conn).unwrap();

        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'compaction_checkpoints'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1);
    }
}

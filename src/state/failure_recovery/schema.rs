use anyhow::Result;
use rusqlite::Connection;

/// 创建 failure recovery 相关数据表。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 表结构初始化是否成功
pub(in crate::state) fn create_failure_recovery_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS failure_recovery_records (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            turn_id TEXT,
            kind TEXT NOT NULL,
            status TEXT NOT NULL,
            reason TEXT NOT NULL,
            retry_count INTEGER NOT NULL DEFAULT 0,
            checkpoint_id TEXT,
            context_chars INTEGER NOT NULL DEFAULT 0,
            context_limit_chars INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            resolved_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_failure_recovery_session_status
        ON failure_recovery_records(session_id, status, created_at);
        CREATE INDEX IF NOT EXISTS idx_failure_recovery_session_kind
        ON failure_recovery_records(session_id, kind, created_at);",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_failure_recovery_table_and_indexes() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();

        create_failure_recovery_tables(&conn).unwrap();

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'failure_recovery_records'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let index_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_failure_recovery_session_status'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(table_count, 1);
        assert_eq!(index_count, 1);
    }
}

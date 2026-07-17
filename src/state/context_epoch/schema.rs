use anyhow::Result;
use rusqlite::Connection;

/// 创建 Context Epoch 相关数据表。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 表结构初始化是否成功
pub(in crate::state) fn create_context_epoch_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS context_epochs (
            session_id         TEXT PRIMARY KEY,
            baseline           TEXT NOT NULL,
            baseline_hash      TEXT NOT NULL,
            snapshot_json      TEXT NOT NULL,
            source_count       INTEGER NOT NULL DEFAULT 0,
            last_change_reason TEXT NOT NULL,
            blocked_source     TEXT,
            created_at         TEXT NOT NULL,
            updated_at         TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS context_epoch_events (
            id         TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            seq        INTEGER NOT NULL,
            source_key TEXT NOT NULL,
            kind       TEXT NOT NULL,
            text       TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_context_epoch_events_session_seq
        ON context_epoch_events(session_id, seq);",
    )?;
    ensure_blocked_source_column(conn)?;
    Ok(())
}

/// 确保旧数据库包含 blocked source 列。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 表结构补齐是否成功
fn ensure_blocked_source_column(conn: &Connection) -> Result<()> {
    let column_count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM pragma_table_info('context_epochs')
         WHERE name = 'blocked_source'",
        [],
        |row| row.get(0),
    )?;
    if column_count == 0 {
        conn.execute(
            "ALTER TABLE context_epochs ADD COLUMN blocked_source TEXT",
            [],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_context_epoch_tables() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();

        create_context_epoch_tables(&conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                 AND name IN ('context_epochs', 'context_epoch_events')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }
}

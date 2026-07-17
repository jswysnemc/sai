use anyhow::Result;
use rusqlite::Connection;

/// 创建工具历史相关数据表。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 建表是否成功
pub(in crate::state) fn create_tool_history_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tool_calls (
            id               TEXT PRIMARY KEY,
            session_id       TEXT NOT NULL,
            turn_id          TEXT NOT NULL,
            seq              INTEGER NOT NULL,
            provider_call_id TEXT NOT NULL,
            tool_name        TEXT NOT NULL,
            arguments        TEXT NOT NULL,
            status           TEXT NOT NULL,
            created_at       TEXT NOT NULL,
            updated_at       TEXT NOT NULL,
            UNIQUE(session_id, provider_call_id)
        );
        CREATE INDEX IF NOT EXISTS idx_tool_calls_session_turn
            ON tool_calls(session_id, turn_id, seq);
        CREATE INDEX IF NOT EXISTS idx_tool_calls_status
            ON tool_calls(session_id, status);

        CREATE TABLE IF NOT EXISTS tool_results (
            id               TEXT PRIMARY KEY,
            session_id       TEXT NOT NULL,
            turn_id          TEXT NOT NULL,
            provider_call_id TEXT NOT NULL,
            ok               INTEGER NOT NULL,
            result_preview   TEXT NOT NULL,
            result_ref       TEXT,
            error            TEXT,
            original_chars   INTEGER NOT NULL DEFAULT 0,
            created_at       TEXT NOT NULL,
            completed_at     TEXT NOT NULL,
            UNIQUE(session_id, provider_call_id)
        );
        CREATE INDEX IF NOT EXISTS idx_tool_results_session_turn
            ON tool_results(session_id, turn_id);

        CREATE TABLE IF NOT EXISTS tool_output_replacements (
            provider_call_id TEXT PRIMARY KEY,
            session_id       TEXT NOT NULL,
            replacement      TEXT NOT NULL,
            original_chars   INTEGER NOT NULL,
            result_ref       TEXT NOT NULL,
            policy           TEXT NOT NULL,
            created_at       TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_tool_replacements_session
            ON tool_output_replacements(session_id);",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_tool_history_tables() {
        let conn = Connection::open_in_memory().unwrap();

        create_tool_history_tables(&conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                 AND name IN ('tool_calls', 'tool_results', 'tool_output_replacements')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);
    }
}

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
            assistant_round  INTEGER NOT NULL DEFAULT 0,
            assistant_reasoning TEXT,
            provider_call_id TEXT NOT NULL,
            tool_name        TEXT NOT NULL,
            arguments        TEXT NOT NULL,
            display_tool_name TEXT,
            display_arguments TEXT,
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
    ensure_tool_call_metadata_columns(conn)?;
    Ok(())
}

/// 为旧版工具调用表补齐工具子轮上下文字段。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 数据库迁移是否成功
fn ensure_tool_call_metadata_columns(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(tool_calls)")?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    // 1. 子轮编号用于按原始 assistant 消息分组工具调用
    if !columns.iter().any(|column| column == "assistant_round") {
        conn.execute(
            "ALTER TABLE tool_calls ADD COLUMN assistant_round INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    // 2. 思考内容用于 DeepSeek 工具子轮恢复
    if !columns.iter().any(|column| column == "assistant_reasoning") {
        conn.execute(
            "ALTER TABLE tool_calls ADD COLUMN assistant_reasoning TEXT",
            [],
        )?;
    }
    // 3. 展示字段保存统一网关解包后的真实工具信息，不影响供应商消息投影
    if !columns.iter().any(|column| column == "display_tool_name") {
        conn.execute(
            "ALTER TABLE tool_calls ADD COLUMN display_tool_name TEXT",
            [],
        )?;
    }
    if !columns.iter().any(|column| column == "display_arguments") {
        conn.execute(
            "ALTER TABLE tool_calls ADD COLUMN display_arguments TEXT",
            [],
        )?;
    }
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

    /// 【会话历史】【工具子轮】验证旧数据库会补齐工具子轮上下文字段。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn migrates_tool_call_reasoning_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE tool_calls (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                turn_id TEXT NOT NULL,
                seq INTEGER NOT NULL,
                provider_call_id TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                arguments TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(session_id, provider_call_id)
            );",
        )
        .unwrap();

        create_tool_history_tables(&conn).unwrap();

        let mut stmt = conn.prepare("PRAGMA table_info(tool_calls)").unwrap();
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert!(columns.iter().any(|column| column == "assistant_round"));
        assert!(columns.iter().any(|column| column == "assistant_reasoning"));
        assert!(columns.iter().any(|column| column == "display_tool_name"));
        assert!(columns.iter().any(|column| column == "display_arguments"));
    }
}

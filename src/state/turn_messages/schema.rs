use anyhow::Result;
use rusqlite::Connection;

/// 创建轮次内消息表。
///
/// 参数:
/// - `conn`: 对话数据库连接
///
/// 返回:
/// - 建表是否成功
pub(in crate::state) fn create_turn_message_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS turn_messages (
            id               TEXT PRIMARY KEY,
            turn_id          TEXT NOT NULL,
            seq              INTEGER NOT NULL,
            after_tool_seq   INTEGER NOT NULL DEFAULT 0,
            kind             TEXT NOT NULL,
            model_content    TEXT NOT NULL,
            display_content  TEXT NOT NULL,
            reasoning        TEXT,
            image_urls       TEXT NOT NULL DEFAULT '[]',
            created_at       TEXT NOT NULL,
            UNIQUE(turn_id, seq),
            FOREIGN KEY(turn_id) REFERENCES turns(turn_id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_turn_messages_turn_seq
            ON turn_messages(turn_id, seq);",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【会话历史】【消息间隙】验证轮次内消息表包含排序与展示字段。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn creates_turn_message_table() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE turns (turn_id TEXT PRIMARY KEY)", [])
            .unwrap();

        create_turn_message_table(&conn).unwrap();

        let columns = conn
            .prepare("PRAGMA table_info(turn_messages)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert!(columns.iter().any(|column| column == "after_tool_seq"));
        assert!(columns.iter().any(|column| column == "display_content"));
    }
}

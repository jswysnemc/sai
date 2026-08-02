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
    ensure_running_turn_boundary_columns(conn)?;
    Ok(())
}

/// 为旧版 checkpoint 表补齐运行中轮次压缩边界字段。
///
/// 轮次内压缩需要记录"当前运行轮次的工具调用已被摘要覆盖到第几条"，
/// 否则重建上下文时无法区分哪些工具消息已经进了摘要。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 数据库迁移是否成功
fn ensure_running_turn_boundary_columns(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(compaction_checkpoints)")?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    // 1. 被摘要覆盖的运行中轮次标识；为空表示本次压缩只涉及已完成轮次
    if !columns.iter().any(|column| column == "running_turn_id") {
        conn.execute(
            "ALTER TABLE compaction_checkpoints ADD COLUMN running_turn_id TEXT",
            [],
        )?;
    }
    // 2. 该轮次中已被摘要覆盖的工具调用条数，重建时跳过这些工具消息
    if !columns
        .iter()
        .any(|column| column == "running_turn_compacted_calls")
    {
        conn.execute(
            "ALTER TABLE compaction_checkpoints ADD COLUMN running_turn_compacted_calls INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
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

    /// 验证旧库补齐运行中轮次边界字段且可重复执行。
    #[test]
    fn migrates_running_turn_boundary_columns() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        // 1. 先建出不含新列的旧表
        conn.execute_batch(
            "CREATE TABLE compaction_checkpoints (
                id TEXT PRIMARY KEY,
                seq INTEGER NOT NULL UNIQUE,
                compacted_from_seq INTEGER NOT NULL,
                compacted_to_seq INTEGER NOT NULL,
                summary TEXT NOT NULL,
                recent TEXT NOT NULL,
                source_turn_count INTEGER NOT NULL,
                reason TEXT NOT NULL,
                created_at TEXT NOT NULL
            );",
        )
        .unwrap();

        create_checkpoint_tables(&conn).unwrap();
        // 2. 重复执行不得报错
        create_checkpoint_tables(&conn).unwrap();

        let mut stmt = conn.prepare("PRAGMA table_info(compaction_checkpoints)").unwrap();
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert!(columns.iter().any(|column| column == "running_turn_id"));
        assert!(columns
            .iter()
            .any(|column| column == "running_turn_compacted_calls"));
    }
}

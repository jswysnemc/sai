use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

/// 【上下文】【局部压缩】建立独立归档表，原文不随普通轮次删除
/// 参数: conn 为会话数据库；返回建表结果
pub(in crate::state) fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS context_blocks (
            block_id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            request_hash TEXT NOT NULL,
            body TEXT NOT NULL,
            UNIQUE(session_id, request_hash)
         );
         CREATE TABLE IF NOT EXISTS context_block_messages (
            session_id TEXT NOT NULL,
            message_id TEXT NOT NULL,
            block_id TEXT NOT NULL,
            turn_id TEXT NOT NULL,
            tool TEXT NOT NULL,
            arguments TEXT NOT NULL,
            original TEXT NOT NULL,
            checkpoint_id TEXT,
            PRIMARY KEY(session_id, message_id)
         );
         CREATE INDEX IF NOT EXISTS idx_context_block_members
            ON context_block_messages(session_id, block_id);
         CREATE TABLE IF NOT EXISTS context_block_revision (
            session_id TEXT PRIMARY KEY,
            revision INTEGER NOT NULL
         );",
    )?;
    Ok(())
}

/// 【上下文】【局部压缩】读取只随摘要提交变化的修订号
/// 参数: conn 为连接，session_id 为会话；返回修订号，首次为零
pub(super) fn revision(conn: &Connection, session_id: &str) -> Result<u64> {
    Ok(conn
        .query_row(
            "SELECT revision FROM context_block_revision WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

/// 【上下文】【局部压缩】读取分支叶子，用于提交前拒绝已经切换的分支
/// 参数: conn 为连接；返回当前叶子标识
pub(super) fn leaf(conn: &Connection) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM session_tree_meta WHERE key = 'active_leaf'",
            [],
            |row| row.get(0),
        )
        .optional()?
        .flatten())
}

/// 【上下文】【全局压缩】在删除轮次的同一事务中续接归档的 checkpoint 归属
/// 参数: conn 为事务，turn_ids 为本次覆盖轮次，previous/current 为摘要标识；返回更新结果
pub(in crate::state) fn carry_checkpoint(
    conn: &Connection,
    turn_ids: &[String],
    previous: Option<&str>,
    current: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE context_block_messages SET checkpoint_id = ?1
         WHERE turn_id IN (SELECT value FROM json_each(?2)) OR checkpoint_id = ?3",
        params![current, serde_json::to_string(turn_ids)?, previous],
    )?;
    Ok(())
}

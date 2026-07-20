use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

/// 打开并初始化对话 SQLite 数据库。
///
/// 参数:
/// - `state_dir`: 状态目录
///
/// 返回:
/// - 已初始化的数据库连接
pub(super) fn open_connection(state_dir: &Path) -> Result<Connection> {
    std::fs::create_dir_all(state_dir)?;
    let db_path = state_dir.join("conversation.db");
    let conn = Connection::open(&db_path)
        .with_context(|| format!("failed to open conversation db: {}", db_path.display()))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA foreign_keys = ON;",
    )?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS turns (
            turn_id             TEXT PRIMARY KEY,
            seq                 INTEGER NOT NULL UNIQUE,
            user_content        TEXT NOT NULL,
            user_timestamp      TEXT NOT NULL,
            assistant_content   TEXT NOT NULL,
            assistant_reasoning TEXT,
            assistant_timestamp TEXT,
            status              TEXT NOT NULL DEFAULT 'running',
            tool_reports        TEXT NOT NULL DEFAULT '[]',
            user_image_urls     TEXT NOT NULL DEFAULT '[]'
        );
        CREATE INDEX IF NOT EXISTS idx_turns_seq ON turns(seq);
        CREATE INDEX IF NOT EXISTS idx_turns_status ON turns(status);",
    )?;
    crate::state::checkpoints::schema::create_checkpoint_tables(&conn)?;
    crate::state::context_epoch::schema::create_context_epoch_tables(&conn)?;
    crate::state::failure_recovery::schema::create_failure_recovery_tables(&conn)?;
    crate::state::session_memory::schema::create_session_memory_tables(&conn)?;
    crate::state::tool_history::schema::create_tool_history_tables(&conn)?;
    crate::runtime_recovery::schema::create_runtime_recovery_tables(&conn)?;
    ensure_user_image_urls_column(&conn)?;
    conn.execute_batch(
        "UPDATE turns
         SET assistant_content = '', assistant_reasoning = NULL
         WHERE status = 'interrupted'
           AND assistant_content IN (
             '此轮响应正在由另一条对话线处理...',
             '此轮响应被中断，但是除非用户重新要求否则不要重新执行此轮对话。'
           );",
    )?;
    Ok(conn)
}

/// 确保 turns 表包含用户图片列。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 表结构补齐是否成功
fn ensure_user_image_urls_column(conn: &Connection) -> Result<()> {
    let column_count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM pragma_table_info('turns')
         WHERE name = 'user_image_urls'",
        [],
        |row| row.get(0),
    )?;
    if column_count == 0 {
        conn.execute(
            "ALTER TABLE turns ADD COLUMN user_image_urls TEXT NOT NULL DEFAULT '[]'",
            [],
        )?;
    }
    Ok(())
}

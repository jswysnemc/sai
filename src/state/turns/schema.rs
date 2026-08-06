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
            provider_user_content TEXT,
            user_timestamp      TEXT NOT NULL,
            assistant_content   TEXT NOT NULL,
            assistant_reasoning TEXT,
            assistant_timestamp TEXT,
            status              TEXT NOT NULL DEFAULT 'running',
            tool_reports        TEXT NOT NULL DEFAULT '[]',
            user_image_urls     TEXT NOT NULL DEFAULT '[]',
            duration_ms         INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_turns_seq ON turns(seq);
        CREATE INDEX IF NOT EXISTS idx_turns_status ON turns(status);",
    )?;
    crate::state::checkpoints::schema::create_checkpoint_tables(&conn)?;
    crate::state::context_epoch::schema::create_context_epoch_tables(&conn)?;
    crate::state::failure_recovery::schema::create_failure_recovery_tables(&conn)?;
    crate::state::session_memory::schema::create_session_memory_tables(&conn)?;
    crate::state::tool_history::schema::create_tool_history_tables(&conn)?;
    crate::state::turn_messages::schema::create_turn_message_table(&conn)?;
    crate::runtime_recovery::schema::create_runtime_recovery_tables(&conn)?;
    ensure_user_image_urls_column(&conn)?;
    ensure_duration_ms_column(&conn)?;
    ensure_parent_turn_id_column(&conn)?;
    ensure_provider_user_content_column(&conn)?;
    backfill_linear_parents(&conn)?;
    create_tree_meta_table(&conn)?;
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

/// 确保 turns 表包含供应商用户消息列。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 表结构补齐是否成功
fn ensure_provider_user_content_column(conn: &Connection) -> Result<()> {
    let column_count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM pragma_table_info('turns')
         WHERE name = 'provider_user_content'",
        [],
        |row| row.get(0),
    )?;
    if column_count == 0 {
        conn.execute(
            "ALTER TABLE turns ADD COLUMN provider_user_content TEXT",
            [],
        )?;
    }
    Ok(())
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

/// 确保 turns 表包含处理耗时列。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 表结构补齐是否成功
fn ensure_duration_ms_column(conn: &Connection) -> Result<()> {
    let column_count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM pragma_table_info('turns')
         WHERE name = 'duration_ms'",
        [],
        |row| row.get(0),
    )?;
    if column_count == 0 {
        conn.execute(
            "ALTER TABLE turns ADD COLUMN duration_ms INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    Ok(())
}

/// 确保 turns 表包含父轮次列。
///
/// 会话从线性历史升级为分支树：每一轮记录它的前驱轮次，
/// 同一个父轮次下出现多个子轮次即构成分叉。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 表结构补齐是否成功
fn ensure_parent_turn_id_column(conn: &Connection) -> Result<()> {
    let column_count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM pragma_table_info('turns')
         WHERE name = 'parent_turn_id'",
        [],
        |row| row.get(0),
    )?;
    if column_count == 0 {
        conn.execute("ALTER TABLE turns ADD COLUMN parent_turn_id TEXT", [])?;
    }
    conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_turns_parent ON turns(parent_turn_id);")?;
    Ok(())
}

/// 为升级前的历史轮次补齐线性父子关系。
///
/// 旧会话按 seq 排成一条链，升级后每一轮的父轮次即 seq 紧邻的前一轮，
/// 首轮父为空。只处理 parent_turn_id 为空的行，重复执行不改变结果。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 回填是否成功
fn backfill_linear_parents(conn: &Connection) -> Result<()> {
    conn.execute(
        "UPDATE turns
         SET parent_turn_id = (
             SELECT previous.turn_id
             FROM turns AS previous
             WHERE previous.seq < turns.seq
             ORDER BY previous.seq DESC
             LIMIT 1
         )
         WHERE parent_turn_id IS NULL
           AND seq > (SELECT MIN(seq) FROM turns)",
        [],
    )?;
    Ok(())
}

/// 创建会话树元数据表。
///
/// 目前只存活动叶子轮次：树上可以有多条分支，需要一个指针标明
/// 当前对话处在哪一条上。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 表结构初始化是否成功
fn create_tree_meta_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS session_tree_meta (
            key   TEXT PRIMARY KEY,
            value TEXT
        );",
    )?;
    Ok(())
}

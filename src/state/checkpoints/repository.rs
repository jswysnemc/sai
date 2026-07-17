use super::model::{CheckpointReason, CompactionCheckpoint};
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use crate::state::turns::ConversationDb;

/// 写入 checkpoint。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `checkpoint`: 待写入 checkpoint
///
/// 返回:
/// - 写入是否成功
pub(in crate::state) fn insert_checkpoint(
    conn: &Connection,
    checkpoint: &CompactionCheckpoint,
) -> Result<()> {
    conn.execute(
        "INSERT INTO compaction_checkpoints (
            id, seq, compacted_from_seq, compacted_to_seq, summary,
            recent, source_turn_count, reason, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            checkpoint.id,
            checkpoint.seq,
            checkpoint.compacted_from_seq,
            checkpoint.compacted_to_seq,
            checkpoint.summary,
            checkpoint.recent,
            checkpoint.source_turn_count as i64,
            reason_to_str(&checkpoint.reason),
            checkpoint.created_at,
        ],
    )?;
    Ok(())
}

/// 读取最近 checkpoint。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 最近 checkpoint
pub(in crate::state) fn load_latest_checkpoint(
    conn: &Connection,
) -> Result<Option<CompactionCheckpoint>> {
    conn.query_row(
        "SELECT id, seq, compacted_from_seq, compacted_to_seq, summary,
                recent, source_turn_count, reason, created_at
         FROM compaction_checkpoints ORDER BY seq DESC LIMIT 1",
        [],
        |row| {
            Ok(CompactionCheckpoint {
                id: row.get(0)?,
                seq: row.get(1)?,
                compacted_from_seq: row.get(2)?,
                compacted_to_seq: row.get(3)?,
                summary: row.get(4)?,
                recent: row.get(5)?,
                source_turn_count: row.get::<_, i64>(6)? as usize,
                reason: reason_from_str(&row.get::<_, String>(7)?),
                created_at: row.get(8)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

/// 统计 checkpoint 数量。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - checkpoint 数量
pub(in crate::state) fn count_checkpoints(conn: &Connection) -> Result<usize> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM compaction_checkpoints", [], |row| {
        row.get(0)
    })?;
    Ok(count as usize)
}

/// 写入压缩 checkpoint 并删除被覆盖 turns。
///
/// 参数:
/// - `db`: 对话数据库
/// - `request`: 压缩请求
/// - `summary`: 压缩摘要
/// - `source_turn_count`: checkpoint 累计覆盖轮次数
/// - `reason`: 压缩原因
///
/// 返回:
/// - 写入后的 checkpoint
pub(in crate::state) fn apply_checkpoint_compaction(
    db: &ConversationDb,
    request: &crate::state::CompactionRequest,
    summary: &str,
    source_turn_count: usize,
    reason: CheckpointReason,
) -> Result<CompactionCheckpoint> {
    let (from_seq, to_seq) = request
        .seq_range()
        .ok_or_else(|| anyhow::anyhow!("compaction request has no turns"))?;
    let now = Utc::now().to_rfc3339();
    let checkpoint = CompactionCheckpoint {
        id: format!(
            "cp_{}_{}",
            Utc::now().timestamp_millis(),
            rand::random::<u16>()
        ),
        seq: to_seq,
        compacted_from_seq: from_seq,
        compacted_to_seq: to_seq,
        summary: summary.trim().to_string(),
        recent: request.recent_context(),
        source_turn_count,
        reason,
        created_at: now,
    };
    let mut conn = db.conn.lock().unwrap();
    let tx = conn.transaction()?;
    insert_checkpoint(&tx, &checkpoint)?;
    for turn_id in &request.compact_turn_ids {
        tx.execute(
            "DELETE FROM turns WHERE turn_id = ?1 AND status != 'running'",
            params![turn_id],
        )?;
    }
    tx.commit()?;
    Ok(checkpoint)
}

/// 写入旧摘要迁移 checkpoint 并清理已覆盖原始轮次。
///
/// 参数:
/// - `db`: 对话数据库
/// - `checkpoint`: 旧摘要迁移生成的 checkpoint
/// - `delete_to_seq`: 需要清理的最大原始轮次 seq
///
/// 返回:
/// - 写入是否成功
pub(in crate::state) fn apply_legacy_checkpoint_migration(
    db: &ConversationDb,
    checkpoint: &CompactionCheckpoint,
    delete_to_seq: i64,
) -> Result<()> {
    let mut conn = db.conn.lock().unwrap();
    let tx = conn.transaction()?;
    insert_checkpoint(&tx, checkpoint)?;
    if delete_to_seq > 0 {
        tx.execute(
            "DELETE FROM turns WHERE seq <= ?1 AND status != 'running'",
            params![delete_to_seq],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// 转换 checkpoint 原因为数据库文本。
///
/// 参数:
/// - `reason`: checkpoint 原因
///
/// 返回:
/// - 数据库文本
fn reason_to_str(reason: &CheckpointReason) -> &'static str {
    match reason {
        CheckpointReason::Auto => "auto",
        CheckpointReason::Manual => "manual",
        CheckpointReason::Legacy => "legacy",
    }
}

/// 从数据库文本恢复 checkpoint 原因。
///
/// 参数:
/// - `value`: 数据库文本
///
/// 返回:
/// - checkpoint 原因
fn reason_from_str(value: &str) -> CheckpointReason {
    match value {
        "manual" => CheckpointReason::Manual,
        "legacy" => CheckpointReason::Legacy,
        _ => CheckpointReason::Auto,
    }
}

#[cfg(test)]
mod tests {
    use super::{insert_checkpoint, load_latest_checkpoint};
    use crate::state::checkpoints::schema::create_checkpoint_tables;
    use crate::state::checkpoints::{CheckpointReason, CompactionCheckpoint};

    #[test]
    fn inserts_and_loads_latest_checkpoint() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        create_checkpoint_tables(&conn).unwrap();
        let checkpoint = CompactionCheckpoint {
            id: "cp_1".to_string(),
            seq: 10,
            compacted_from_seq: 1,
            compacted_to_seq: 4,
            summary: "summary".to_string(),
            recent: "recent".to_string(),
            source_turn_count: 4,
            reason: CheckpointReason::Auto,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        };

        insert_checkpoint(&conn, &checkpoint).unwrap();
        let latest = load_latest_checkpoint(&conn).unwrap().unwrap();

        assert_eq!(latest.id, "cp_1");
        assert_eq!(latest.compacted_to_seq, 4);
        assert_eq!(latest.reason, CheckpointReason::Auto);
    }
}

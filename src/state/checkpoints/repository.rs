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
            recent, source_turn_count, reason, created_at,
            running_turn_id, running_turn_compacted_calls
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
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
            checkpoint.running_turn_id,
            checkpoint.running_turn_compacted_calls as i64,
        ],
    )?;
    Ok(())
}

/// 写入 checkpoint，同一运行轮次的重复压缩覆盖上一条。
///
/// 轮次内压缩会在已完成轮次范围不变的情况下反复写入。若上一条 checkpoint 覆盖的
/// 正是同一个运行中轮次，说明这是该轮次的又一次压缩，应当覆盖而非堆叠记录。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `checkpoint`: 待写入 checkpoint
///
/// 返回:
/// - 写入是否成功
pub(in crate::state) fn upsert_checkpoint(
    conn: &Connection,
    checkpoint: &CompactionCheckpoint,
) -> Result<()> {
    // 同一运行轮次的连续压缩只保留一条记录，避免 checkpoint 无限堆叠
    let replaces_previous = checkpoint.running_turn_id.is_some()
        && load_latest_checkpoint(conn)?
            .and_then(|previous| previous.running_turn_id)
            .as_deref()
            == checkpoint.running_turn_id.as_deref();
    if replaces_previous {
        conn.execute(
            "DELETE FROM compaction_checkpoints
             WHERE rowid = (SELECT rowid FROM compaction_checkpoints ORDER BY rowid DESC LIMIT 1)",
            [],
        )?;
    }
    // seq 冲突仍可能出现：压缩清空轮次后 seq 会从头计数
    conn.execute(
        "DELETE FROM compaction_checkpoints WHERE seq = ?1",
        params![checkpoint.seq],
    )?;
    insert_checkpoint(conn, checkpoint)
}

/// 读取最近 checkpoint。
///
/// 按 rowid 而非 seq 排序：压缩删光全部轮次后 turn seq 会从 1 重新开始，
/// 新 checkpoint 的 seq 可能小于旧的，按 seq 取会返回过期记录。
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
                recent, source_turn_count, reason, created_at,
                running_turn_id, running_turn_compacted_calls
         FROM compaction_checkpoints ORDER BY rowid DESC LIMIT 1",
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
                running_turn_id: row.get(9)?,
                running_turn_compacted_calls: row.get::<_, i64>(10)? as usize,
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
    // 只压缩运行中轮次时没有已完成轮次范围，seq 沿用上一个 checkpoint 的边界
    let (from_seq, to_seq) = match request.seq_range() {
        Some(range) => range,
        None => {
            let conn = db.conn.lock().unwrap();
            let previous_seq = load_latest_checkpoint(&conn)?
                .map(|checkpoint| checkpoint.compacted_to_seq)
                .unwrap_or_default();
            drop(conn);
            (previous_seq, previous_seq)
        }
    };
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
        running_turn_id: request
            .running_turn
            .as_ref()
            .map(|running| running.turn_id.clone()),
        running_turn_compacted_calls: request
            .running_turn
            .as_ref()
            .map(|running| running.compacted_calls)
            .unwrap_or_default(),
    };
    let mut conn = db.conn.lock().unwrap();
    let tx = conn.transaction()?;
    // 只压缩运行中轮次时 seq 与上一个 checkpoint 相同，覆盖而非新增
    upsert_checkpoint(&tx, &checkpoint)?;
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
    use super::{count_checkpoints, insert_checkpoint, load_latest_checkpoint, upsert_checkpoint};
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
            running_turn_id: None,
            running_turn_compacted_calls: 0,
        };

        insert_checkpoint(&conn, &checkpoint).unwrap();
        let latest = load_latest_checkpoint(&conn).unwrap().unwrap();

        assert_eq!(latest.id, "cp_1");
        assert_eq!(latest.compacted_to_seq, 4);
        assert_eq!(latest.reason, CheckpointReason::Auto);
    }

    /// 验证同一 seq 的 checkpoint 被覆盖而不是插入新行。
    ///
    /// 轮次内压缩会在已完成轮次范围不变的情况下反复写入。
    #[test]
    fn upsert_replaces_checkpoint_on_same_seq() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        create_checkpoint_tables(&conn).unwrap();
        let first = CompactionCheckpoint {
            id: "cp_1".to_string(),
            seq: 10,
            compacted_from_seq: 1,
            compacted_to_seq: 4,
            summary: "first".to_string(),
            recent: "recent".to_string(),
            source_turn_count: 4,
            reason: CheckpointReason::Auto,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            running_turn_id: Some("turn_5".to_string()),
            running_turn_compacted_calls: 10,
        };
        upsert_checkpoint(&conn, &first).unwrap();

        let second = CompactionCheckpoint {
            id: "cp_2".to_string(),
            summary: "second".to_string(),
            running_turn_compacted_calls: 25,
            ..first.clone()
        };
        upsert_checkpoint(&conn, &second).unwrap();

        assert_eq!(count_checkpoints(&conn).unwrap(), 1);
        let latest = load_latest_checkpoint(&conn).unwrap().unwrap();
        assert_eq!(latest.id, "cp_2");
        assert_eq!(latest.summary, "second");
        assert_eq!(latest.running_turn_id.as_deref(), Some("turn_5"));
        assert_eq!(latest.running_turn_compacted_calls, 25);
    }
}

use super::model::{
    reason_from_str, reason_to_str, ContextChangeReason, ContextEpoch, ContextEpochSummary,
};
use super::snapshot;
use crate::state::turns::ConversationDb;
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};

pub(crate) struct PreparedEpoch {
    pub baseline: String,
    pub baseline_hash: String,
    pub snapshot_json: String,
    pub source_count: usize,
    pub blocked_source: Option<String>,
}

/// 准备并持久化 Context Epoch。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 当前会话标识
/// - `prepared`: 当前 source 生成的 baseline
///
/// 返回:
/// - 最新 Context Epoch
pub(crate) fn prepare_epoch(
    db: &ConversationDb,
    session_id: &str,
    prepared: PreparedEpoch,
) -> Result<ContextEpoch> {
    let existing = load_epoch(db, session_id)?;
    if let Some(epoch) = &existing {
        snapshot::validate_snapshot_json(&epoch.snapshot_json)
            .context("invalid Context Epoch snapshot")?;
    }
    match existing {
        Some(epoch)
            if epoch.snapshot_json == prepared.snapshot_json
                && epoch.blocked_source == prepared.blocked_source =>
        {
            Ok(epoch)
        }
        Some(epoch) if epoch.snapshot_json == prepared.snapshot_json => {
            upsert_epoch(db, session_id, prepared, epoch.last_change_reason)
        }
        Some(_) => upsert_epoch(
            db,
            session_id,
            prepared,
            ContextChangeReason::StableSourceChanged,
        ),
        None => upsert_epoch(db, session_id, prepared, ContextChangeReason::Initialized),
    }
}

/// 标记 Context Epoch 存在不可用 source。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 当前会话标识
/// - `blocked_source`: 不可用 source 描述
///
/// 返回:
/// - 复用旧 baseline 后的 Context Epoch
pub(crate) fn mark_blocked_source(
    db: &ConversationDb,
    session_id: &str,
    blocked_source: String,
) -> Result<Option<ContextEpoch>> {
    let Some(mut epoch) = load_epoch(db, session_id)? else {
        return Ok(None);
    };
    let now = Utc::now().to_rfc3339();
    let conn = db.conn.lock().unwrap();
    conn.execute(
        "UPDATE context_epochs
         SET blocked_source = ?1, updated_at = ?2
         WHERE session_id = ?3",
        params![blocked_source, now, session_id],
    )?;
    epoch.blocked_source = Some(blocked_source);
    epoch.updated_at = now;
    Ok(Some(epoch))
}

/// 读取 Context Epoch 摘要。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 当前会话标识
///
/// 返回:
/// - Context Epoch 摘要
pub(crate) fn load_summary(
    db: &ConversationDb,
    session_id: &str,
) -> Result<Option<ContextEpochSummary>> {
    let conn = db.conn.lock().unwrap();
    conn.query_row(
        "SELECT baseline_hash, source_count, last_change_reason, blocked_source
         FROM context_epochs WHERE session_id = ?1",
        params![session_id],
        |row| {
            Ok(ContextEpochSummary {
                baseline_hash: row.get(0)?,
                source_count: row.get::<_, i64>(1)? as usize,
                last_change_reason: row.get(2)?,
                blocked_source: row.get(3)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

/// 读取完整 Context Epoch。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 当前会话标识
///
/// 返回:
/// - Context Epoch
fn load_epoch(db: &ConversationDb, session_id: &str) -> Result<Option<ContextEpoch>> {
    let conn = db.conn.lock().unwrap();
    conn.query_row(
        "SELECT session_id, baseline, baseline_hash, snapshot_json,
                source_count, last_change_reason, blocked_source, created_at, updated_at
         FROM context_epochs WHERE session_id = ?1",
        params![session_id],
        |row| {
            let reason = row.get::<_, String>(5)?;
            Ok(ContextEpoch {
                session_id: row.get(0)?,
                baseline: row.get(1)?,
                baseline_hash: row.get(2)?,
                snapshot_json: row.get(3)?,
                source_count: row.get::<_, i64>(4)? as usize,
                last_change_reason: reason_from_str(&reason),
                blocked_source: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

/// 写入或更新 Context Epoch。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 当前会话标识
/// - `prepared`: 当前 source 生成的 baseline
/// - `reason`: 变更原因
///
/// 返回:
/// - 最新 Context Epoch
fn upsert_epoch(
    db: &ConversationDb,
    session_id: &str,
    prepared: PreparedEpoch,
    reason: ContextChangeReason,
) -> Result<ContextEpoch> {
    let now = Utc::now().to_rfc3339();
    let reason_text = reason_to_str(&reason);
    let conn = db.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO context_epochs (
            session_id, baseline, baseline_hash, snapshot_json,
            source_count, last_change_reason, blocked_source, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
         ON CONFLICT(session_id) DO UPDATE SET
            baseline = excluded.baseline,
            baseline_hash = excluded.baseline_hash,
            snapshot_json = excluded.snapshot_json,
            source_count = excluded.source_count,
            last_change_reason = excluded.last_change_reason,
            blocked_source = excluded.blocked_source,
            updated_at = excluded.updated_at",
        params![
            session_id,
            prepared.baseline,
            prepared.baseline_hash,
            prepared.snapshot_json,
            prepared.source_count as i64,
            reason_text,
            prepared.blocked_source,
            now,
        ],
    )?;
    Ok(ContextEpoch {
        session_id: session_id.to_string(),
        baseline: prepared.baseline,
        baseline_hash: prepared.baseline_hash,
        snapshot_json: prepared.snapshot_json,
        source_count: prepared.source_count,
        last_change_reason: reason,
        blocked_source: prepared.blocked_source,
        created_at: now.clone(),
        updated_at: now,
    })
}

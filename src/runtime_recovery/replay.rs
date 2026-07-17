use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection};

struct ReplayLogSource {
    stream: String,
    path: String,
}

/// 尝试从已保留日志读取缺失事件的 replay 内容。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `process_id`: 运行时进程标识
/// - `missing_seq`: 缺失事件序号
///
/// 返回:
/// - 是否写入了 replay 事件
pub(super) fn try_insert_log_tail_replay_locked(
    conn: &Connection,
    process_id: &str,
    missing_seq: i64,
) -> Result<bool> {
    let Some(source) = load_nearest_log_source_locked(conn, process_id, missing_seq)? else {
        return Ok(false);
    };
    let Some(preview) = read_log_tail_preview(&source.path)? else {
        return Ok(false);
    };
    let created_at = Utc::now().to_rfc3339();
    let changed = conn.execute(
        "INSERT OR IGNORE INTO runtime_process_events (
            id, process_id, seq, stream, event_kind, payload_ref, payload_preview, created_at
         ) VALUES (?1, ?2, ?3, ?4, 'log_tail_replay', ?5, ?6, ?7)",
        params![
            new_replay_marker_id(),
            process_id,
            missing_seq,
            source.stream,
            source.path,
            preview,
            created_at,
        ],
    )?;
    Ok(changed > 0)
}

/// 写入无法 replay 的序列缺口边界事件。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `process_id`: 运行时进程标识
/// - `missing_seq`: 缺失事件序号
///
/// 返回:
/// - 是否写入了新的边界事件
pub(super) fn insert_replay_unavailable_marker_locked(
    conn: &Connection,
    process_id: &str,
    missing_seq: i64,
) -> Result<bool> {
    let created_at = Utc::now().to_rfc3339();
    let changed = conn.execute(
        "INSERT OR IGNORE INTO runtime_process_events (
            id, process_id, seq, stream, event_kind, payload_ref, payload_preview, created_at
         ) VALUES (?1, ?2, ?3, 'recovery', 'replay_unavailable', NULL, ?4, ?5)",
        params![
            new_replay_marker_id(),
            process_id,
            missing_seq,
            format!("runtime process event replay unavailable: missing seq {missing_seq}"),
            created_at,
        ],
    )?;
    Ok(changed > 0)
}

/// 读取最接近缺失序号的日志来源。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `process_id`: 运行时进程标识
/// - `missing_seq`: 缺失事件序号
///
/// 返回:
/// - 日志来源
fn load_nearest_log_source_locked(
    conn: &Connection,
    process_id: &str,
    missing_seq: i64,
) -> Result<Option<ReplayLogSource>> {
    let result = conn.query_row(
        "SELECT stream, payload_ref
         FROM runtime_process_events
         WHERE process_id = ?1
         AND payload_ref IS NOT NULL
         AND event_kind IN ('output_read', 'log_tail_replay')
         ORDER BY ABS(seq - ?2) ASC, seq DESC
         LIMIT 1",
        params![process_id, missing_seq],
        |row| {
            Ok(ReplayLogSource {
                stream: row.get(0)?,
                path: row.get(1)?,
            })
        },
    );
    match result {
        Ok(source) => Ok(Some(source)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// 读取日志尾部预览。
///
/// 参数:
/// - `path`: 日志路径
///
/// 返回:
/// - 日志尾部预览
fn read_log_tail_preview(path: &str) -> Result<Option<String>> {
    const MAX_REPLAY_BYTES: u64 = 4_096;
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return Ok(None),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    let start = metadata.len().saturating_sub(MAX_REPLAY_BYTES);
    let mut file = std::fs::File::open(path)?;
    use std::io::{Read, Seek, SeekFrom};
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if text.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

/// 创建 replay 边界事件标识。
///
/// 参数:
/// - 无
///
/// 返回:
/// - replay 边界事件标识
fn new_replay_marker_id() -> String {
    format!(
        "rpe_replay_{}_{}",
        Utc::now().timestamp_millis(),
        rand::random::<u16>()
    )
}

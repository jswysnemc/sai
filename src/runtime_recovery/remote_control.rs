use super::model::{NewRuntimeRecoveryRecord, RuntimeRecoveryKind, RuntimeRecoveryStatus};
use super::repository::record_recovery;
use crate::state::ConversationDb;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, OptionalExtension, Row};

/// 远端控制期望状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RemoteControlDesiredState {
    Enabled,
    Disabled,
}

impl RemoteControlDesiredState {
    /// 转换为数据库状态文本。
    ///
    /// 返回:
    /// - 数据库状态文本
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }

    /// 从数据库状态文本恢复远端控制期望状态。
    ///
    /// 参数:
    /// - `value`: 数据库状态文本
    ///
    /// 返回:
    /// - 远端控制期望状态
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "enabled" => Self::Enabled,
            _ => Self::Disabled,
        }
    }
}

/// 待写入远端控制状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteControlStateUpsert {
    pub session_id: String,
    pub desired_state: RemoteControlDesiredState,
    pub enrollment_id: Option<String>,
    pub server_id: Option<String>,
    pub client_id: Option<String>,
    pub auth_scope: Option<String>,
    pub subscribe_cursor: i64,
    pub server_seq: i64,
    pub acked_server_seq: i64,
    pub bounded_replay_limit: i64,
}

/// 远端控制持久状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemoteControlState {
    pub session_id: String,
    pub desired_state: RemoteControlDesiredState,
    pub enrollment_id: Option<String>,
    pub server_id: Option<String>,
    pub client_id: Option<String>,
    pub auth_scope: Option<String>,
    pub subscribe_cursor: i64,
    pub server_seq: i64,
    pub acked_server_seq: i64,
    pub bounded_replay_limit: i64,
    pub last_auth_failure: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 写入或更新远端控制状态。
///
/// 参数:
/// - `db`: 对话数据库
/// - `state`: 待写入远端控制状态
///
/// 返回:
/// - 写入后的远端控制状态
pub(crate) fn upsert_remote_control_state(
    db: &ConversationDb,
    state: RemoteControlStateUpsert,
) -> Result<RemoteControlState> {
    let bounded_replay_limit = state.bounded_replay_limit.max(1);
    let server_seq = state.server_seq.max(0);
    let subscribe_cursor = state.subscribe_cursor.max(0);
    let acked_server_seq = state.acked_server_seq.max(0).min(server_seq);
    db.with_conn(|conn| {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO runtime_remote_control_state (
                session_id, desired_state, enrollment_id, server_id, client_id, auth_scope,
                subscribe_cursor, server_seq, acked_server_seq, bounded_replay_limit,
                last_auth_failure, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, ?11, ?11)
             ON CONFLICT(session_id) DO UPDATE SET
                desired_state = excluded.desired_state,
                enrollment_id = excluded.enrollment_id,
                server_id = excluded.server_id,
                client_id = excluded.client_id,
                auth_scope = excluded.auth_scope,
                subscribe_cursor = MAX(runtime_remote_control_state.subscribe_cursor, excluded.subscribe_cursor),
                server_seq = MAX(runtime_remote_control_state.server_seq, excluded.server_seq),
                acked_server_seq = MAX(runtime_remote_control_state.acked_server_seq, excluded.acked_server_seq),
                bounded_replay_limit = excluded.bounded_replay_limit,
                updated_at = excluded.updated_at",
            params![
                state.session_id,
                state.desired_state.as_str(),
                state.enrollment_id,
                state.server_id,
                state.client_id,
                state.auth_scope,
                subscribe_cursor,
                server_seq,
                acked_server_seq,
                bounded_replay_limit,
                now,
            ],
        )?;
        load_remote_control_state_locked(conn, &state.session_id)?.ok_or_else(|| {
            anyhow::anyhow!(
                "remote control state was not found after upsert: {}",
                state.session_id
            )
        })
    })
}

/// 读取远端控制状态。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
///
/// 返回:
/// - 远端控制状态
pub(crate) fn load_remote_control_state(
    db: &ConversationDb,
    session_id: &str,
) -> Result<Option<RemoteControlState>> {
    db.with_conn(|conn| load_remote_control_state_locked(conn, session_id))
}

/// 推进远端控制游标和确认序号。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `subscribe_cursor`: 订阅游标
/// - `server_seq`: 服务端最新序号
/// - `acked_server_seq`: 已确认服务端序号
///
/// 返回:
/// - 推进后的远端控制状态
pub(crate) fn advance_remote_control_cursor(
    db: &ConversationDb,
    session_id: &str,
    subscribe_cursor: i64,
    server_seq: i64,
    acked_server_seq: i64,
) -> Result<RemoteControlState> {
    db.with_conn(|conn| {
        let now = Utc::now().to_rfc3339();
        let server_seq = server_seq.max(0);
        let subscribe_cursor = subscribe_cursor.max(0);
        let acked_server_seq = acked_server_seq.max(0).min(server_seq);
        conn.execute(
            "UPDATE runtime_remote_control_state
             SET subscribe_cursor = MAX(subscribe_cursor, ?1),
                 server_seq = MAX(server_seq, ?2),
                 acked_server_seq = MAX(acked_server_seq, ?3),
                 updated_at = ?4
             WHERE session_id = ?5",
            params![
                subscribe_cursor,
                server_seq,
                acked_server_seq,
                now,
                session_id
            ],
        )?;
        load_remote_control_state_locked(conn, session_id)?
            .ok_or_else(|| anyhow::anyhow!("remote control state was not found: {session_id}"))
    })
}

/// 记录远端控制认证失败。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `reason`: 认证失败原因
///
/// 返回:
/// - 写入是否成功
pub(crate) fn record_remote_control_auth_failure(
    db: &ConversationDb,
    session_id: &str,
    reason: &str,
) -> Result<()> {
    db.with_conn(|conn| {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO runtime_remote_control_state (
                session_id, desired_state, enrollment_id, server_id, client_id, auth_scope,
                subscribe_cursor, server_seq, acked_server_seq, bounded_replay_limit,
                last_auth_failure, created_at, updated_at
             ) VALUES (?1, 'disabled', NULL, NULL, NULL, NULL, 0, 0, 0, 100, ?2, ?3, ?3)
             ON CONFLICT(session_id) DO UPDATE SET
                last_auth_failure = excluded.last_auth_failure,
                updated_at = excluded.updated_at",
            params![session_id, reason, now],
        )?;
        Ok(())
    })?;
    record_recovery(
        db,
        NewRuntimeRecoveryRecord {
            session_id: session_id.to_string(),
            process_id: None,
            kind: RuntimeRecoveryKind::RemoteControlAuthFailed,
            status: RuntimeRecoveryStatus::Observed,
            reason: reason.to_string(),
            last_safe_seq: None,
        },
    )?;
    Ok(())
}

/// 读取远端控制状态。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
///
/// 返回:
/// - 远端控制状态
fn load_remote_control_state_locked(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Option<RemoteControlState>> {
    conn.query_row(
        "SELECT session_id, desired_state, enrollment_id, server_id, client_id, auth_scope,
                subscribe_cursor, server_seq, acked_server_seq, bounded_replay_limit,
                last_auth_failure, created_at, updated_at
         FROM runtime_remote_control_state
         WHERE session_id = ?1",
        params![session_id],
        map_remote_control_state,
    )
    .optional()
    .map_err(Into::into)
}

/// 从查询行恢复远端控制状态。
///
/// 参数:
/// - `row`: 查询行
///
/// 返回:
/// - 远端控制状态
fn map_remote_control_state(row: &Row<'_>) -> rusqlite::Result<RemoteControlState> {
    let desired_state: String = row.get(1)?;
    Ok(RemoteControlState {
        session_id: row.get(0)?,
        desired_state: RemoteControlDesiredState::from_str(&desired_state),
        enrollment_id: row.get(2)?,
        server_id: row.get(3)?,
        client_id: row.get(4)?,
        auth_scope: row.get(5)?,
        subscribe_cursor: row.get(6)?,
        server_seq: row.get(7)?,
        acked_server_seq: row.get(8)?,
        bounded_replay_limit: row.get(9)?,
        last_auth_failure: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_recovery::{session_summary, RuntimeRecoveryKind};

    /// 创建测试数据库。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 临时目录和对话数据库
    fn db() -> (tempfile::TempDir, ConversationDb) {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        (temp, db)
    }

    #[test]
    fn remote_control_state_round_trips_desired_state_and_enrollment() {
        let (_temp, db) = db();

        let state = upsert_remote_control_state(
            &db,
            RemoteControlStateUpsert {
                session_id: "default".to_string(),
                desired_state: RemoteControlDesiredState::Enabled,
                enrollment_id: Some("enroll_1".to_string()),
                server_id: Some("server_1".to_string()),
                client_id: Some("client_1".to_string()),
                auth_scope: Some("session:default".to_string()),
                subscribe_cursor: 7,
                server_seq: 9,
                acked_server_seq: 8,
                bounded_replay_limit: 32,
            },
        )
        .unwrap();
        let loaded = load_remote_control_state(&db, "default").unwrap().unwrap();

        assert_eq!(state, loaded);
        assert_eq!(loaded.desired_state, RemoteControlDesiredState::Enabled);
        assert_eq!(loaded.enrollment_id.as_deref(), Some("enroll_1"));
        assert_eq!(loaded.server_seq, 9);
        assert_eq!(loaded.acked_server_seq, 8);
        assert_eq!(loaded.bounded_replay_limit, 32);
    }

    #[test]
    fn remote_control_cursor_and_ack_do_not_move_backwards() {
        let (_temp, db) = db();
        upsert_remote_control_state(
            &db,
            RemoteControlStateUpsert {
                session_id: "default".to_string(),
                desired_state: RemoteControlDesiredState::Enabled,
                enrollment_id: None,
                server_id: None,
                client_id: None,
                auth_scope: None,
                subscribe_cursor: 10,
                server_seq: 20,
                acked_server_seq: 15,
                bounded_replay_limit: 64,
            },
        )
        .unwrap();

        let state = advance_remote_control_cursor(&db, "default", 5, 12, 9).unwrap();

        assert_eq!(state.subscribe_cursor, 10);
        assert_eq!(state.server_seq, 20);
        assert_eq!(state.acked_server_seq, 15);
    }

    #[test]
    fn remote_control_auth_failure_does_not_write_conversation_turn() {
        let (_temp, db) = db();

        record_remote_control_auth_failure(&db, "default", "token expired").unwrap();

        let state = load_remote_control_state(&db, "default").unwrap().unwrap();
        let summary = session_summary(&db, "default").unwrap();
        let turn_count: i64 = db
            .with_conn(|conn| {
                Ok(conn.query_row("SELECT COUNT(*) FROM turns", [], |row| row.get(0))?)
            })
            .unwrap();
        let failure = summary.latest_failure.unwrap();

        assert_eq!(turn_count, 0);
        assert_eq!(state.desired_state, RemoteControlDesiredState::Disabled);
        assert_eq!(state.last_auth_failure.as_deref(), Some("token expired"));
        assert_eq!(failure.kind, RuntimeRecoveryKind::RemoteControlAuthFailed);
        assert_eq!(failure.reason, "token expired");
    }
}

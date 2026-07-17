use anyhow::Result;
use rusqlite::Connection;

/// 创建 Runtime Recovery 相关数据表。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 表结构初始化是否成功
pub(crate) fn create_runtime_recovery_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS runtime_processes (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            owner_kind TEXT NOT NULL,
            owner_id TEXT NOT NULL,
            process_kind TEXT NOT NULL,
            command TEXT NOT NULL,
            cwd TEXT NOT NULL,
            pid INTEGER,
            pgid INTEGER,
            status TEXT NOT NULL,
            last_seq INTEGER NOT NULL DEFAULT 0,
            last_seen_at TEXT,
            started_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            ended_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_runtime_processes_session_status
            ON runtime_processes(session_id, status, updated_at);
        CREATE INDEX IF NOT EXISTS idx_runtime_processes_owner
            ON runtime_processes(owner_kind, owner_id, status);

        CREATE TABLE IF NOT EXISTS runtime_process_events (
            id TEXT PRIMARY KEY,
            process_id TEXT NOT NULL,
            seq INTEGER NOT NULL,
            stream TEXT NOT NULL,
            event_kind TEXT NOT NULL,
            payload_ref TEXT,
            payload_preview TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(process_id, seq)
        );
        CREATE INDEX IF NOT EXISTS idx_runtime_process_events_process_seq
            ON runtime_process_events(process_id, seq);

        CREATE TABLE IF NOT EXISTS runtime_recovery_records (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            process_id TEXT,
            kind TEXT NOT NULL,
            status TEXT NOT NULL,
            reason TEXT NOT NULL,
            last_safe_seq INTEGER,
            created_at TEXT NOT NULL,
            resolved_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_runtime_recovery_session_status
            ON runtime_recovery_records(session_id, status, created_at);
        CREATE INDEX IF NOT EXISTS idx_runtime_recovery_process
            ON runtime_recovery_records(process_id, created_at);

        CREATE TABLE IF NOT EXISTS runtime_remote_control_state (
            session_id TEXT PRIMARY KEY,
            desired_state TEXT NOT NULL,
            enrollment_id TEXT,
            server_id TEXT,
            client_id TEXT,
            auth_scope TEXT,
            subscribe_cursor INTEGER NOT NULL DEFAULT 0,
            server_seq INTEGER NOT NULL DEFAULT 0,
            acked_server_seq INTEGER NOT NULL DEFAULT 0,
            bounded_replay_limit INTEGER NOT NULL DEFAULT 100,
            last_auth_failure TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_runtime_remote_control_state_updated
            ON runtime_remote_control_state(updated_at);

        CREATE TABLE IF NOT EXISTS runtime_transport_state (
            session_id TEXT NOT NULL,
            transport_kind TEXT NOT NULL,
            transport_id TEXT NOT NULL,
            cursor_seq INTEGER NOT NULL DEFAULT 0,
            acked_seq INTEGER NOT NULL DEFAULT 0,
            bounded_replay_limit INTEGER NOT NULL DEFAULT 100,
            last_close_reason TEXT,
            last_closed_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (session_id, transport_kind, transport_id)
        );
        CREATE INDEX IF NOT EXISTS idx_runtime_transport_state_updated
            ON runtime_transport_state(updated_at);

        CREATE TABLE IF NOT EXISTS runtime_transport_events (
            session_id TEXT NOT NULL,
            transport_kind TEXT NOT NULL,
            transport_id TEXT NOT NULL,
            seq INTEGER NOT NULL,
            payload_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (session_id, transport_kind, transport_id, seq)
        );
        CREATE INDEX IF NOT EXISTS idx_runtime_transport_events_transport_seq
            ON runtime_transport_events(session_id, transport_kind, transport_id, seq);",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_runtime_recovery_tables_and_indexes() {
        let conn = Connection::open_in_memory().unwrap();

        create_runtime_recovery_tables(&conn).unwrap();

        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table'
                 AND name IN (
                    'runtime_processes',
                    'runtime_process_events',
                    'runtime_recovery_records',
                    'runtime_remote_control_state',
                    'runtime_transport_state',
                    'runtime_transport_events'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let index_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index'
                 AND name IN (
                    'idx_runtime_processes_session_status',
                    'idx_runtime_process_events_process_seq',
                    'idx_runtime_recovery_session_status',
                    'idx_runtime_remote_control_state_updated',
                    'idx_runtime_transport_state_updated',
                    'idx_runtime_transport_events_transport_seq'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(table_count, 6);
        assert_eq!(index_count, 6);
    }
}

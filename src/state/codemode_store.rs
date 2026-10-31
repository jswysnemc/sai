use super::StateStore;
use anyhow::{bail, Result};
use rusqlite::{params, Connection};
use sai_codemode::store::{self, Store};

/// 【CodeMode】【状态建表】在 conn 创建绑定轮次的增量表，返回建表结果。
pub(crate) fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS codemode_store (
        seq INTEGER PRIMARY KEY AUTOINCREMENT,
        turn_id TEXT NOT NULL REFERENCES turns(turn_id) ON DELETE CASCADE,
        call_id TEXT NOT NULL,
        writes_json TEXT NOT NULL,
        UNIQUE(turn_id, call_id)
    ); CREATE INDEX IF NOT EXISTS idx_codemode_store_turn ON codemode_store(turn_id);",
    )?;
    Ok(())
}

/// 【CodeMode】【分支读取】读取 conn 中 turn_id 的祖先增量，返回仅属于该路径的状态。
fn load_locked(conn: &Connection, turn_id: &str) -> Result<Store> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM turns WHERE turn_id=?1)",
        [turn_id],
        |row| row.get(0),
    )?;
    if !exists {
        bail!("codemode store turn not found: {turn_id}");
    }
    let mut statement = conn.prepare(
        "WITH RECURSIVE lineage(turn_id, parent_turn_id, depth) AS (
        SELECT turn_id, parent_turn_id, 0 FROM turns WHERE turn_id = ?1
        UNION ALL
        SELECT t.turn_id, t.parent_turn_id, l.depth + 1 FROM turns t
        JOIN lineage l ON t.turn_id = l.parent_turn_id
    ) SELECT s.writes_json FROM lineage l JOIN codemode_store s ON s.turn_id = l.turn_id
      ORDER BY l.depth DESC, s.seq ASC",
    )?;
    let mut values = Store::new();
    for json in statement.query_map([turn_id], |row| row.get::<_, String>(0))? {
        values = store::apply(&values, &serde_json::from_str(&json?)?)?;
    }
    Ok(values)
}

impl StateStore {
    /// 【CodeMode】【状态恢复】从 turn_id 当前分支恢复脚本状态，返回快照。
    pub(crate) fn load_codemode_store(&self, turn_id: &str) -> Result<Store> {
        self.conv_db.with_conn(|conn| load_locked(conn, turn_id))
    }

    /// 【CodeMode】【状态提交】保存 turn_id/call_id 成功脚本的 writes，返回原子提交结果。
    pub(crate) fn commit_codemode_store(
        &self,
        turn_id: &str,
        call_id: &str,
        writes: &Store,
    ) -> Result<()> {
        self.conv_db.with_conn(|conn| {
            let transaction = conn.unchecked_transaction()?;
            let values = load_locked(&transaction, turn_id)?;
            store::apply(&values, writes)?;
            transaction.execute(
                "INSERT INTO codemode_store(turn_id, call_id, writes_json) VALUES (?1, ?2, ?3)",
                params![turn_id, call_id, serde_json::to_string(writes)?],
            )?;
            transaction.commit()?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ConversationDb;
    use serde_json::json;

    #[test]
    fn state_follows_ancestors_not_other_branches_and_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(dir.path()).unwrap();
        db.start_turn("root", "root").unwrap();
        db.start_turn("branch_a", "a").unwrap();
        db.switch_active_leaf("root").unwrap();
        db.start_turn("branch_b", "b").unwrap();
        db.with_conn(|conn| {
            for (turn, value) in [("root", 1), ("branch_a", 2), ("branch_b", 3)] {
                conn.execute(
                    "INSERT INTO codemode_store(turn_id,call_id,writes_json) VALUES(?1,?2,?3)",
                    params![turn, turn, json!({"n":{"value":value}}).to_string()],
                )?;
            }
            assert_eq!(load_locked(conn, "branch_a")?["n"], json!(2));
            assert_eq!(load_locked(conn, "branch_b")?["n"], json!(3));
            assert_eq!(load_locked(conn, "root")?["n"], json!(1));
            Ok(())
        })
        .unwrap();
        drop(db);
        let reopened = ConversationDb::open(dir.path()).unwrap();
        reopened
            .with_conn(|conn| {
                assert_eq!(load_locked(conn, "branch_a")?["n"], json!(2));
                conn.execute("DELETE FROM turns WHERE turn_id='branch_a'", [])?;
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM codemode_store WHERE turn_id='branch_a'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(count, 0);
                Ok(())
            })
            .unwrap();
    }
}

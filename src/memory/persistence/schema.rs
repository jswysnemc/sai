use anyhow::Result;
use rusqlite::Connection;

/// 统一记忆表的建表语句。
///
/// 旧模型把 facts 与 episodes 拆成两张结构相同的表，检索时要分别查询再合并，
/// 类型是隐含在表名里的。这里合成一张表，类型与作用域成为可查询的一等字段。
const CREATE_MEMORIES: &str = "CREATE TABLE IF NOT EXISTS memories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    scope_path TEXT,
    content TEXT NOT NULL,
    salience REAL NOT NULL DEFAULT 0.5,
    tags TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)";

/// 按类型与作用域检索的索引。
const CREATE_SCOPE_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS idx_memories_kind_scope ON memories (kind, scope_path)";

/// 西文全文索引；unicode61 按词边界切分，适合 ASCII 与带变音符号的文本。
///
/// 不使用 contentless 模式（`content=''`）：它不支持 DELETE，
/// 记忆更新与删除时无法清理旧索引项，会留下指向已删除行的残留命中。
const CREATE_FTS: &str = "CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    body,
    tokenize = 'unicode61 remove_diacritics 2'
)";

/// 中文全文索引；unicode61 不切分连续 CJK，必须用 trigram 才能命中子串。
const CREATE_FTS_TRIGRAM: &str = "CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts_tri USING fts5(
    body,
    tokenize = 'trigram'
)";

/// 建立统一记忆表及其索引。
///
/// 参数:
/// - `conn`: 记忆数据库连接
///
/// 返回:
/// - 建表是否成功
pub fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(&format!(
        "{CREATE_MEMORIES};{CREATE_SCOPE_INDEX};{CREATE_FTS};{CREATE_FTS_TRIGRAM};"
    ))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证建表语句可重复执行。
    #[test]
    fn schema_creation_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();
        ensure_schema(&conn).unwrap();
    }

    /// 验证表与索引都被建立。
    #[test]
    fn creates_the_table_and_its_index() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();

        let table: String = conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='memories'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table, "memories");

        let index: String = conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_memories_kind_scope'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(index, "idx_memories_kind_scope");
    }

    /// 验证全文索引表被建立。
    #[test]
    fn creates_the_fts_table() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='memories_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}

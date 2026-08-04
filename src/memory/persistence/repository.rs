use super::super::model::{MemoryCandidate, MemoryItem, MemoryKind, MemoryScope};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};

/// 写入一条新记忆。
///
/// 参数:
/// - `conn`: 记忆数据库连接
/// - `candidate`: 已通过准入判定的候选
/// - `now`: 写入时间，RFC3339
///
/// 返回:
/// - 新记忆的主键
pub fn insert(conn: &Connection, candidate: &MemoryCandidate, now: &str) -> Result<i64> {
    let tags = candidate.tags.join(",");
    conn.execute(
        "INSERT INTO memories (kind, scope_path, content, salience, tags, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![
            candidate.kind.as_str(),
            candidate.scope.stored_path(),
            candidate.content,
            candidate.salience,
            tags,
            now,
        ],
    )?;
    let id = conn.last_insert_rowid();
    sync_fts(conn, id, &candidate.content, &tags)?;
    Ok(id)
}

/// 用新内容覆盖既有记忆。
///
/// 同一事实被再次提到时更新而非新增，避免库里堆满近义重复。
/// 显著性取两者较大值：一件事被反复提起说明它更重要，不是更不重要。
///
/// 参数:
/// - `conn`: 记忆数据库连接
/// - `id`: 既有记忆主键
/// - `candidate`: 新的候选内容
/// - `now`: 更新时间，RFC3339
///
/// 返回:
/// - 更新是否成功
pub fn update(conn: &Connection, id: i64, candidate: &MemoryCandidate, now: &str) -> Result<()> {
    let tags = candidate.tags.join(",");
    conn.execute(
        "UPDATE memories
         SET content=?1, salience=max(salience, ?2), tags=?3, updated_at=?4
         WHERE id=?5",
        params![candidate.content, candidate.salience, tags, now, id],
    )?;
    sync_fts(conn, id, &candidate.content, &tags)?;
    Ok(())
}

/// 删除一条记忆。
///
/// 参数:
/// - `conn`: 记忆数据库连接
/// - `id`: 记忆主键
///
/// 返回:
/// - 是否确实删除了记录
pub fn delete(conn: &Connection, id: i64) -> Result<bool> {
    for table in ["memories_fts", "memories_fts_tri"] {
        conn.execute(&format!("DELETE FROM {table} WHERE rowid=?1"), params![id])?;
    }
    let affected = conn.execute("DELETE FROM memories WHERE id=?1", params![id])?;
    Ok(affected > 0)
}

/// 列出指定类型与作用域下的记忆，用于去重比对。
///
/// 参数:
/// - `conn`: 记忆数据库连接
/// - `kind`: 记忆类型
/// - `scope`: 作用域
///
/// 返回:
/// - 该类型与作用域下的全部记忆
pub fn list_by_kind_and_scope(
    conn: &Connection,
    kind: MemoryKind,
    scope: &MemoryScope,
) -> Result<Vec<MemoryItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, scope_path, content, salience, tags, created_at, updated_at
         FROM memories
         WHERE kind=?1 AND coalesce(scope_path,'')=coalesce(?2,'')",
    )?;
    let rows = stmt.query_map(params![kind.as_str(), scope.stored_path()], map_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .context("读取同类记忆失败")
}

/// 按主键读取一条记忆。
///
/// 参数:
/// - `conn`: 记忆数据库连接
/// - `id`: 记忆主键
///
/// 返回:
/// - 记忆条目；不存在时为 None
pub fn find(conn: &Connection, id: i64) -> Result<Option<MemoryItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, scope_path, content, salience, tags, created_at, updated_at
         FROM memories WHERE id=?1",
    )?;
    let mut rows = stmt.query_map(params![id], map_row)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// 同步全文索引。
///
/// 两套索引各司其职：unicode61 按词切分处理西文，trigram 处理连续 CJK
/// ——unicode61 会把整段中文当成一个词元，导致中文查询永远无法命中。
///
/// 参数:
/// - `conn`: 记忆数据库连接
/// - `id`: 记忆主键
/// - `content`: 记忆正文
/// - `tags`: 逗号分隔的标签
///
/// 返回:
/// - 同步是否成功
fn sync_fts(conn: &Connection, id: i64, content: &str, tags: &str) -> Result<()> {
    // 标签与正文一并索引，让正文中未出现的同义词也能命中
    let body = if tags.is_empty() {
        content.to_string()
    } else {
        format!("{content}\n{}", tags.replace(',', " "))
    };
    for table in ["memories_fts", "memories_fts_tri"] {
        conn.execute(&format!("DELETE FROM {table} WHERE rowid=?1"), params![id])?;
        conn.execute(
            &format!("INSERT INTO {table} (rowid, body) VALUES (?1, ?2)"),
            params![id, body],
        )?;
    }
    Ok(())
}

/// 把查询结果行映射为记忆条目。
///
/// 参数:
/// - `row`: 查询结果行
///
/// 返回:
/// - 记忆条目
fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryItem> {
    let kind_text: String = row.get(1)?;
    let scope_path: Option<String> = row.get(2)?;
    let tags_text: String = row.get(5)?;
    Ok(MemoryItem {
        id: row.get(0)?,
        // 类型字段由本模块写入，遇到未知值按事实处理而不是丢弃整条记录
        kind: MemoryKind::parse(&kind_text).unwrap_or(MemoryKind::Fact),
        scope: MemoryScope::from_stored(scope_path.as_deref()),
        content: row.get(3)?,
        salience: row.get(4)?,
        tags: tags_text
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(str::to_string)
            .collect(),
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::persistence::schema::ensure_schema;

    const NOW: &str = "2026-08-04T00:00:00Z";

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();
        conn
    }

    fn candidate(kind: MemoryKind, content: &str, salience: f64) -> MemoryCandidate {
        MemoryCandidate {
            kind,
            scope: MemoryScope::Global,
            content: content.to_string(),
            salience,
            tags: vec!["pnpm".to_string()],
        }
    }

    /// 验证写入后可以按主键读回。
    #[test]
    fn inserts_and_reads_back_a_memory() {
        let conn = conn();
        let id = insert(&conn, &candidate(MemoryKind::Preference, "用户使用 pnpm", 0.8), NOW).unwrap();

        let item = find(&conn, id).unwrap().expect("刚写入的记忆应当存在");
        assert_eq!(item.kind, MemoryKind::Preference);
        assert_eq!(item.content, "用户使用 pnpm");
        assert_eq!(item.salience, 0.8);
        assert_eq!(item.tags, vec!["pnpm".to_string()]);
        assert_eq!(item.scope, MemoryScope::Global);
    }

    /// 验证更新覆盖正文并保留较高的显著性。
    #[test]
    fn update_keeps_the_higher_salience() {
        let conn = conn();
        let id = insert(&conn, &candidate(MemoryKind::Preference, "用户使用 pnpm", 0.9), NOW).unwrap();
        update(
            &conn,
            id,
            &candidate(MemoryKind::Preference, "用户在所有项目使用 pnpm", 0.4),
            "2026-08-05T00:00:00Z",
        )
        .unwrap();

        let item = find(&conn, id).unwrap().unwrap();
        assert_eq!(item.content, "用户在所有项目使用 pnpm");
        assert_eq!(item.salience, 0.9, "反复提起的事实不应降低显著性");
        assert_eq!(item.updated_at, "2026-08-05T00:00:00Z");
    }

    /// 验证按类型与作用域过滤。
    #[test]
    fn lists_only_the_matching_kind_and_scope() {
        let conn = conn();
        insert(&conn, &candidate(MemoryKind::Preference, "偏好一", 0.8), NOW).unwrap();
        insert(&conn, &candidate(MemoryKind::Fact, "事实一", 0.8), NOW).unwrap();
        let mut scoped = candidate(MemoryKind::Preference, "项目偏好", 0.8);
        scoped.scope = MemoryScope::from_stored(Some("/home/a"));
        insert(&conn, &scoped, NOW).unwrap();

        let global = list_by_kind_and_scope(&conn, MemoryKind::Preference, &MemoryScope::Global).unwrap();
        assert_eq!(global.len(), 1);
        assert_eq!(global[0].content, "偏好一");

        let project = list_by_kind_and_scope(
            &conn,
            MemoryKind::Preference,
            &MemoryScope::from_stored(Some("/home/a")),
        )
        .unwrap();
        assert_eq!(project.len(), 1);
        assert_eq!(project[0].content, "项目偏好");
    }

    /// 验证删除同时清理全文索引。
    #[test]
    fn delete_removes_the_row_and_its_index_entry() {
        let conn = conn();
        let id = insert(&conn, &candidate(MemoryKind::Fact, "用户使用 Arch Linux", 0.7), NOW).unwrap();

        assert!(delete(&conn, id).unwrap());
        assert!(find(&conn, id).unwrap().is_none());
        let indexed: i64 = conn
            .query_row("SELECT count(*) FROM memories_fts WHERE rowid=?1", params![id], |row| row.get(0))
            .unwrap();
        assert_eq!(indexed, 0);
    }

    /// 验证删除不存在的记录不报错。
    #[test]
    fn deleting_a_missing_row_reports_no_change() {
        let conn = conn();
        assert!(!delete(&conn, 999).unwrap());
    }

    /// 验证标签进入全文索引，可按同义词命中。
    #[test]
    fn tags_are_searchable_through_the_index() {
        let conn = conn();
        let mut item = candidate(MemoryKind::Preference, "用户偏好特定的包管理器", 0.8);
        item.tags = vec!["pnpm".to_string()];
        insert(&conn, &item, NOW).unwrap();

        let hits: i64 = conn
            .query_row("SELECT count(*) FROM memories_fts WHERE body MATCH 'pnpm'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(hits, 1, "正文未出现的标签应当可以命中");
    }
}

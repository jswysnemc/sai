use super::search::{query_tokens, score_text, snippet};
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// 单次检索最多扫描的轮次条数。
///
/// 逐出记录只增不减，全表打分会随会话时长线性变慢。取最近若干条即可：
/// 越早的内容越可能已经被更近的摘要覆盖过。
const SCAN_LIMIT: usize = 1_000;

/// 一条被压缩清出上下文的对话轮次。
#[derive(Debug, Clone)]
pub struct EvictedTurn {
    /// 原始时间戳
    pub timestamp: String,
    /// 角色
    pub role: String,
    /// 正文
    pub content: String,
}

/// 逐出轮次的存储。
///
/// 与记忆分开存放在 state 目录：它是会话的派生数据，随会话重置而清空，
/// 不该跟着长期记忆一起被备份或迁移。
#[derive(Clone)]
pub struct EvictedStore {
    db_path: PathBuf,
}

impl EvictedStore {
    /// 创建存储句柄。
    ///
    /// 参数:
    /// - `db_path`: 数据库文件路径
    ///
    /// 返回:
    /// - 存储句柄
    pub fn new(db_path: impl AsRef<Path>) -> Self {
        Self {
            db_path: db_path.as_ref().to_path_buf(),
        }
    }

    /// 打开连接并确保表结构存在。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 已建表的连接
    fn conn(&self) -> Result<Connection> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&self.db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS evicted_turns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL
            );",
        )?;
        Ok(conn)
    }

    /// 记录一批被清出上下文的轮次。
    ///
    /// 参数:
    /// - `turns`: 逐出的轮次
    ///
    /// 返回:
    /// - 写入结果
    pub fn remember(&self, turns: &[EvictedTurn]) -> Result<()> {
        if turns.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn()?;
        let created_at = Utc::now().to_rfc3339();
        let tx = conn.transaction()?;
        for turn in turns {
            tx.execute(
                "INSERT INTO evicted_turns (timestamp, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![turn.timestamp, turn.role, turn.content, created_at],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// 清空全部记录。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 清空结果
    pub fn clear(&self) -> Result<()> {
        self.conn()?.execute("DELETE FROM evicted_turns", [])?;
        Ok(())
    }

    /// 统计已记录的轮次条数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 条数；库文件尚未建立时为 0
    pub fn count(&self) -> Result<i64> {
        if !self.db_path.is_file() {
            return Ok(0);
        }
        Ok(self
            .conn()?
            .query_row("SELECT COUNT(*) FROM evicted_turns", [], |row| row.get(0))?)
    }

    /// 检索逐出轮次。
    ///
    /// 参数:
    /// - `query`: 查询文本
    /// - `limit`: 返回条数上限
    /// - `snippet_chars`: 片段最大字符数
    ///
    /// 返回:
    /// - 检索结果的 JSON；库文件不存在时返回空结果而不是报错
    pub fn search(&self, query: &str, limit: usize, snippet_chars: usize) -> Result<Value> {
        if !self.db_path.is_file() {
            return Ok(json!({ "ok": true, "query": query, "results": [] }));
        }
        let tokens = query_tokens(query);
        let conn = self.conn()?;
        let mut statement = conn.prepare(
            "SELECT id, timestamp, role, content FROM evicted_turns ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map([SCAN_LIMIT], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut hits = Vec::new();
        for row in rows {
            let (id, timestamp, role, content) = row?;
            let score = score_text(&content, &tokens);
            if score <= 0.0 {
                continue;
            }
            hits.push(json!({
                "id": id,
                "timestamp": timestamp,
                "role": role,
                "score": score,
                "snippet": snippet(&content, &tokens, snippet_chars),
            }));
        }
        hits.sort_by(|left, right| {
            right
                .get("score")
                .and_then(Value::as_f64)
                .unwrap_or_default()
                .partial_cmp(
                    &left
                        .get("score")
                        .and_then(Value::as_f64)
                        .unwrap_or_default(),
                )
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(limit);
        Ok(json!({ "ok": true, "query": query, "results": hits }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一条逐出轮次。
    ///
    /// 参数:
    /// - `content`: 正文
    ///
    /// 返回:
    /// - 轮次
    fn turn(content: &str) -> EvictedTurn {
        EvictedTurn {
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            role: "user".to_string(),
            content: content.to_string(),
        }
    }

    /// 构造指向临时目录的存储。
    ///
    /// 参数:
    /// - `dir`: 临时目录
    ///
    /// 返回:
    /// - 存储句柄
    fn store(dir: &tempfile::TempDir) -> EvictedStore {
        EvictedStore::new(dir.path().join("evicted.db"))
    }

    /// 验证写入后能检索到。
    #[test]
    fn a_remembered_turn_can_be_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store
            .remember(&[turn("压缩改为前缀回放以复用缓存")])
            .unwrap();

        let found = store.search("前缀回放", 5, 200).unwrap();

        assert_eq!(found["results"].as_array().unwrap().len(), 1);
    }

    /// 验证库文件不存在时返回空结果而不是报错。
    ///
    /// 摘要末尾的回读指引会让模型直接调用，还没压缩过就报错很难看。
    #[test]
    fn searching_before_anything_was_evicted_yields_empty() {
        let dir = tempfile::tempdir().unwrap();

        let found = store(&dir).search("任意", 5, 200).unwrap();

        assert!(found["results"].as_array().unwrap().is_empty());
    }

    /// 验证不命中的内容不进结果。
    #[test]
    fn unrelated_turns_are_excluded() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store.remember(&[turn("完全无关的内容")]).unwrap();

        let found = store.search("压缩缓存", 5, 200).unwrap();

        assert!(found["results"].as_array().unwrap().is_empty());
    }

    /// 验证结果按得分降序且受条数限制。
    #[test]
    fn results_are_ranked_and_capped() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store
            .remember(&[
                turn("只提到压缩"),
                turn("同时提到压缩与缓存两件事"),
                turn("也只提到压缩"),
            ])
            .unwrap();

        let found = store.search("压缩 缓存", 2, 200).unwrap();
        let results = found["results"].as_array().unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0]["snippet"].as_str().unwrap().contains("缓存"));
    }

    /// 验证清空后检索不到。
    #[test]
    fn clearing_removes_everything() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);
        store.remember(&[turn("压缩")]).unwrap();

        store.clear().unwrap();

        assert_eq!(store.count().unwrap(), 0);
    }

    /// 验证空批次不建库也不报错。
    #[test]
    fn remembering_nothing_is_a_no_op() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(&dir);

        store.remember(&[]).unwrap();

        assert_eq!(store.count().unwrap(), 0);
    }
}

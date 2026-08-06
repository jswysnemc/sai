use super::capture::{evaluate_capture, evaluate_dedupe, CapturePolicyVerdict, DedupeOutcome};
use super::injection::{render_injection, select_for_injection};
use super::model::MemoryCandidate;
use super::persistence::{repository, schema};
use super::retrieval;
use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

/// 统一记忆库的读写入口。
///
/// 把准入判定、去重、检索、注入筛选串成两条链路：写入链路决定什么值得记，
/// 召回链路决定什么值得注入。三个旧问题各由一环负责——
/// 写入太滥归准入与去重，召回不准归作用域过滤与双索引检索，
/// 注入无效归相关度门槛与断言式措辞。
pub struct MemoryLibrary {
    db_path: std::path::PathBuf,
    half_life_days: f64,
}

impl MemoryLibrary {
    /// 创建记忆库句柄。
    ///
    /// 参数:
    /// - `db_path`: 记忆数据库文件路径
    /// - `half_life_days`: 记忆强度半衰期
    ///
    /// 返回:
    /// - 记忆库句柄
    pub fn new(db_path: impl AsRef<Path>, half_life_days: f64) -> Self {
        Self {
            db_path: db_path.as_ref().to_path_buf(),
            half_life_days,
        }
    }

    /// 打开数据库连接并确保表结构存在。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 已建表的数据库连接
    fn conn(&self) -> Result<Connection> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&self.db_path)?;
        schema::ensure_schema(&conn)?;
        Ok(conn)
    }

    /// 写入一批候选记忆。
    ///
    /// 每条候选先过准入判定，通过后再与同类同作用域的既有记忆比对：
    /// 命中等价记忆则更新，否则新增。
    ///
    /// 参数:
    /// - `candidates`: 抽取模型给出的候选
    /// - `now`: 写入时间，RFC3339
    ///
    /// 返回:
    /// - 实际写入或更新的条数
    pub fn capture(&self, candidates: Vec<MemoryCandidate>, now: &str) -> Result<usize> {
        let conn = self.conn()?;
        let mut written = 0usize;
        for candidate in candidates {
            let Some(candidate) = candidate.normalized() else {
                continue;
            };
            // 1. 准入判定挡掉流水账与对话噪声
            if evaluate_capture(&candidate) != CapturePolicyVerdict::Accept {
                continue;
            }
            // 2. 与既有记忆比对，命中则更新而不是堆近义重复
            let existing =
                repository::list_by_kind_and_scope(&conn, candidate.kind, &candidate.scope)?;
            match evaluate_dedupe(&candidate, &existing) {
                DedupeOutcome::Update { existing_id } => {
                    repository::update(&conn, existing_id, &candidate, now)?;
                }
                DedupeOutcome::Insert => {
                    repository::insert(&conn, &candidate, now)?;
                }
            }
            written += 1;
        }
        Ok(written)
    }

    /// 按当前输入召回记忆并渲染为注入文本。
    ///
    /// 参数:
    /// - `query`: 当前用户输入
    /// - `workspace`: 当前工作区路径；无工作区时为 None
    /// - `now_days`: 当前时间的天数刻度
    ///
    /// 返回:
    /// - 注入文本；没有足够相关的记忆时为 None
    pub fn recall(
        &self,
        query: &str,
        workspace: Option<&str>,
        now_days: f64,
    ) -> Result<Option<String>> {
        let conn = self.conn()?;
        let hits = retrieval::search(&conn, query, workspace, now_days, self.half_life_days)?;
        Ok(render_injection(&select_for_injection(hits)))
    }
}

/// 返回当前时间的天数刻度。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 距 Unix 纪元的天数
pub fn now_days() -> f64 {
    chrono::Utc::now().timestamp() as f64 / 86_400.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::model::{MemoryKind, MemoryScope};

    const NOW: &str = "2026-08-04T00:00:00Z";
    /// 2026-08-04 对应的天数刻度。
    const TODAY: f64 = 20_669.0;

    fn library(dir: &tempfile::TempDir) -> MemoryLibrary {
        MemoryLibrary::new(dir.path().join("memory.db"), 90.0)
    }

    fn candidate(kind: MemoryKind, content: &str, salience: f64) -> MemoryCandidate {
        MemoryCandidate {
            kind,
            scope: MemoryScope::Global,
            content: content.to_string(),
            salience,
            tags: Vec::new(),
        }
    }

    /// 验证显著的偏好写入后可以召回并注入。
    #[test]
    fn a_salient_preference_is_captured_and_recalled() {
        let dir = tempfile::tempdir().unwrap();
        let library = library(&dir);

        let written = library
            .capture(
                vec![candidate(
                    MemoryKind::Preference,
                    "用户在 Node 项目中一律使用 pnpm",
                    0.8,
                )],
                NOW,
            )
            .unwrap();
        assert_eq!(written, 1);

        let injected = library.recall("该用 pnpm 还是 npm", None, TODAY).unwrap();
        let injected = injected.expect("显著的偏好应当被召回");
        assert!(injected.contains("用户在 Node 项目中一律使用 pnpm"));
        assert!(injected.contains("请在本轮中遵循"));
    }

    /// 验证低显著性的往事不入库。
    ///
    /// 这正是旧实现堆成流水账的那一类内容。
    #[test]
    fn a_low_salience_episode_is_not_captured() {
        let dir = tempfile::tempdir().unwrap();
        let library = library(&dir);

        let written = library
            .capture(
                vec![candidate(
                    MemoryKind::Episode,
                    "帮用户看了一下配置文件",
                    0.3,
                )],
                NOW,
            )
            .unwrap();
        assert_eq!(written, 0);
        assert!(library.recall("配置文件", None, TODAY).unwrap().is_none());
    }

    /// 验证复述对话动作的内容不入库。
    #[test]
    fn conversational_narration_is_not_captured() {
        let dir = tempfile::tempdir().unwrap();
        let library = library(&dir);

        let written = library
            .capture(
                vec![candidate(
                    MemoryKind::Fact,
                    "用户询问了如何配置代理，助手给出了步骤",
                    0.95,
                )],
                NOW,
            )
            .unwrap();
        assert_eq!(written, 0);
    }

    /// 验证同一偏好反复写入不会堆成重复记录。
    #[test]
    fn repeating_the_same_preference_updates_instead_of_duplicating() {
        let dir = tempfile::tempdir().unwrap();
        let library = library(&dir);

        library
            .capture(
                vec![candidate(MemoryKind::Preference, "用户一律使用 pnpm", 0.8)],
                NOW,
            )
            .unwrap();
        library
            .capture(
                vec![candidate(
                    MemoryKind::Preference,
                    "用户一律使用 pnpm。",
                    0.6,
                )],
                NOW,
            )
            .unwrap();

        let conn = library.conn().unwrap();
        let count: i64 = conn
            .query_row("SELECT count(*) FROM memories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "等价记忆应当合并为一条");
    }

    /// 验证项目记忆不会泄漏到其它工作区。
    #[test]
    fn project_memories_stay_within_their_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let library = library(&dir);
        let mut scoped = candidate(MemoryKind::Decision, "构建改用 vite 而非 webpack", 0.9);
        scoped.scope = MemoryScope::from_stored(Some("/home/a"));

        library.capture(vec![scoped], NOW).unwrap();

        assert!(library
            .recall("构建改用什么", Some("/home/b"), TODAY)
            .unwrap()
            .is_none());
        assert!(library
            .recall("构建改用什么", Some("/home/a"), TODAY)
            .unwrap()
            .is_some());
    }

    /// 验证无关输入不会触发注入。
    ///
    /// 旧实现无论相关度如何都注入，再用免责声明推卸，反而让模型忽略全部记忆。
    #[test]
    fn an_unrelated_query_injects_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let library = library(&dir);
        library
            .capture(
                vec![candidate(MemoryKind::Preference, "用户一律使用 pnpm", 0.8)],
                NOW,
            )
            .unwrap();

        assert!(library
            .recall("今天天气怎么样", None, TODAY)
            .unwrap()
            .is_none());
    }

    /// 验证空候选列表不产生写入。
    #[test]
    fn an_empty_batch_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(library(&dir).capture(Vec::new(), NOW).unwrap(), 0);
    }
}

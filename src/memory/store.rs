use crate::config::{AppConfig, MemoryConfig};
use crate::memory::evicted::{EvictedStore, EvictedTurn};
use crate::memory::file_store::{render_index_injection_for, FileMemoryLibrary, MemoryScope};
use crate::paths::SaiPaths;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// 记忆功能的统一入口。
///
/// 底下是两套彼此独立的存储：长期记忆是人可读的 markdown 文件，
/// 被压缩清出的对话轮次是 SQLite。前者跨会话长存、可以手改也可以纳入
/// 版本控制；后者是会话的派生数据，随会话重置而清空。
#[derive(Clone)]
pub struct MemoryStore {
    config: MemoryConfig,
    /// 文件式记忆的根目录，已按人格隔离
    notes_dir: PathBuf,
    evicted: EvictedStore,
}

impl MemoryStore {
    /// 创建记忆入口。
    ///
    /// 参数:
    /// - `config`: 应用配置
    /// - `paths`: Sai 路径集合
    ///
    /// 返回:
    /// - 记忆入口
    pub fn new(config: &AppConfig, paths: &SaiPaths) -> Self {
        let state_dir = config.active_persona_memory_state_dir(paths).join("memory");
        Self {
            config: config.memory_config().clone(),
            notes_dir: crate::memory::notes_dir(config, paths),
            evicted: EvictedStore::new(state_dir.join("evicted_context.db")),
        }
    }

    /// 返回记忆功能是否启用。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 配置中记忆开关的当前值
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// 建立记忆目录。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 建立结果
    pub fn init(&self) -> Result<()> {
        std::fs::create_dir_all(&self.notes_dir)?;
        Ok(())
    }

    /// 打开文件式记忆库。
    ///
    /// 参数:
    /// - `workspace`: 当前工作区路径；无工作区时只有全局记忆
    ///
    /// 返回:
    /// - 记忆库
    pub fn notes(&self, workspace: Option<&Path>) -> FileMemoryLibrary {
        FileMemoryLibrary::new(&self.notes_dir, workspace)
    }

    /// 把记忆索引渲染为注入文本。
    ///
    /// 不按当前输入做相关性筛选：索引每条只占一行，全量带上成本很低，
    /// 而按分数挑选会让低于阈值的记忆彻底消失——那正是「明明记过却没生效」
    /// 的来源。正文改为让模型按需读取。
    ///
    /// 参数:
    /// - `_query`: 当前用户输入，全量注入下不参与筛选
    /// - `workspace`: 当前工作区路径；无工作区时只注入全局记忆
    ///
    /// 返回:
    /// - 注入文本；没有任何记忆时为 None
    pub fn recall_for_turn(&self, _query: &str, workspace: Option<&str>) -> Result<Option<String>> {
        if !self.config.enabled || !self.config.association_enabled {
            return Ok(None);
        }
        Ok(render_index_injection_for(&self.notes_dir, workspace))
    }

    /// 记录一批被压缩清出上下文的轮次。
    ///
    /// 参数:
    /// - `turns`: 逐出的轮次
    ///
    /// 返回:
    /// - 写入结果
    pub fn remember_evicted_turns(&self, turns: &[EvictedTurn]) -> Result<()> {
        if !self.config.enabled || !self.config.evicted_context_enabled {
            return Ok(());
        }
        self.evicted.remember(turns)
    }

    /// 清空逐出轮次记录。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 清空结果
    pub fn clear_evicted_context(&self) -> Result<()> {
        self.evicted.clear()
    }

    /// 检索逐出轮次。
    ///
    /// 参数:
    /// - `query`: 查询文本
    /// - `limit`: 返回条数上限
    ///
    /// 返回:
    /// - 检索结果的 JSON
    pub fn search_evicted_context(&self, query: &str, limit: usize) -> Result<Value> {
        self.evicted.search(query, limit, self.config.snippet_chars)
    }

    /// 只读地检索逐出轮次。
    ///
    /// 与写入路径同实现：底层检索本就不建表也不写入，两个入口的差别只剩
    /// 语义，保留是为了调用点读起来意图清楚。
    ///
    /// 参数:
    /// - `query`: 查询文本
    /// - `limit`: 返回条数上限
    ///
    /// 返回:
    /// - 检索结果的 JSON
    pub fn search_evicted_context_readonly(&self, query: &str, limit: usize) -> Result<Value> {
        self.search_evicted_context(query, limit)
    }

    /// 列出全部记忆。
    ///
    /// 参数:
    /// - `limit`: 返回条数上限
    ///
    /// 返回:
    /// - 记忆列表的 JSON
    pub fn list_entries(&self, limit: usize) -> Result<Value> {
        let workspace = crate::runtime_cwd::current_dir().ok();
        let mut entries: Vec<Value> = self
            .notes(workspace.as_deref())
            .list()?
            .into_iter()
            .map(|summary| {
                json!({
                    "name": summary.name,
                    "description": summary.description,
                    "type": summary.memory_type.as_str(),
                    "scope": scope_label(summary.scope),
                })
            })
            .collect();
        entries.truncate(limit);
        Ok(json!({ "ok": true, "count": entries.len(), "entries": entries }))
    }

    /// 删除一条记忆。
    ///
    /// 参数:
    /// - `name`: 记忆标识
    ///
    /// 返回:
    /// - 是否确实删除了一条
    pub fn delete_entry(&self, name: &str) -> Result<bool> {
        let workspace = crate::runtime_cwd::current_dir().ok();
        self.notes(workspace.as_deref()).delete(name)
    }

    /// 清空全部记忆与逐出记录。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 清空结果
    pub fn reset_all(&self) -> Result<()> {
        if self.notes_dir.is_dir() {
            std::fs::remove_dir_all(&self.notes_dir)?;
        }
        self.clear_evicted_context()
    }

    /// 汇总记忆与逐出记录的状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 状态 JSON
    pub fn stats(&self) -> Result<Value> {
        let workspace = crate::runtime_cwd::current_dir().ok();
        let summaries = self.notes(workspace.as_deref()).list()?;
        let project = summaries
            .iter()
            .filter(|summary| summary.scope == MemoryScope::Project)
            .count();
        Ok(json!({
            "ok": true,
            "notes_dir": self.notes_dir.display().to_string(),
            "memories": summaries.len(),
            "project_memories": project,
            "global_memories": summaries.len() - project,
            "evicted_turns": self.evicted.count()?,
            "storage": { "mode": "markdown_files" },
        }))
    }
}

/// 返回作用域的展示标识。
///
/// 参数:
/// - `scope`: 作用域
///
/// 返回:
/// - 小写标识
fn scope_label(scope: MemoryScope) -> &'static str {
    match scope {
        MemoryScope::Global => "global",
        MemoryScope::Project => "project",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::file_store::{Frontmatter, MemoryEntry, MemoryType};

    /// 构造记忆入口与工作区路径。
    ///
    /// 参数:
    /// - `root`: 临时根目录
    ///
    /// 返回:
    /// - （记忆入口，工作区路径）
    fn setup(root: &Path) -> (MemoryStore, PathBuf) {
        let paths = SaiPaths::for_tests(root);
        let store = MemoryStore::new(&AppConfig::default(), &paths);
        (store, root.join("project"))
    }

    /// 写入一条记忆。
    ///
    /// 参数:
    /// - `store`: 记忆入口
    /// - `workspace`: 工作区路径
    /// - `name`: 标识
    /// - `hook`: 索引提示
    ///
    /// 返回:
    /// - 无
    fn save(store: &MemoryStore, workspace: &Path, name: &str, hook: &str) {
        store
            .notes(Some(workspace))
            .save(
                MemoryScope::Project,
                &MemoryEntry {
                    front: Frontmatter {
                        name: name.to_string(),
                        description: format!("{name} 的摘要"),
                        memory_type: MemoryType::Feedback,
                    },
                    body: "正文".to_string(),
                },
                hook,
            )
            .unwrap();
    }

    /// 验证写入的记忆出现在注入文本里。
    ///
    /// 这条锁的是接线：写入走文件、召回走索引，两者中间隔着一次索引更新，
    /// 少了那一步记忆写进去了也召不回来。
    #[test]
    fn a_saved_memory_shows_up_in_the_injected_index() {
        let dir = tempfile::tempdir().unwrap();
        let (store, workspace) = setup(dir.path());
        save(&store, &workspace, "zh-writing", "中文书写规范");

        let injected = store
            .recall_for_turn("任意输入", Some(&workspace.display().to_string()))
            .unwrap()
            .unwrap();

        assert!(injected.contains("中文书写规范"));
    }

    /// 验证注入不按当前输入筛选。
    ///
    /// 全量注入是这套方案与相关性检索的根本差别：输入与记忆毫不相干时，
    /// 那条记忆同样必须出现，否则就退回了「明明记过却没生效」的旧行为。
    #[test]
    fn the_index_is_injected_regardless_of_the_input() {
        let dir = tempfile::tempdir().unwrap();
        let (store, workspace) = setup(dir.path());
        save(&store, &workspace, "pnpm-only", "包管理器选择");

        let injected = store
            .recall_for_turn("今天天气怎么样", Some(&workspace.display().to_string()))
            .unwrap()
            .unwrap();

        assert!(injected.contains("包管理器选择"));
    }

    /// 验证没有任何记忆时不产生注入。
    #[test]
    fn nothing_is_injected_without_memories() {
        let dir = tempfile::tempdir().unwrap();
        let (store, workspace) = setup(dir.path());

        let injected = store
            .recall_for_turn("任意", Some(&workspace.display().to_string()))
            .unwrap();

        assert!(injected.is_none());
    }

    /// 验证关闭注入开关后不再注入。
    #[test]
    fn the_injection_switch_is_honored() {
        let dir = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(dir.path());
        let mut config = AppConfig::default();
        config.plugins.memory.association_enabled = false;
        let store = MemoryStore::new(&config, &paths);
        let workspace = dir.path().join("project");
        save(&store, &workspace, "a", "提示");

        assert!(store
            .recall_for_turn("任意", Some(&workspace.display().to_string()))
            .unwrap()
            .is_none());
    }

    /// 验证压缩留档后能按关键词回读。
    ///
    /// 摘要末尾那句回读指引依赖这条链路；此前它从未被写入过，指引指向的是
    /// 一个永远为空的库。
    #[test]
    fn evicted_turns_can_be_searched_back() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = setup(dir.path());
        store
            .remember_evicted_turns(&[EvictedTurn {
                timestamp: "2026-01-01T00:00:00Z".to_string(),
                role: "user".to_string(),
                content: "把压缩改成前缀回放以复用供应商缓存".to_string(),
            }])
            .unwrap();

        let found = store.search_evicted_context("前缀回放", 5).unwrap();

        assert_eq!(found["results"].as_array().unwrap().len(), 1);
    }
}

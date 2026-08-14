use crate::config::{AppConfig, KnowledgeBasePluginConfig, MemoryConfig};
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
    kb_config: KnowledgeBasePluginConfig,
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
            kb_config: config.plugins.knowledge_base.clone(),
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
        self.evicted
            .search(query, limit, self.kb_config.snippet_context_chars)
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

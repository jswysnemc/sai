use crate::config::{AppConfig, KnowledgeBasePluginConfig, MemoryConfig};
use crate::paths::SaiPaths;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Clone)]
pub struct MemoryStore {
    config: MemoryConfig,
    kb_config: KnowledgeBasePluginConfig,
    data_db: PathBuf,
    state_db: PathBuf,
    /// Markdown memory source files (`facts/*.md`, `episodes/*.md`).
    files_dir: PathBuf,
    skills_dir: PathBuf,
    /// 统一记忆库；结构化写入与作用域感知召回走这一路
    library_db: PathBuf,
}

impl MemoryStore {
    pub fn new(config: &AppConfig, paths: &SaiPaths) -> Self {
        let data_dir = config.active_persona_memory_data_dir(paths).join("memory");
        let state_dir = config.active_persona_memory_state_dir(paths).join("memory");
        Self {
            config: config.memory_config().clone(),
            kb_config: config.plugins.knowledge_base.clone(),
            data_db: data_dir.join("memory.db"),
            state_db: state_dir.join("evicted_context.db"),
            files_dir: data_dir.join("files"),
            skills_dir: config.active_persona_skills_dir(paths),
            library_db: data_dir.join("library.db"),
        }
    }

    /// 返回统一记忆库句柄。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 绑定当前人格与半衰期配置的记忆库
    pub fn library(&self) -> crate::memory::MemoryLibrary {
        crate::memory::MemoryLibrary::new(&self.library_db, self.config.forgetting_half_life_days)
    }

    /// 按当前输入召回结构化记忆并渲染为注入文本。
    ///
    /// 参数:
    /// - `query`: 当前用户输入
    /// - `workspace`: 当前工作区路径；无工作区时为 None
    ///
    /// 返回:
    /// - 注入文本；没有足够相关的记忆时为 None
    pub fn recall_for_turn(&self, query: &str, workspace: Option<&str>) -> Result<Option<String>> {
        if !self.config.enabled || !self.config.association_enabled {
            return Ok(None);
        }
        self.library()
            .recall(query, workspace, crate::memory::now_days())
    }

    /// 写入一批抽取出的候选记忆。
    ///
    /// 参数:
    /// - `candidates`: 抽取模型给出的候选
    ///
    /// 返回:
    /// - 实际写入或更新的条数
    pub fn capture_candidates(
        &self,
        candidates: Vec<crate::memory::MemoryCandidate>,
    ) -> Result<usize> {
        if !self.config.enabled {
            return Ok(0);
        }
        self.library().capture(candidates, &now())
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

    pub fn init(&self) -> Result<()> {
        if let Some(parent) = self.data_db.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if let Some(parent) = self.state_db.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::create_dir_all(self.files_dir.join("facts"))?;
        std::fs::create_dir_all(self.files_dir.join("episodes"))?;
        init_data_db(&self.data_conn()?)?;
        init_state_db(&self.state_conn()?)?;
        self.ensure_markdown_and_fts()?;
        self.decay_memories()?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn remember_evicted_turns(&self, turns: &[EvictedTurn]) -> Result<()> {
        if !self.config.enabled || !self.config.evicted_context_enabled || turns.is_empty() {
            return Ok(());
        }
        self.init()?;
        let mut conn = self.state_conn()?;
        let tx = conn.transaction()?;
        for turn in turns {
            tx.execute(
                "INSERT INTO evicted_turns (timestamp, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![turn.timestamp, turn.role, turn.content, now()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn clear_evicted_context(&self) -> Result<()> {
        self.init()?;
        self.state_conn()?
            .execute("DELETE FROM evicted_turns", [])?;
        Ok(())
    }

    pub fn clear_pending_events(&self) -> Result<()> {
        self.init()?;
        let data = self.data_conn()?;
        data.execute("DELETE FROM pending_events", [])?;
        data.execute(
            "DELETE FROM sqlite_sequence WHERE name = 'pending_events'",
            [],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn search_evicted_context(&self, query: &str, limit: usize) -> Result<Value> {
        self.init()?;
        self.search_evicted_context_existing(query, limit)
    }

    pub fn search_evicted_context_readonly(&self, query: &str, limit: usize) -> Result<Value> {
        if !self.state_db.is_file() {
            return Ok(json!({ "ok": true, "query": query, "results": [] }));
        }
        self.search_evicted_context_existing(query, limit)
    }

    fn search_evicted_context_existing(&self, query: &str, limit: usize) -> Result<Value> {
        let tokens = query_tokens(query);
        let conn = self.state_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, role, content FROM evicted_turns ORDER BY id DESC LIMIT 1000",
        )?;
        let rows = stmt.query_map([], |row| {
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
                "snippet": snippet(&content, &tokens, self.kb_config.snippet_context_chars),
            }));
        }
        sort_json_hits(&mut hits);
        hits.truncate(limit.clamp(1, 50));
        Ok(json!({ "ok": true, "query": query, "results": hits }))
    }

    pub fn remember_fact(&self, content: &str, source: &str) -> Result<i64> {
        self.remember_fact_with_tags(content, source, &[])
    }

    /// 保存带标签的长期事实。
    ///
    /// 参数:
    /// - `content`: 事实正文
    /// - `source`: 来源标签
    /// - `tags`: 检索标签
    ///
    /// 返回:
    /// - 事实 ID
    pub fn remember_fact_with_tags(
        &self,
        content: &str,
        source: &str,
        tags: &[String],
    ) -> Result<i64> {
        if !self.config.enabled || content.trim().is_empty() {
            return Ok(0);
        }
        self.init()?;
        let conn = self.data_conn()?;
        let ts = now();
        let content = content.trim();
        let source = source.trim();
        let tags_text = normalize_tags(tags);
        conn.execute(
            "INSERT INTO facts (content, source, status, confidence, strength, recall_count, created_at, updated_at, tags) VALUES (?1, ?2, 'active', 1.0, 1.0, 0, ?3, ?3, ?4)",
            params![content, source, ts, tags_text],
        )?;
        let id = conn.last_insert_rowid();
        // FTS 同步写入标签，便于按标签检索
        let fts_body = if tags_text.is_empty() {
            content.to_string()
        } else {
            format!("{content}\n{tags_text}")
        };
        fts_upsert_row(&conn, "facts", id, &fts_body)?;
        write_memory_markdown(
            &self.files_dir,
            "facts",
            id,
            content,
            source,
            "active",
            Some(1.0),
            1.0,
            &ts,
            &ts,
            &tags_text,
        )?;
        Ok(id)
    }

    /// 轮次结束后的记忆维护。
    ///
    /// 长期记忆的写入已由 `spawn_memory_capture` 经统一记忆库完成——
    /// 那条链路用模型判断什么值得记，而这里的 pending_events 是按字符串
    /// 规则生成的对话流水账，两者并存只会污染召回。因此这里不再写入，
    /// 只保留方法本身供旧调用点使用。
    ///
    /// 参数:
    /// - `user_message`: 用户原文
    /// - `assistant_message`: 助手回复原文
    ///
    /// 返回:
    /// - 始终成功
    pub fn process_after_turn(&self, _user_message: &str, _assistant_message: &str) -> Result<()> {
        Ok(())
    }

    /// 列出事实与往事，供记忆管理界面使用。
    pub fn list_entries(&self, limit: usize) -> Result<Value> {
        self.init()?;
        let limit = limit.clamp(1, 500) as i64;
        let data = self.data_conn()?;
        let mut facts = Vec::new();
        {
            let mut stmt = data.prepare(
                "SELECT id, content, source, status, confidence, strength, recall_count, created_at, updated_at, tags
                 FROM facts ORDER BY updated_at DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                let tags_raw: String = row.get::<_, String>(9).unwrap_or_default();
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "kind": "fact",
                    "content": row.get::<_, String>(1)?,
                    "source": row.get::<_, String>(2)?,
                    "status": row.get::<_, String>(3)?,
                    "confidence": row.get::<_, f64>(4).unwrap_or(1.0),
                    "strength": row.get::<_, f64>(5).unwrap_or(1.0),
                    "recall_count": row.get::<_, i64>(6).unwrap_or(0),
                    "created_at": row.get::<_, String>(7)?,
                    "updated_at": row.get::<_, String>(8)?,
                    "tags": split_tags(&tags_raw),
                }))
            })?;
            for row in rows {
                facts.push(row?);
            }
        }
        let mut episodes = Vec::new();
        {
            let mut stmt = data.prepare(
                "SELECT id, content, source, status, strength, recall_count, created_at, updated_at
                 FROM episodes ORDER BY updated_at DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "kind": "episode",
                    "content": row.get::<_, String>(1)?,
                    "source": row.get::<_, String>(2)?,
                    "status": row.get::<_, String>(3)?,
                    "strength": row.get::<_, f64>(4).unwrap_or(1.0),
                    "recall_count": row.get::<_, i64>(5).unwrap_or(0),
                    "created_at": row.get::<_, String>(6)?,
                    "updated_at": row.get::<_, String>(7)?,
                }))
            })?;
            for row in rows {
                episodes.push(row?);
            }
        }
        for entry in facts.iter_mut().chain(episodes.iter_mut()) {
            attach_markdown_meta(entry, &self.files_dir);
        }
        Ok(json!({ "ok": true, "facts": facts, "episodes": episodes }))
    }

    /// 删除一条事实或往事。
    pub fn delete_entry(&self, kind: &str, id: i64) -> Result<bool> {
        self.init()?;
        let table = match kind {
            "fact" | "facts" => "facts",
            "episode" | "episodes" => "episodes",
            _ => anyhow::bail!("unsupported memory kind: {kind}"),
        };
        let conn = self.data_conn()?;
        let affected = conn.execute(&format!("DELETE FROM {table} WHERE id = ?1"), params![id])?;
        if affected > 0 {
            fts_delete_row(&conn, table, id)?;
            delete_memory_markdown(&self.files_dir, table, id)?;
        }
        Ok(affected > 0)
    }

    pub fn reset_all(&self, include_skills: bool) -> Result<()> {
        self.init()?;
        let data = self.data_conn()?;
        data.execute("DELETE FROM facts", [])?;
        data.execute("DELETE FROM episodes", [])?;
        data.execute("DELETE FROM pending_events", [])?;
        data.execute("DELETE FROM skill_records", [])?;
        data.execute(
            "DELETE FROM sqlite_sequence WHERE name IN ('facts', 'episodes', 'pending_events', 'skill_records')",
            [],
        )?;
        // Clear external-content FTS indexes.
        for table in [
            "facts_fts",
            "facts_fts_tri",
            "episodes_fts",
            "episodes_fts_tri",
        ] {
            let _ = data.execute(&format!("DELETE FROM {table}"), []);
        }
        clear_memory_markdown(&self.files_dir)?;
        self.clear_evicted_context()?;
        if include_skills {
            self.remove_auto_skills()?;
        }
        Ok(())
    }

    fn remove_auto_skills(&self) -> Result<()> {
        if !self.skills_dir.exists() {
            return Ok(());
        }
        for entry in std::fs::read_dir(&self.skills_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let skill_file = entry.path().join("SKILL.md");
            let raw = std::fs::read_to_string(&skill_file).unwrap_or_default();
            if raw.contains("Auto-learned method from assistant conversation")
                || raw.contains("Auto-learned method from Sai conversation")
                || raw.contains("generated_by: sai")
            {
                std::fs::remove_dir_all(entry.path())?;
            }
        }
        Ok(())
    }

    fn prune_missing_skill_records(&self) -> Result<()> {
        let conn = self.data_conn()?;
        let mut stmt = conn.prepare("SELECT id, path FROM skill_records")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut missing = Vec::new();
        for row in rows {
            let (id, path) = row?;
            if !PathBuf::from(path).exists() {
                missing.push(id);
            }
        }
        drop(stmt);
        for id in missing {
            conn.execute("DELETE FROM skill_records WHERE id=?1", params![id])?;
        }
        Ok(())
    }

    pub fn recall_memories(
        &self,
        query: &str,
        limit: usize,
        include_forgotten: bool,
    ) -> Result<Value> {
        self.init()?;
        self.recall_memories_existing(query, limit, include_forgotten)
    }

    pub fn recall_memories_readonly(
        &self,
        query: &str,
        limit: usize,
        include_forgotten: bool,
    ) -> Result<Value> {
        if !self.data_db.is_file() {
            return Ok(json!({ "ok": true, "query": query, "facts": [], "episodes": [] }));
        }
        self.recall_memories_existing(query, limit, include_forgotten)
    }

    fn recall_memories_existing(
        &self,
        query: &str,
        limit: usize,
        include_forgotten: bool,
    ) -> Result<Value> {
        let facts = self.search_facts(query, limit, include_forgotten)?;
        let episodes = self.search_episodes(query, limit, include_forgotten)?;
        Ok(json!({
            "ok": true,
            "query": query,
            "facts": facts.iter().map(memory_hit_json).collect::<Vec<_>>(),
            "episodes": episodes.iter().map(memory_hit_json).collect::<Vec<_>>(),
        }))
    }

    #[allow(dead_code)]
    pub fn recall_past_events(&self, query: &str, limit: usize) -> Result<Value> {
        self.init()?;
        self.recall_past_events_existing(query, limit)
    }

    pub fn recall_past_events_readonly(&self, query: &str, limit: usize) -> Result<Value> {
        if !self.data_db.is_file() {
            return Ok(json!({ "ok": true, "query": query, "episodes": [] }));
        }
        self.recall_past_events_existing(query, limit)
    }

    fn recall_past_events_existing(&self, query: &str, limit: usize) -> Result<Value> {
        let episodes = self.search_episodes(query, limit, true)?;
        Ok(json!({
            "ok": true,
            "query": query,
            "episodes": episodes.iter().map(memory_hit_json).collect::<Vec<_>>(),
        }))
    }

    pub fn association(&self, query: &str) -> Result<Option<AssociationContext>> {
        if !self.config.enabled || !self.config.association_enabled {
            return Ok(None);
        }
        self.init()?;
        let facts = self.search_facts(query, self.config.association_facts, false)?;
        let episodes = self.search_episodes(query, self.config.association_episodes, false)?;
        for hit in facts.iter().chain(episodes.iter()) {
            self.reinforce(hit.id, &hit.source)?;
        }
        if facts.is_empty() && episodes.is_empty() {
            return Ok(None);
        }
        Ok(Some(AssociationContext { facts, episodes }))
    }

    pub fn format_association(&self, association: &AssociationContext) -> String {
        let mut output = String::new();
        output.push_str("<associative-memory>\n");
        output.push_str("以下是根据当前用户输入联想到的旧记忆，可能相关也可能不相关；必要时使用，不要强行引用。\n");
        if !association.facts.is_empty() {
            output.push_str("\n曾经记住的相关知识点：\n");
            for hit in &association.facts {
                output.push_str("- ");
                output.push_str(&compact_line(&hit.content));
                output.push('\n');
            }
        }
        if !association.episodes.is_empty() {
            output.push_str("\n曾经发生的事情：\n");
            for hit in &association.episodes {
                output.push_str("- ");
                output.push_str(&compact_line(&hit.content));
                output.push('\n');
            }
        }
        output.push_str("</associative-memory>");
        truncate_chars(&output, self.config.association_max_chars)
    }

    fn search_facts(
        &self,
        query: &str,
        limit: usize,
        include_forgotten: bool,
    ) -> Result<Vec<MemoryHit>> {
        self.search_table("facts", query, limit, include_forgotten)
    }

    fn search_episodes(
        &self,
        query: &str,
        limit: usize,
        include_forgotten: bool,
    ) -> Result<Vec<MemoryHit>> {
        self.search_table("episodes", query, limit, include_forgotten)
    }

    fn search_table(
        &self,
        table: &str,
        query: &str,
        limit: usize,
        include_forgotten: bool,
    ) -> Result<Vec<MemoryHit>> {
        let tokens = query_tokens(query);
        let conn = self.data_conn()?;
        let mut hits = Vec::new();
        let fts_match = fts_query_terms(query);
        if !fts_match.is_empty() {
            self.search_table_fts(
                &conn,
                table,
                &fts_match,
                include_forgotten,
                contains_cjk(query),
                &mut hits,
            )?;
        }
        // Keyword fallback keeps recall working when FTS returns nothing (short tokens, etc.).
        if hits.is_empty() {
            let has_tags = table == "facts";
            let sql = if has_tags {
                format!(
                    "SELECT id, content, source, status, created_at, tags FROM {table} ORDER BY updated_at DESC LIMIT 1000"
                )
            } else {
                format!(
                    "SELECT id, content, source, status, created_at, '' as tags FROM {table} ORDER BY updated_at DESC LIMIT 1000"
                )
            };
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5).unwrap_or_default(),
                ))
            })?;
            for row in rows {
                let (id, content, source, status, timestamp, tags_raw) = row?;
                if !include_forgotten && status == "forgotten" {
                    continue;
                }
                let tags = split_tags(&tags_raw);
                let searchable = if tags_raw.is_empty() {
                    content.clone()
                } else {
                    format!("{content} {tags_raw}")
                };
                let mut score = score_text(&searchable, &tokens);
                // 标签精确匹配加权
                for token in &tokens {
                    if tags.iter().any(|tag| tag.eq_ignore_ascii_case(token)) {
                        score += 25.0;
                    }
                }
                if score <= 0.0 {
                    continue;
                }
                hits.push(MemoryHit {
                    id,
                    content,
                    score,
                    timestamp,
                    source,
                    tags,
                });
            }
        }
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(limit.clamp(1, 50));
        Ok(hits)
    }

    fn search_table_fts(
        &self,
        conn: &Connection,
        table: &str,
        fts_match: &str,
        include_forgotten: bool,
        prefer_trigram: bool,
        hits: &mut Vec<MemoryHit>,
    ) -> Result<()> {
        let tables = if prefer_trigram {
            [format!("{table}_fts_tri"), format!("{table}_fts")]
        } else {
            [format!("{table}_fts"), format!("{table}_fts_tri")]
        };
        let mut seen = HashSet::new();
        for fts_table in tables {
            let tags_expr = if table == "facts" { "m.tags" } else { "''" };
            let sql = format!(
                "SELECT m.id, m.content, m.source, m.status, m.created_at, bm25({fts_table}) AS rank, {tags_expr}
                 FROM {fts_table}
                 JOIN {table} m ON m.id = {fts_table}.rowid
                 WHERE {fts_table} MATCH ?1
                 LIMIT 64"
            );
            let mut stmt = match conn.prepare(&sql) {
                Ok(stmt) => stmt,
                Err(_) => continue,
            };
            let rows = stmt.query_map(params![fts_match], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, f64>(5).unwrap_or(0.0),
                    row.get::<_, String>(6).unwrap_or_default(),
                ))
            });
            let Ok(rows) = rows else {
                continue;
            };
            for row in rows {
                let (id, content, source, status, timestamp, rank, tags_raw) = row?;
                if !include_forgotten && status == "forgotten" {
                    continue;
                }
                if !seen.insert(id) {
                    continue;
                }
                // bm25 is lower-is-better; invert into a positive score.
                let mut score = 100.0 / (1.0 + rank.max(0.0) as f32);
                let tags = split_tags(&tags_raw);
                for tag in &tags {
                    if fts_match
                        .to_ascii_lowercase()
                        .contains(&tag.to_ascii_lowercase())
                    {
                        score += 15.0;
                    }
                }
                hits.push(MemoryHit {
                    id,
                    content,
                    score,
                    timestamp,
                    source,
                    tags,
                });
            }
        }
        Ok(())
    }
}

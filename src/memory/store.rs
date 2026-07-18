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
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct EvictedTurn {
    pub timestamp: String,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct AssociationContext {
    pub facts: Vec<MemoryHit>,
    pub episodes: Vec<MemoryHit>,
}

#[derive(Debug, Clone)]
pub struct MemoryHit {
    pub id: i64,
    pub content: String,
    pub score: f32,
    pub timestamp: String,
    pub source: String,
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
        }
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
        if !self.config.enabled || content.trim().is_empty() {
            return Ok(0);
        }
        self.init()?;
        let conn = self.data_conn()?;
        let ts = now();
        let content = content.trim();
        let source = source.trim();
        conn.execute(
            "INSERT INTO facts (content, source, status, confidence, strength, recall_count, created_at, updated_at) VALUES (?1, ?2, 'active', 1.0, 1.0, 0, ?3, ?3)",
            params![content, source, ts],
        )?;
        let id = conn.last_insert_rowid();
        fts_upsert_row(&conn, "facts", id, content)?;
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
        )?;
        Ok(id)
    }

    pub fn remember_pending_event(
        &self,
        user_message: &str,
        assistant_message: &str,
    ) -> Result<()> {
        if !self.config.enabled || !self.config.auto_diary_enabled {
            return Ok(());
        }
        self.init()?;
        self.data_conn()?.execute(
            "INSERT INTO pending_events (user_message, assistant_message, created_at) VALUES (?1, ?2, ?3)",
            params![user_message.trim(), assistant_message.trim(), now()],
        )?;
        Ok(())
    }

    pub fn process_after_turn(&self, user_message: &str, assistant_message: &str) -> Result<()> {
        self.remember_pending_event(user_message, assistant_message)?;
        self.flush_pending_events()?;
        Ok(())
    }

    pub fn stats(&self) -> Result<Value> {
        self.init()?;
        self.prune_missing_skill_records()?;
        let data = self.data_conn()?;
        let state = self.state_conn()?;
        Ok(json!({
            "ok": true,
            "data_db": self.data_db.display().to_string(),
            "state_db": self.state_db.display().to_string(),
            "files_dir": self.files_dir.display().to_string(),
            "skills_dir": self.skills_dir.display().to_string(),
            "facts": count_rows(&data, "facts")?,
            "episodes": count_rows(&data, "episodes")?,
            "unprocessed_pending_events": count_where(&data, "pending_events", "processed_at IS NULL")?,
            "total_pending_events": count_rows(&data, "pending_events")?,
            "skill_records": count_rows(&data, "skill_records")?,
            "skill_dirs": count_skill_dirs(&self.skills_dir)?,
            "evicted_turns": count_rows(&state, "evicted_turns")?,
        }))
    }

    /// 列出事实与往事，供记忆管理界面使用。
    pub fn list_entries(&self, limit: usize) -> Result<Value> {
        self.init()?;
        let limit = limit.clamp(1, 500) as i64;
        let data = self.data_conn()?;
        let mut facts = Vec::new();
        {
            let mut stmt = data.prepare(
                "SELECT id, content, source, status, confidence, strength, recall_count, created_at, updated_at
                 FROM facts ORDER BY updated_at DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], |row| {
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
        for table in ["facts_fts", "facts_fts_tri", "episodes_fts", "episodes_fts_tri"] {
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

    fn flush_pending_events(&self) -> Result<()> {
        if !self.config.enabled || !self.config.auto_diary_enabled {
            return Ok(());
        }
        self.init()?;
        let conn = self.data_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, user_message, assistant_message, created_at FROM pending_events WHERE processed_at IS NULL ORDER BY id LIMIT 20",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (id, user, assistant, created_at) = row?;
            let content = format!(
                "{}，我被要求：{}；结果：{}",
                created_at,
                truncate_chars(&compact_line(&user), 260),
                truncate_chars(&compact_line(&assistant), 520)
            );
            conn.execute(
                "INSERT INTO episodes (content, source, status, strength, recall_count, created_at, updated_at) VALUES (?1, 'episode', 'active', 1.0, 0, ?2, ?2)",
                params![content, created_at],
            )?;
            let episode_id = conn.last_insert_rowid();
            fts_upsert_row(&conn, "episodes", episode_id, &content)?;
            write_memory_markdown(
                &self.files_dir,
                "episodes",
                episode_id,
                &content,
                "episode",
                "active",
                None,
                1.0,
                &created_at,
                &created_at,
            )?;
            conn.execute(
                "UPDATE pending_events SET processed_at=?1 WHERE id=?2",
                params![now(), id],
            )?;
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
            let sql = format!(
                "SELECT id, content, source, status, created_at FROM {table} ORDER BY updated_at DESC LIMIT 1000"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?;
            for row in rows {
                let (id, content, source, status, timestamp) = row?;
                if !include_forgotten && status == "forgotten" {
                    continue;
                }
                let score = score_text(&content, &tokens);
                if score <= 0.0 {
                    continue;
                }
                hits.push(MemoryHit {
                    id,
                    content,
                    score,
                    timestamp,
                    source,
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
            let sql = format!(
                "SELECT m.id, m.content, m.source, m.status, m.created_at, bm25({fts_table}) AS rank
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
                ))
            });
            let Ok(rows) = rows else {
                continue;
            };
            for row in rows {
                let (id, content, source, status, timestamp, rank) = row?;
                if !include_forgotten && status == "forgotten" {
                    continue;
                }
                if !seen.insert(id) {
                    continue;
                }
                // bm25 is lower-is-better; invert into a positive score.
                let score = 100.0 / (1.0 + rank.max(0.0) as f32);
                hits.push(MemoryHit {
                    id,
                    content,
                    score,
                    timestamp,
                    source,
                });
            }
        }
        Ok(())
    }

    /// Ensure markdown files exist for SQL rows and FTS indexes are populated.
    fn ensure_markdown_and_fts(&self) -> Result<()> {
        let conn = self.data_conn()?;
        for table in ["facts", "episodes"] {
            let sql = if table == "facts" {
                "SELECT id, content, source, status, confidence, strength, created_at, updated_at FROM facts"
            } else {
                "SELECT id, content, source, status, 1.0, strength, created_at, updated_at FROM episodes"
            };
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, f64>(4).unwrap_or(1.0),
                    row.get::<_, f64>(5).unwrap_or(1.0),
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })?;
            for row in rows {
                let (id, content, source, status, confidence, strength, created_at, updated_at) =
                    row?;
                let path = self.files_dir.join(table).join(format!("{id}.md"));
                if !path.is_file() {
                    write_memory_markdown(
                        &self.files_dir,
                        table,
                        id,
                        &content,
                        &source,
                        &status,
                        if table == "facts" {
                            Some(confidence)
                        } else {
                            None
                        },
                        strength,
                        &created_at,
                        &updated_at,
                    )?;
                }
            }
            drop(stmt);
            let base_count: i64 = conn.query_row(
                &format!("SELECT COUNT(*) FROM {table}"),
                [],
                |row| row.get(0),
            )?;
            let fts_count: i64 = conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table}_fts"),
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            if base_count > 0 && fts_count == 0 {
                rebuild_fts_table(&conn, table)?;
            }
        }
        Ok(())
    }

    fn reinforce(&self, id: i64, source: &str) -> Result<()> {
        let table = if source == "episode" {
            "episodes"
        } else {
            "facts"
        };
        let sql = format!(
            "UPDATE {table} SET recall_count=recall_count+1, strength=MIN(1.0, strength+?1), last_recalled_at=?2, updated_at=?2, status='active' WHERE id=?3"
        );
        self.data_conn()?.execute(
            &sql,
            params![self.config.forgetting_review_boost, now(), id],
        )?;
        Ok(())
    }

    fn decay_memories(&self) -> Result<()> {
        if !self.config.enabled || !self.config.forgetting_enabled {
            return Ok(());
        }
        let conn = self.data_conn()?;
        decay_table(&conn, "facts", &self.config)?;
        decay_table(&conn, "episodes", &self.config)?;
        Ok(())
    }

    fn data_conn(&self) -> Result<Connection> {
        Ok(Connection::open(&self.data_db)?)
    }

    fn state_conn(&self) -> Result<Connection> {
        Ok(Connection::open(&self.state_db)?)
    }
}

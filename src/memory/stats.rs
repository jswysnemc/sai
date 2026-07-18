impl MemoryStore {
    /// 汇总记忆数据库、Markdown 文件和 FTS 索引状态。
    ///
    /// 返回:
    /// - 记忆数量、存储路径及索引状态组成的 JSON 对象
    pub fn stats(&self) -> Result<Value> {
        self.init()?;
        self.prune_missing_skill_records()?;
        let data = self.data_conn()?;
        let state = self.state_conn()?;
        let facts = count_rows(&data, "facts")?;
        let episodes = count_rows(&data, "episodes")?;

        Ok(json!({
            "ok": true,
            "data_db": self.data_db.display().to_string(),
            "state_db": self.state_db.display().to_string(),
            "files_dir": self.files_dir.display().to_string(),
            "skills_dir": self.skills_dir.display().to_string(),
            "facts": facts,
            "episodes": episodes,
            "unprocessed_pending_events": count_where(&data, "pending_events", "processed_at IS NULL")?,
            "total_pending_events": count_rows(&data, "pending_events")?,
            "skill_records": count_rows(&data, "skill_records")?,
            "skill_dirs": count_skill_dirs(&self.skills_dir)?,
            "evicted_turns": count_rows(&state, "evicted_turns")?,
            // Markdown 源与 FTS 索引状态，供记忆管理界面展示
            "storage": {
                "mode": "markdown+sqlite_fts",
                "markdown_facts": count_markdown_files(&self.files_dir, "facts")?,
                "markdown_episodes": count_markdown_files(&self.files_dir, "episodes")?,
                "fts": {
                    "facts": count_fts_rows(&data, "facts_fts")?,
                    "facts_trigram": count_fts_rows(&data, "facts_fts_tri")?,
                    "episodes": count_fts_rows(&data, "episodes_fts")?,
                    "episodes_trigram": count_fts_rows(&data, "episodes_fts_tri")?,
                    "ready": fts_ready(&data, facts, episodes)?,
                },
            },
        }))
    }
}

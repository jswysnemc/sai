pub struct KnowledgeBase {
    config: AppConfig,
    root: PathBuf,
    files_dir: PathBuf,
    meta_db: PathBuf,
    semantic_db: PathBuf,
}

impl KnowledgeBase {
    pub fn new(config: AppConfig, paths: SaiPaths) -> Result<Self> {
        let root = kb_root(&config.plugins.knowledge_base, &paths);
        let files_dir = root.join("files");
        let meta_db = root.join("kb_meta.db");
        let semantic_db = root.join("semantic_index.db");
        Ok(Self {
            config,
            root,
            files_dir,
            meta_db,
            semantic_db,
        })
    }

    pub fn init(&self) -> Result<()> {
        std::fs::create_dir_all(&self.files_dir)?;
        let conn = self.meta_conn()?;
        init_meta_db(&conn)?;
        let semantic = self.semantic_conn()?;
        init_semantic_db(&semantic)?;
        Ok(())
    }

    fn readonly_available(&self) -> bool {
        self.root.is_dir() && self.files_dir.is_dir() && self.meta_db.is_file()
    }

    pub async fn add_path(&self, source: &Path) -> Result<Vec<String>> {
        self.init()?;
        let mut added = Vec::new();
        if source.is_dir() {
            let root_name = source
                .file_name()
                .and_then(|name| name.to_str())
                .context("source directory has no valid directory name")?;
            for file in collect_files(source)? {
                let rel = file.strip_prefix(source).unwrap_or(&file);
                let name = normalize_relative_path(&format!(
                    "{}/{}",
                    root_name,
                    rel.display().to_string().replace('\\', "/")
                ))?;
                if let Ok(name) = self.import_file(&file, &name) {
                    added.push(name);
                }
            }
        } else {
            let name = normalize_relative_path(
                source
                    .file_name()
                    .and_then(|name| name.to_str())
                    .context("source file has no valid file name")?,
            )?;
            added.push(self.import_file(source, &name)?);
        }
        self.spawn_embedding_reindex()?;
        Ok(added)
    }

    pub fn list(&self) -> Result<Vec<FileRecord>> {
        self.init()?;
        self.list_existing()
    }

    fn list_existing(&self) -> Result<Vec<FileRecord>> {
        let conn = self.meta_conn()?;
        let mut stmt =
            conn.prepare("SELECT name, path, size_bytes, content_sha256 FROM files ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok(FileRecord {
                name: row.get(0)?,
                path: row.get(1)?,
                size_bytes: row.get(2)?,
                content_sha256: row.get(3)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub async fn search(&self, query: &str, max_results: Option<usize>) -> Result<Value> {
        self.init()?;
        self.search_existing(query, max_results, true).await
    }

    pub async fn search_readonly(&self, query: &str, max_results: Option<usize>) -> Result<Value> {
        if !self.readonly_available() {
            return Ok(
                json!({"ok": true, "query": query, "total_matches": 0, "semantic_used": false, "results": []}),
            );
        }
        self.search_existing(query, max_results, self.semantic_db.is_file())
            .await
    }

    async fn search_existing(
        &self,
        query: &str,
        max_results: Option<usize>,
        allow_semantic: bool,
    ) -> Result<Value> {
        let limit = max_results
            .unwrap_or(self.config.plugins.knowledge_base.max_search_results)
            .clamp(1, 50);
        let mut results = self.keyword_search(query, limit)?;
        let strongest = results.first().map(|item| item.score).unwrap_or(0.0);
        let mut semantic_used = false;
        if allow_semantic
            && self.config.plugins.knowledge_base.embedding_enabled
            && strongest
                < self
                    .config
                    .plugins
                    .knowledge_base
                    .keyword_strong_score_threshold
        {
            if let Ok(semantic) = self.semantic_search(query).await {
                semantic_used = !semantic.is_empty();
                merge_results(&mut results, semantic, limit);
            }
        }
        Ok(json!({
            "ok": true,
            "query": query,
            "total_matches": results.len(),
            "semantic_used": semantic_used,
            "results": results.iter().map(SearchResult::to_json).collect::<Vec<_>>(),
        }))
    }

    pub fn find_by_name(&self, query: &str, max_results: Option<usize>) -> Result<Value> {
        self.init()?;
        self.find_by_name_existing(query, max_results)
    }

    pub fn find_by_name_readonly(&self, query: &str, max_results: Option<usize>) -> Result<Value> {
        if !self.readonly_available() {
            return Ok(json!({"ok": true, "query": query, "total_matches": 0, "results": []}));
        }
        self.find_by_name_existing(query, max_results)
    }

    fn find_by_name_existing(&self, query: &str, max_results: Option<usize>) -> Result<Value> {
        let limit = max_results
            .unwrap_or(self.config.plugins.knowledge_base.max_search_results)
            .clamp(1, 50);
        let mut results = Vec::new();
        for record in self.list()? {
            let (score, reason) = score_file_name(query, &record.name);
            if score <= 0.0 {
                continue;
            }
            results.push(json!({
                "path": record.name,
                "name": file_name(&record.name),
                "directory": directory_name(&record.name),
                "score": score,
                "match_reason": reason,
                "size_kb": (record.size_bytes as f64 / 1024.0 * 10.0).round() / 10.0,
            }));
        }
        results.sort_by(|a, b| {
            b.get("score")
                .and_then(Value::as_f64)
                .unwrap_or_default()
                .partial_cmp(&a.get("score").and_then(Value::as_f64).unwrap_or_default())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(json!({
            "ok": true,
            "query": query,
            "total_matches": results.len(),
            "results": results,
        }))
    }

    pub fn read_file(
        &self,
        name: &str,
        start_line: usize,
        max_lines: Option<usize>,
    ) -> Result<String> {
        self.init()?;
        self.read_file_existing(name, start_line, max_lines, true)
    }

    pub fn read_file_readonly(
        &self,
        name: &str,
        start_line: usize,
        max_lines: Option<usize>,
    ) -> Result<String> {
        if !self.readonly_available() {
            bail!("knowledge base is not initialized")
        }
        self.read_file_existing(name, start_line, max_lines, false)
    }

    fn read_file_existing(
        &self,
        name: &str,
        start_line: usize,
        max_lines: Option<usize>,
        create_parent: bool,
    ) -> Result<String> {
        let rel = normalize_relative_path(name)?;
        let path = if create_parent {
            self.safe_file_path(&rel)?
        } else {
            self.existing_file_path(&rel)?
        };
        if !path.exists() {
            bail!("knowledge base file not found: {rel}")
        }
        let content = std::fs::read_to_string(&path)?;
        let start = start_line.max(1);
        let max_lines = max_lines
            .unwrap_or(self.config.plugins.knowledge_base.max_read_lines)
            .clamp(1, 5000);
        // 1. 单次遍历同时统计总行数和收集目标分页，避免为整个文件分配行指针数组
        let mut total = 0usize;
        let mut selected = Vec::with_capacity(max_lines);
        for (index, line) in content.lines().enumerate() {
            let line_number = index + 1;
            total = line_number;
            if line_number >= start && selected.len() < max_lines {
                selected.push(line);
            }
        }
        if start > total.max(1) {
            return Ok(format!(
                "=== {rel} | start_line {start} out of range / {total} lines ==="
            ));
        }
        let end = (start + max_lines - 1).min(total);
        let mut output = format!("=== {rel} | lines {start}-{end} / {total} ===\n");
        output.push_str(&selected.join("\n"));
        if end < total {
            output.push_str(&format!(
                "\n\n... {remaining} more lines; continue with start_line={next}",
                remaining = total - end,
                next = end + 1
            ));
        }
        Ok(output)
    }

    pub fn remove(&self, name: &str) -> Result<()> {
        self.init()?;
        let rel = normalize_relative_path(name)?;
        let path = self.safe_file_path(&rel)?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        let conn = self.meta_conn()?;
        conn.execute("DELETE FROM files WHERE name=?1", params![rel])?;
        let semantic = self.semantic_conn()?;
        semantic.execute(
            "DELETE FROM semantic_chunks WHERE file_name=?1",
            params![rel],
        )?;
        Ok(())
    }

    pub fn edit_lines(
        &self,
        name: &str,
        start_line: usize,
        end_line: usize,
        replacement: &str,
    ) -> Result<EditResult> {
        self.init()?;
        let rel = normalize_relative_path(name)?;
        if start_line == 0 || end_line == 0 {
            bail!("line numbers must be 1-based")
        }
        if start_line > end_line {
            bail!("start_line must be less than or equal to end_line")
        }
        let path = self.existing_file_path(&rel)?;
        if !path.exists() {
            bail!("knowledge base file not found: {rel}")
        }
        let original = std::fs::read_to_string(&path)?;
        let had_trailing_newline = original.ends_with('\n');
        let mut lines = original.lines().map(str::to_string).collect::<Vec<_>>();
        let total_lines = lines.len();
        if start_line > total_lines || end_line > total_lines {
            bail!("line range {start_line}-{end_line} out of range: {total_lines} lines")
        }
        let replacement = replacement.replace("\r\n", "\n").replace('\r', "\n");
        let replacement_lines = if replacement.is_empty() {
            Vec::new()
        } else {
            replacement.lines().map(str::to_string).collect::<Vec<_>>()
        };
        lines.splice(start_line - 1..end_line, replacement_lines);
        let mut updated = lines.join("\n");
        if had_trailing_newline && !updated.is_empty() {
            updated.push('\n');
        }
        let temp = tempfile::NamedTempFile::new()?;
        std::fs::write(temp.path(), updated.as_bytes())?;
        self.import_file(temp.path(), &rel)?;
        let semantic_refreshed = self.refresh_semantic_after_write(&rel)?;
        Ok(EditResult {
            path: rel,
            old_line_count: total_lines,
            new_line_count: lines.len(),
            semantic_refreshed,
        })
    }


    pub async fn reindex_embeddings(&self, quiet: bool) -> Result<usize> {
        self.init()?;
        if !self.config.plugins.knowledge_base.embedding_enabled {
            if !quiet {
                println!("embedding is disabled");
            }
            return Ok(0);
        }
        let Some((provider, model)) = self.embedding_provider()? else {
            if !quiet {
                println!("embedding provider/model is not configured; skipped");
            }
            return Ok(0);
        };
        let lock_path = self.root.join("embedding.lock");
        let lock = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(lock) => lock,
            Err(_) => {
                if !quiet {
                    println!(
                        "embedding reindex already running; lock file: {}",
                        lock_path.display()
                    );
                    println!(
                        "if no sai reindex process is running, remove the stale lock file and retry"
                    );
                }
                return Ok(0);
            }
        };
        drop(lock);
        let result = self
            .reindex_embeddings_inner(&provider, &model, quiet)
            .await;
        let _ = std::fs::remove_file(lock_path);
        result
    }

    pub fn stats(&self) -> Result<Value> {
        self.init()?;
        let files = self.list()?;
        let semantic = self.semantic_conn()?;
        let chunks: i64 =
            semantic.query_row("SELECT COUNT(*) FROM semantic_chunks", [], |row| row.get(0))?;
        Ok(json!({
            "ok": true,
            "root": self.root.display().to_string(),
            "files_dir": self.files_dir.display().to_string(),
            "files": files.len(),
            "total_size_kb": (files.iter().map(|file| file.size_bytes).sum::<i64>() as f64 / 1024.0 * 10.0).round() / 10.0,
            "semantic_chunks": chunks,
            "embedding_enabled": self.config.plugins.knowledge_base.embedding_enabled,
            "embedding_provider_id": self.config.plugins.knowledge_base.embedding_provider_id,
            "embedding_model": self.config.plugins.knowledge_base.embedding_model,
        }))
    }

    fn import_file(&self, source: &Path, name: &str) -> Result<String> {
        let bytes = std::fs::read(source)?;
        self.validate_file(name, &bytes)?;
        let dest = self.safe_file_path(name)?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, &bytes)?;
        let hash = sha256_hex(&bytes);
        let mtime = unix_time(std::fs::metadata(&dest)?.modified()?);
        let conn = self.meta_conn()?;
        init_meta_db(&conn)?;
        conn.execute(
            "INSERT INTO files (name, path, size_bytes, mtime, content_sha256, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(name) DO UPDATE SET path=excluded.path, size_bytes=excluded.size_bytes, mtime=excluded.mtime, content_sha256=excluded.content_sha256, updated_at=excluded.updated_at",
            params![name, dest.display().to_string(), bytes.len() as i64, mtime, hash, now_secs()],
        )?;
        Ok(name.to_string())
    }

    fn refresh_semantic_after_write(&self, name: &str) -> Result<bool> {
        if !self.config.plugins.knowledge_base.embedding_enabled {
            return Ok(false);
        }
        self.semantic_conn()?.execute(
            "DELETE FROM semantic_chunks WHERE file_name=?1",
            params![name],
        )?;
        self.spawn_embedding_reindex()?;
        Ok(true)
    }

    fn keyword_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let tokens = query_tokens(query);
        let phrase = query.to_ascii_lowercase();
        let mut results = Vec::new();
        for record in self.list()? {
            let path = PathBuf::from(&record.path);
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let content_lower = content.to_ascii_lowercase();
            let name_lower = record.name.to_ascii_lowercase();
            let mut score = 0.0;
            let mut positions_by_token: HashMap<String, Vec<usize>> = HashMap::new();
            let mut matched = HashSet::new();
            if phrase.len() > 1 && content_lower.contains(&phrase) {
                score += 90.0;
                matched.insert(phrase.clone());
            }
            if phrase.len() > 1 && name_lower.contains(&phrase) {
                score += 140.0;
            }
            for token in &tokens {
                let positions = find_positions(&content_lower, token, 100);
                if !positions.is_empty() {
                    score += 20.0 + positions.len().min(10) as f32 * 2.0;
                    matched.insert(token.clone());
                    positions_by_token.insert(token.clone(), positions);
                }
                if name_lower.contains(token) {
                    score += 45.0;
                    matched.insert(token.clone());
                }
            }
            if !tokens.is_empty() {
                score += (matched.len() as f32 / tokens.len() as f32) * 55.0;
            }
            if let Some((start, end, coverage)) = best_window(
                &positions_by_token,
                &tokens,
                self.config.plugins.knowledge_base.proximity_window_chars,
            ) {
                score += coverage * 120.0;
                let snippet = snippet_chars(
                    &content,
                    start,
                    end,
                    self.config.plugins.knowledge_base.snippet_context_chars,
                );
                results.push(SearchResult::new(
                    record.name,
                    score,
                    vec![snippet],
                    "keyword",
                ));
                continue;
            }
            if score > 0.0 {
                let snippets = extract_snippets(
                    &content,
                    &content_lower,
                    &tokens,
                    self.config.plugins.knowledge_base.snippet_context_chars,
                );
                results.push(SearchResult::new(record.name, score, snippets, "keyword"));
            }
        }
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
    }

    async fn semantic_search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let Some((provider, model)) = self.embedding_provider()? else {
            return Ok(Vec::new());
        };
        let query_embedding = embed_text(&self.config, &provider, &model, query).await?;
        let semantic = self.semantic_conn()?;
        let mut stmt = semantic.prepare(
            "SELECT file_name, start_char, end_char, text, embedding_json FROM semantic_chunks",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, usize>(1)?,
                row.get::<_, usize>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let mut results = Vec::new();
        for row in rows {
            let (file_name, _start, _end, text, embedding_json) = row?;
            let Ok(embedding) = serde_json::from_str::<Vec<f32>>(&embedding_json) else {
                continue;
            };
            let score = cosine(&query_embedding, &embedding);
            if score < self.config.plugins.knowledge_base.semantic_min_score {
                continue;
            }
            results.push(SearchResult::new(
                file_name,
                score * 200.0,
                vec![compact_whitespace(&text)],
                "semantic",
            ));
        }
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(self.config.plugins.knowledge_base.semantic_top_k);
        Ok(results)
    }

    async fn reindex_embeddings_inner(
        &self,
        provider: &ProviderConfig,
        model: &str,
        quiet: bool,
    ) -> Result<usize> {
        let files = self.list()?;
        let semantic = self.semantic_conn()?;
        init_semantic_db(&semantic)?;
        let mut indexed = 0usize;
        for record in files {
            let content = match std::fs::read_to_string(&record.path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            let chunks = build_chunks(
                &content,
                self.config.plugins.knowledge_base.semantic_chunk_chars,
                self.config.plugins.knowledge_base.semantic_chunk_overlap,
            );
            semantic.execute(
                "DELETE FROM semantic_chunks WHERE file_name=?1",
                params![record.name],
            )?;
            for chunk in chunks {
                let embedding = match embed_text(&self.config, provider, model, &chunk.text).await {
                    Ok(value) => value,
                    Err(err) => {
                        if !quiet {
                            eprintln!(
                                "embedding failed for {} chunk {}: {err}",
                                record.name, chunk.index
                            );
                        }
                        continue;
                    }
                };
                semantic.execute(
                    "INSERT INTO semantic_chunks (provider_id, model, file_name, content_sha256, chunk_index, start_char, end_char, text, embedding_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![provider.id, model, record.name, record.content_sha256, chunk.index as i64, chunk.start as i64, chunk.end as i64, chunk.text, serde_json::to_string(&embedding)?, now_secs()],
                )?;
                indexed += 1;
            }
        }
        if !quiet {
            println!("indexed semantic chunks: {indexed}");
        }
        Ok(indexed)
    }

    fn spawn_embedding_reindex(&self) -> Result<()> {
        if !self.config.plugins.knowledge_base.embedding_enabled {
            return Ok(());
        }
        if self
            .config
            .plugins
            .knowledge_base
            .embedding_provider_id
            .trim()
            .is_empty()
            || self
                .config
                .plugins
                .knowledge_base
                .embedding_model
                .trim()
                .is_empty()
        {
            return Ok(());
        }
        let exe = std::env::current_exe()?;
        Command::new(exe)
            .args(["kb", "embed", "reindex", "--quiet"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        Ok(())
    }

    fn validate_file(&self, name: &str, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() {
            bail!("file is empty")
        }
        if bytes.len() > self.config.plugins.knowledge_base.max_file_size_kb * 1024 {
            bail!("file too large: {} bytes", bytes.len())
        }
        std::str::from_utf8(bytes).context("file is not valid UTF-8 text")?;
        let file_name = file_name(name).to_ascii_lowercase();
        let ext = Path::new(&file_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| format!(".{ext}"));
        let allowed_ext = split_csv(&self.config.plugins.knowledge_base.allowed_extensions);
        let allowed_names = split_csv(&self.config.plugins.knowledge_base.allowed_filenames);
        if ext.as_ref().is_some_and(|ext| allowed_ext.contains(ext))
            || allowed_names.contains(&file_name)
        {
            Ok(())
        } else {
            bail!("unsupported file type or name: {file_name}")
        }
    }

    fn embedding_provider(&self) -> Result<Option<(ProviderConfig, String)>> {
        let kb = &self.config.plugins.knowledge_base;
        if kb.embedding_provider_id.trim().is_empty() || kb.embedding_model.trim().is_empty() {
            return Ok(None);
        }
        let mut provider = self
            .config
            .provider(Some(kb.embedding_provider_id.trim()))?
            .clone();
        provider.default_model = kb.embedding_model.trim().to_string();
        Ok(Some((provider, kb.embedding_model.trim().to_string())))
    }

    fn meta_conn(&self) -> Result<Connection> {
        if let Some(parent) = self.meta_db.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Connection::open(&self.meta_db)?)
    }

    fn semantic_conn(&self) -> Result<Connection> {
        if let Some(parent) = self.semantic_db.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Connection::open(&self.semantic_db)?)
    }

    fn safe_file_path(&self, rel: &str) -> Result<PathBuf> {
        let rel = normalize_relative_path(rel)?;
        let path = self.files_dir.join(&rel);
        let parent = path.parent().unwrap_or(&self.files_dir);
        std::fs::create_dir_all(&self.files_dir)?;
        let base = self.files_dir.canonicalize()?;
        std::fs::create_dir_all(parent)?;
        let resolved_parent = parent.canonicalize()?;
        if !resolved_parent.starts_with(&base) {
            bail!("knowledge base path escapes files dir")
        }
        Ok(path)
    }

    fn existing_file_path(&self, rel: &str) -> Result<PathBuf> {
        let rel = normalize_relative_path(rel)?;
        let path = self.files_dir.join(&rel);
        let base = self
            .files_dir
            .canonicalize()
            .unwrap_or_else(|_| self.files_dir.clone());
        let parent = path.parent().unwrap_or(&self.files_dir);
        let resolved_parent = parent.canonicalize()?;
        if !resolved_parent.starts_with(&base) {
            bail!("knowledge base path escapes files dir")
        }
        Ok(path)
    }
}

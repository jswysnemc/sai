fn init_data_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS facts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'active',
            confidence REAL NOT NULL DEFAULT 1.0,
            strength REAL NOT NULL DEFAULT 1.0,
            recall_count INTEGER NOT NULL DEFAULT 0,
            last_recalled_at TEXT,
            last_decay_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS episodes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'episode',
            status TEXT NOT NULL DEFAULT 'active',
            strength REAL NOT NULL DEFAULT 1.0,
            recall_count INTEGER NOT NULL DEFAULT 0,
            last_recalled_at TEXT,
            last_decay_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS pending_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_message TEXT NOT NULL,
            assistant_message TEXT NOT NULL,
            created_at TEXT NOT NULL,
            processed_at TEXT
        );
        CREATE TABLE IF NOT EXISTS skill_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            path TEXT NOT NULL,
            summary TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );",
    )?;
    add_column_if_missing(conn, "facts", "strength", "REAL NOT NULL DEFAULT 1.0")?;
    add_column_if_missing(conn, "facts", "last_decay_at", "TEXT")?;
    add_column_if_missing(conn, "facts", "tags", "TEXT NOT NULL DEFAULT ''")?;
    add_column_if_missing(conn, "episodes", "strength", "REAL NOT NULL DEFAULT 1.0")?;
    add_column_if_missing(conn, "episodes", "last_decay_at", "TEXT")?;
    ensure_fts(conn)?;
    Ok(())
}

fn ensure_fts(conn: &Connection) -> Result<()> {
    // Standalone FTS5 indexes (not external-content) over memory bodies.
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts USING fts5(
            content,
            tokenize = 'unicode61 remove_diacritics 2'
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS episodes_fts USING fts5(
            content,
            tokenize = 'unicode61 remove_diacritics 2'
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS facts_fts_tri USING fts5(
            content,
            tokenize = 'trigram'
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS episodes_fts_tri USING fts5(
            content,
            tokenize = 'trigram'
        );",
    )?;
    Ok(())
}

fn rebuild_fts_table(conn: &Connection, table: &str) -> Result<()> {
    for suffix in ["_fts", "_fts_tri"] {
        let fts = format!("{table}{suffix}");
        conn.execute(&format!("DELETE FROM {fts}"), [])?;
        conn.execute(
            &format!("INSERT INTO {fts}(rowid, content) SELECT id, content FROM {table}"),
            [],
        )?;
    }
    Ok(())
}

fn fts_upsert_row(conn: &Connection, table: &str, id: i64, content: &str) -> Result<()> {
    for suffix in ["_fts", "_fts_tri"] {
        let fts = format!("{table}{suffix}");
        conn.execute(&format!("DELETE FROM {fts} WHERE rowid = ?1"), params![id])?;
        conn.execute(
            &format!("INSERT INTO {fts}(rowid, content) VALUES (?1, ?2)"),
            params![id, content],
        )?;
    }
    Ok(())
}

fn fts_delete_row(conn: &Connection, table: &str, id: i64) -> Result<()> {
    for suffix in ["_fts", "_fts_tri"] {
        let fts = format!("{table}{suffix}");
        conn.execute(&format!("DELETE FROM {fts} WHERE rowid = ?1"), params![id])?;
    }
    Ok(())
}

fn write_memory_markdown(
    files_dir: &PathBuf,
    kind: &str,
    id: i64,
    content: &str,
    source: &str,
    status: &str,
    confidence: Option<f64>,
    strength: f64,
    created_at: &str,
    updated_at: &str,
    tags: &str,
) -> Result<PathBuf> {
    let dir = files_dir.join(kind);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{id}.md"));
    let conf = confidence
        .map(|value| format!("{value}"))
        .unwrap_or_else(|| "1.0".to_string());
    let tags = tags.trim();
    let body = format!(
        "---\nid: {id}\nkind: {kind}\nsource: {source}\nstatus: {status}\nconfidence: {conf}\nstrength: {strength}\ntags: {tags}\ncreated_at: {created_at}\nupdated_at: {updated_at}\n---\n\n{content}\n"
    );
    std::fs::write(&path, body)?;
    Ok(path)
}

/// 规范化标签列表为逗号分隔小写串。
fn normalize_tags(tags: &[String]) -> String {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for tag in tags {
        let value = tag
            .trim()
            .trim_start_matches('#')
            .to_ascii_lowercase();
        if value.is_empty() || !seen.insert(value.clone()) {
            continue;
        }
        out.push(value);
    }
    out.join(",")
}

/// 将标签文本拆成列表。
fn split_tags(raw: &str) -> Vec<String> {
    raw.split(|ch: char| ch == ',' || ch == ';' || ch.is_whitespace())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_start_matches('#').to_string())
        .collect()
}


fn delete_memory_markdown(files_dir: &PathBuf, kind: &str, id: i64) -> Result<()> {
    let path = files_dir.join(kind).join(format!("{id}.md"));
    if path.is_file() {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

fn clear_memory_markdown(files_dir: &PathBuf) -> Result<()> {
    for kind in ["facts", "episodes"] {
        let dir = files_dir.join(kind);
        if !dir.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().extension().and_then(|e| e.to_str()) == Some("md") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    Ok(())
}

fn fts_query_terms(query: &str) -> String {
    // Build a safe FTS5 phrase/OR query from whitespace tokens.
    let tokens = query_tokens(query);
    if tokens.is_empty() {
        return String::new();
    }
    tokens
        .into_iter()
        .map(|token| {
            let escaped = token.replace('"', "\"\"");
            format!("\"{escaped}\"")
        })
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(|ch| {
        ('\u{4e00}'..='\u{9fff}').contains(&ch)
            || ('\u{3400}'..='\u{4dbf}').contains(&ch)
            || ('\u{3040}'..='\u{30ff}').contains(&ch)
            || ('\u{ac00}'..='\u{d7af}').contains(&ch)
    })
}

fn init_state_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS evicted_turns (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        );",
    )?;
    Ok(())
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == column {
            return Ok(());
        }
    }
    conn.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        [],
    )?;
    Ok(())
}

fn decay_table(conn: &Connection, table: &str, config: &MemoryConfig) -> Result<()> {
    let now = Utc::now();
    let mut stmt = conn.prepare(&format!(
        "SELECT id, strength, COALESCE(last_recalled_at, updated_at, created_at), last_decay_at FROM {table} WHERE status='active'"
    ))?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;
    let mut updates = Vec::new();
    for row in rows {
        let (id, strength, recalled_at, last_decay_at) = row?;
        let anchor = last_decay_at.as_deref().unwrap_or(&recalled_at);
        let Ok(anchor) = DateTime::parse_from_rfc3339(anchor) else {
            continue;
        };
        let days = (now - anchor.with_timezone(&Utc)).num_seconds().max(0) as f64 / 86_400.0;
        if days < 0.25 {
            continue;
        }
        let half_life = config.forgetting_half_life_days.max(0.1);
        let new_strength = strength * 2f64.powf(-days / half_life);
        let status = if new_strength < config.forgetting_min_strength {
            "forgotten"
        } else {
            "active"
        };
        updates.push((id, new_strength, status.to_string()));
    }
    drop(stmt);
    for (id, strength, status) in updates {
        conn.execute(
            &format!("UPDATE {table} SET strength=?1, status=?2, last_decay_at=?3 WHERE id=?4"),
            params![strength, status, now.to_rfc3339(), id],
        )?;
    }
    Ok(())
}

fn memory_hit_json(hit: &MemoryHit) -> Value {
    json!({
        "id": hit.id,
        "timestamp": hit.timestamp,
        "score": hit.score,
        "source": hit.source,
        "content": hit.content,
        "tags": hit.tags,
    })
}

fn sort_json_hits(hits: &mut [Value]) {
    hits.sort_by(|a, b| {
        b.get("score")
            .and_then(Value::as_f64)
            .unwrap_or_default()
            .partial_cmp(&a.get("score").and_then(Value::as_f64).unwrap_or_default())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn score_text(text: &str, tokens: &[String]) -> f32 {
    if tokens.is_empty() {
        return 0.0;
    }
    let lower = text.to_ascii_lowercase();
    let mut score = 0.0;
    let mut matched = HashSet::new();
    for token in tokens {
        if lower.contains(token) {
            score += 10.0;
            matched.insert(token);
        }
    }
    score + matched.len() as f32 / tokens.len() as f32 * 20.0
}

fn query_tokens(query: &str) -> Vec<String> {
    query
        .split(|ch: char| ch.is_whitespace() || ch.is_ascii_punctuation())
        .map(str::trim)
        .filter(|token| token.chars().count() >= 2)
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn snippet(text: &str, tokens: &[String], max_chars: usize) -> String {
    let lower = text.to_ascii_lowercase();
    let start = tokens
        .iter()
        .filter_map(|token| lower.find(token))
        .min()
        .unwrap_or(0);
    let start = text[..start.min(text.len())]
        .char_indices()
        .rev()
        .nth(max_chars / 4)
        .map(|(index, _)| index)
        .unwrap_or(0);
    truncate_chars(&text[start..], max_chars)
}

fn compact_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    format!(
        "{}...",
        text.chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>()
    )
}

fn count_rows(conn: &Connection, table: &str) -> Result<i64> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    Ok(conn.query_row(&sql, [], |row| row.get(0))?)
}

fn count_where(conn: &Connection, table: &str, condition: &str) -> Result<i64> {
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE {condition}");
    Ok(conn.query_row(&sql, [], |row| row.get(0))?)
}

fn count_skill_dirs(skills_dir: &PathBuf) -> Result<usize> {
    if !skills_dir.exists() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in std::fs::read_dir(skills_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && entry.path().join("SKILL.md").is_file() {
            count += 1;
        }
    }
    Ok(count)
}


fn count_markdown_files(files_dir: &PathBuf, kind: &str) -> Result<i64> {
    let dir = files_dir.join(kind);
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut count = 0i64;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|e| e.to_str()) == Some("md") {
            count += 1;
        }
    }
    Ok(count)
}

fn count_fts_rows(conn: &Connection, table: &str) -> Result<i64> {
    match conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0)) {
        Ok(count) => Ok(count),
        Err(_) => Ok(0),
    }
}

fn fts_ready(conn: &Connection, facts: i64, episodes: i64) -> Result<bool> {
    let facts_fts = count_fts_rows(conn, "facts_fts")?;
    let episodes_fts = count_fts_rows(conn, "episodes_fts")?;
    // 索引表存在且行数与主表大致一致，即视为就绪
    Ok(facts_fts >= facts && episodes_fts >= episodes)
}

fn attach_markdown_meta(entry: &mut Value, files_dir: &PathBuf) {
    let Some(kind) = entry.get("kind").and_then(Value::as_str) else {
        return;
    };
    let folder = match kind {
        "fact" | "facts" => "facts",
        "episode" | "episodes" => "episodes",
        _ => return,
    };
    let Some(id) = entry.get("id").and_then(Value::as_i64) else {
        return;
    };
    let path = files_dir.join(folder).join(format!("{id}.md"));
    let exists = path.is_file();
    if let Some(obj) = entry.as_object_mut() {
        obj.insert("has_markdown".to_string(), json!(exists));
        if exists {
            obj.insert(
                "markdown_path".to_string(),
                json!(path.display().to_string()),
            );
        }
    }
}

fn now() -> String {
    Utc::now().to_rfc3339()
}


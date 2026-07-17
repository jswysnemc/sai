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
    add_column_if_missing(conn, "episodes", "strength", "REAL NOT NULL DEFAULT 1.0")?;
    add_column_if_missing(conn, "episodes", "last_decay_at", "TEXT")?;
    Ok(())
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

fn now() -> String {
    Utc::now().to_rfc3339()
}


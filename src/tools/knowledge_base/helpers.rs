pub async fn embed_text(
    config: &AppConfig,
    provider: &ProviderConfig,
    model: &str,
    text: &str,
) -> Result<Vec<f32>> {
    let api_key = provider.api_key.as_deref().unwrap_or_default().trim();
    if api_key.is_empty() {
        bail!("embedding provider {} has no api_key", provider.id)
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(
            config.plugins.knowledge_base.embedding_timeout_seconds,
        ))
        .build()?;
    let url = format!("{}/embeddings", provider.base_url.trim_end_matches('/'));
    let response = client
        .post(&url)
        .bearer_auth(api_key)
        .json(&json!({ "model": model, "input": text }))
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        bail!(
            "embedding API error at {url} ({status}): {}",
            compact_whitespace(&text)
        );
    }
    let data: Value = response.json().await?;
    let embedding = data
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("embedding"))
        .and_then(Value::as_array)
        .context("embedding response missing data[0].embedding")?;
    Ok(embedding
        .iter()
        .filter_map(Value::as_f64)
        .map(|value| value as f32)
        .collect())
}

fn ensure_enabled(config: &AppConfig) -> Result<()> {
    if !config.plugins.knowledge_base.enabled {
        bail!("knowledge base plugin is disabled")
    }
    Ok(())
}

fn init_meta_db(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS files (name TEXT PRIMARY KEY, path TEXT NOT NULL, size_bytes INTEGER NOT NULL, mtime REAL NOT NULL, content_sha256 TEXT NOT NULL, updated_at REAL NOT NULL)",
        [],
    )?;
    Ok(())
}

fn init_semantic_db(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS semantic_chunks (id INTEGER PRIMARY KEY AUTOINCREMENT, provider_id TEXT NOT NULL, model TEXT NOT NULL, file_name TEXT NOT NULL, content_sha256 TEXT NOT NULL, chunk_index INTEGER NOT NULL, start_char INTEGER NOT NULL, end_char INTEGER NOT NULL, text TEXT NOT NULL, embedding_json TEXT NOT NULL, created_at REAL NOT NULL)",
        [],
    )?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_semantic_file ON semantic_chunks(file_name, content_sha256)", [])?;
    Ok(())
}

fn kb_root(config: &KnowledgeBasePluginConfig, paths: &SaiPaths) -> PathBuf {
    let configured = config.data_dir.trim();
    if configured.is_empty() {
        paths.data_dir.join("kb")
    } else {
        expand_path(configured)
    }
}

fn normalize_relative_path(value: &str) -> Result<String> {
    let path = Path::new(value.trim());
    if path.is_absolute() {
        bail!("knowledge base path must be relative")
    }
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_string_lossy();
                if part.contains('\0') || part.trim().is_empty() {
                    bail!("invalid path component")
                }
                parts.push(part.to_string());
            }
            Component::CurDir => {}
            _ => bail!("knowledge base path contains illegal component"),
        }
    }
    if parts.is_empty() {
        bail!("knowledge base path is empty")
    }
    Ok(parts.join("/"))
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            out.extend(collect_files(&path)?);
        } else if path.is_file() {
            out.push(path);
        }
    }
    Ok(out)
}

fn split_csv(value: &str) -> HashSet<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn query_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut ascii = String::new();
    let mut chinese = Vec::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            ascii.push(ch.to_ascii_lowercase());
            flush_chinese(&mut chinese, &mut tokens);
        } else if ('\u{4e00}'..='\u{9fff}').contains(&ch) {
            if !ascii.is_empty() {
                tokens.push(std::mem::take(&mut ascii));
            }
            chinese.push(ch);
        } else {
            if !ascii.is_empty() {
                tokens.push(std::mem::take(&mut ascii));
            }
            flush_chinese(&mut chinese, &mut tokens);
        }
    }
    if !ascii.is_empty() {
        tokens.push(ascii);
    }
    flush_chinese(&mut chinese, &mut tokens);
    let mut seen = HashSet::new();
    tokens
        .into_iter()
        .filter(|token| token.chars().count() > 1 || !token.is_ascii())
        .filter(|token| seen.insert(token.clone()))
        .collect()
}

fn flush_chinese(chars: &mut Vec<char>, tokens: &mut Vec<String>) {
    if chars.is_empty() {
        return;
    }
    let text = chars.iter().collect::<String>();
    tokens.push(text);
    for window in chars.windows(2) {
        tokens.push(window.iter().collect());
    }
    chars.clear();
}

fn find_positions(content: &str, needle: &str, limit: usize) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut start = 0;
    while let Some(pos) = content[start..].find(needle) {
        let absolute = start + pos;
        positions.push(absolute);
        if positions.len() >= limit {
            break;
        }
        start = absolute + needle.len().max(1);
    }
    positions
}

fn best_window(
    positions_by_token: &HashMap<String, Vec<usize>>,
    tokens: &[String],
    window_chars: usize,
) -> Option<(usize, usize, f32)> {
    let mut events = Vec::new();
    for token in tokens {
        for pos in positions_by_token.get(token).into_iter().flatten() {
            events.push((*pos, token.as_str()));
        }
    }
    events.sort_by_key(|event| event.0);
    let mut best = None;
    for left in 0..events.len() {
        let mut seen = HashSet::new();
        let start = events[left].0;
        let mut end = start;
        for (pos, token) in events.iter().skip(left) {
            if *pos - start > window_chars {
                break;
            }
            seen.insert(*token);
            end = *pos + token.len();
        }
        let coverage = seen.len() as f32 / tokens.len().max(1) as f32;
        if best.map(|(_, _, score)| coverage > score).unwrap_or(true) {
            best = Some((start, end, coverage));
        }
    }
    best.filter(|(_, _, coverage)| *coverage > 0.0)
}

fn extract_snippets(
    content: &str,
    content_lower: &str,
    tokens: &[String],
    context: usize,
) -> Vec<String> {
    let mut snippets = Vec::new();
    for token in tokens {
        if let Some(pos) = content_lower.find(token) {
            snippets.push(snippet_chars(content, pos, pos + token.len(), context));
        }
        if snippets.len() >= 3 {
            break;
        }
    }
    if snippets.is_empty() && !content.trim().is_empty() {
        snippets.push(compact_whitespace(
            &content.chars().take(context * 2).collect::<String>(),
        ));
    }
    snippets
}

fn snippet_chars(content: &str, start: usize, end: usize, context: usize) -> String {
    let start = content[..start.min(content.len())]
        .char_indices()
        .rev()
        .nth(context)
        .map(|(idx, _)| idx)
        .unwrap_or(0);
    let end = content[end.min(content.len())..]
        .char_indices()
        .nth(context)
        .map(|(idx, _)| end.min(content.len()) + idx)
        .unwrap_or(content.len());
    compact_whitespace(&content[start..end])
}

fn build_chunks(content: &str, chunk_chars: usize, overlap: usize) -> Vec<Chunk> {
    let chars = content.char_indices().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    let mut start_char = 0usize;
    let mut index = 0usize;
    let total_chars = content.chars().count();
    while start_char < total_chars {
        let end_char = (start_char + chunk_chars).min(total_chars);
        let start_byte = chars.get(start_char).map(|(idx, _)| *idx).unwrap_or(0);
        let end_byte = chars
            .get(end_char)
            .map(|(idx, _)| *idx)
            .unwrap_or(content.len());
        let text = content[start_byte..end_byte].to_string();
        if !text.trim().is_empty() {
            chunks.push(Chunk {
                index,
                start: start_byte,
                end: end_byte,
                text,
            });
            index += 1;
        }
        if end_char >= total_chars {
            break;
        }
        start_char = end_char.saturating_sub(overlap).max(start_char + 1);
    }
    chunks
}

fn merge_results(results: &mut Vec<SearchResult>, semantic: Vec<SearchResult>, limit: usize) {
    for item in semantic {
        if let Some(existing) = results.iter_mut().find(|result| result.path == item.path) {
            existing.score += item.score * 0.6;
            existing.snippets.extend(item.snippets);
            existing.snippets.truncate(4);
        } else {
            results.push(item);
        }
    }
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);
}

fn score_file_name(query: &str, name: &str) -> (f64, &'static str) {
    let query = query.replace('\\', "/").to_ascii_lowercase();
    let name = name.replace('\\', "/").to_ascii_lowercase();
    let base = file_name(&name);
    if query == name {
        (1000.0, "exact_path")
    } else if query == base {
        (950.0, "exact_file_name")
    } else if name.contains(&query) {
        (820.0 + query.len().min(60) as f64, "path_contains")
    } else if base.contains(&query) {
        (760.0 + query.len().min(60) as f64, "file_name_contains")
    } else {
        let tokens = query_tokens(&query);
        let matched = tokens.iter().filter(|token| name.contains(*token)).count();
        if matched == 0 {
            (0.0, "")
        } else {
            (300.0 + matched as f64 * 80.0, "partial_name_terms")
        }
    }
}

fn cosine(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;
    for (a, b) in left.iter().zip(right) {
        dot += a * b;
        left_norm += a * a;
        right_norm += b * b;
    }
    if left_norm <= 0.0 || right_norm <= 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn directory_name(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(dir, _)| dir.to_string())
        .unwrap_or_default()
}

fn compact_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn now_secs() -> f64 {
    unix_time(SystemTime::now())
}

fn unix_time(time: SystemTime) -> f64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn expand_path(value: &str) -> PathBuf {
    if let Some(rest) = value.trim().strip_prefix("~/") {
        if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
            return home.join(rest);
        }
    }
    PathBuf::from(value.trim())
}

fn slug(value: &str) -> String {
    let mut slug = value
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() {
                Some(ch.to_ascii_lowercase())
            } else if ch.is_whitespace() || matches!(ch, '-' | '_') {
                Some('-')
            } else {
                None
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        format!("note-{}", Local::now().format("%H%M%S"))
    } else {
        slug.chars().take(48).collect()
    }
}

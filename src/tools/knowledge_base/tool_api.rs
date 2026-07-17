#[derive(Clone)]
pub struct FileRecord {
    pub name: String,
    path: String,
    pub size_bytes: i64,
    content_sha256: String,
}

#[derive(Debug)]
pub struct EditResult {
    path: String,
    old_line_count: usize,
    new_line_count: usize,
    semantic_refreshed: bool,
}

struct SearchResult {
    path: String,
    score: f32,
    snippets: Vec<String>,
    source: &'static str,
}

impl SearchResult {
    fn new(path: String, score: f32, snippets: Vec<String>, source: &'static str) -> Self {
        Self {
            path,
            score,
            snippets,
            source,
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "path": self.path,
            "name": file_name(&self.path),
            "directory": directory_name(&self.path),
            "score": (self.score * 10.0).round() / 10.0,
            "source": self.source,
            "snippets": self.snippets,
        })
    }
}

struct Chunk {
    index: usize,
    start: usize,
    end: usize,
    text: String,
}

async fn tool_search_readonly(args: Value, config: AppConfig, paths: SaiPaths) -> Result<String> {
    ensure_enabled(&config)?;
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if query.is_empty() {
        bail!("query is required")
    }
    let max_results = args
        .get("max_results")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    Ok(KnowledgeBase::new(config, paths)?
        .search_readonly(query, max_results)
        .await?
        .to_string())
}

async fn tool_find_readonly(args: Value, config: AppConfig, paths: SaiPaths) -> Result<String> {
    ensure_enabled(&config)?;
    let query = args
        .get("file_name_query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if query.is_empty() {
        bail!("file_name_query is required")
    }
    let max_results = args
        .get("max_results")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    Ok(KnowledgeBase::new(config, paths)?
        .find_by_name_readonly(query, max_results)?
        .to_string())
}

async fn tool_read_readonly(args: Value, config: AppConfig, paths: SaiPaths) -> Result<String> {
    ensure_enabled(&config)?;
    let name = args
        .get("file_name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if name.is_empty() {
        bail!("file_name is required")
    }
    let start_line = args.get("start_line").and_then(Value::as_u64).unwrap_or(1) as usize;
    let max_lines = args
        .get("max_lines")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    KnowledgeBase::new(config, paths)?.read_file_readonly(name, start_line, max_lines)
}

async fn tool_upload(args: Value, config: AppConfig, paths: SaiPaths) -> Result<String> {
    ensure_enabled(&config)?;
    if !config.plugins.knowledge_base.upload_tool_enabled {
        bail!("knowledge base upload tool is disabled")
    }
    let content = args
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if content.is_empty() {
        bail!("content is required")
    }
    let title = args
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("knowledge note")
        .trim();
    let file_name = args
        .get("file_name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    reject_non_kb_upload(content, title, file_name)?;
    let rel = if file_name.is_empty() {
        format!(
            "chat_uploads/{}/{}.md",
            Local::now().format("%Y-%m-%d"),
            slug(title)
        )
    } else {
        normalize_relative_path(file_name)?
    };
    let body = format!(
        "# {}\n\n> 来源：用户要求保存到本地知识库\n> 上传时间：{}\n\n{}\n",
        if title.is_empty() {
            Path::new(&rel)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("knowledge note")
        } else {
            title
        },
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        content
    );
    let kb = KnowledgeBase::new(config, paths)?;
    kb.init()?;
    let temp = tempfile::NamedTempFile::new()?;
    std::fs::write(temp.path(), body.as_bytes())?;
    let saved = kb.import_file(temp.path(), &rel)?;
    kb.spawn_embedding_reindex()?;
    Ok(json!({
        "ok": true,
        "path": saved,
    })
    .to_string())
}

async fn tool_edit(args: Value, config: AppConfig, paths: SaiPaths) -> Result<String> {
    ensure_enabled(&config)?;
    let name = args
        .get("file_name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if name.is_empty() {
        bail!("file_name is required")
    }
    let start_line = args
        .get("start_line")
        .and_then(Value::as_u64)
        .context("start_line is required")? as usize;
    let end_line = args
        .get("end_line")
        .and_then(Value::as_u64)
        .context("end_line is required")? as usize;
    let replacement = args
        .get("replacement")
        .and_then(Value::as_str)
        .context("replacement is required")?;
    let result =
        KnowledgeBase::new(config, paths)?.edit_lines(name, start_line, end_line, replacement)?;
    Ok(json!({
        "ok": true,
        "path": result.path,
        "old_line_count": result.old_line_count,
        "new_line_count": result.new_line_count,
        "semantic_refreshed": result.semantic_refreshed,
        "warning": None::<&str>,
    })
    .to_string())
}

async fn tool_remove(args: Value, config: AppConfig, paths: SaiPaths) -> Result<String> {
    ensure_enabled(&config)?;
    let name = args
        .get("file_name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if name.is_empty() {
        bail!("file_name is required")
    }
    let rel = normalize_relative_path(name)?;
    KnowledgeBase::new(config, paths)?.remove(&rel)?;
    Ok(json!({
        "ok": true,
        "path": rel,
        "warning": None::<&str>,
    })
    .to_string())
}

fn reject_non_kb_upload(content: &str, title: &str, file_name: &str) -> Result<()> {
    let text = format!("{content}\n{title}\n{file_name}").to_ascii_lowercase();
    let forbidden = [
        "skill", "skills/", "skll", "记忆", "memory", "persona", "identity", "prompt", "配置",
        "config",
    ];
    if forbidden.iter().any(|needle| text.contains(needle)) {
        bail!("this content looks like a skill, memory, prompt, identity, or config request; do not upload it to the knowledge base")
    }
    Ok(())
}

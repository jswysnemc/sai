async fn describe_meme_image(config: &AppConfig, paths: &SaiPaths, image: &Path) -> Result<Value> {
    let text =
        vision::analyze_local_image_with_prompt(config, paths, image, MEME_DESCRIPTION_PROMPT)
            .await?;
    let start = text.find('{').unwrap_or(0);
    let end = text.rfind('}').map(|index| index + 1).unwrap_or(text.len());
    Ok(serde_json::from_str(&text[start..end])?)
}

async fn decide_auto_send(
    client: &OpenAiCompatibleClient,
    user_message: &str,
    candidates: &[(f32, LoadedMeme)],
) -> Result<Option<AutoSendDecision>> {
    let catalog = candidates
        .iter()
        .map(|(score, meme)| {
            json!({
                "id": meme.item.id,
                "local_score": (score * 100.0).round() / 100.0,
                "name": meme.item.name,
                "description": meme.item.description,
                "usage": meme.item.usage,
                "avoid": meme.item.avoid,
                "tags": meme.item.tags,
            })
        })
        .collect::<Vec<_>>();
    let prompt = format!(
        "你在 Sai 回复前决定本轮是否应该搭配一张表情包。概率只控制触发频率；这里需要判断候选表情和用户消息、上下文语气的相关程度。请根据用户消息的语气、场景、关系边界和候选表情的 usage/avoid 决定。严肃、道歉、群管理、技术排障、长篇解释、用户明显在求助时不要发表情。轻松闲聊、调侃、打招呼、夸奖、吐槽、玩梗、情绪回应时可以发。只能从候选表情里选。confidence 表示所选表情与本轮消息/上下文的相关程度，0.0 到 1.0。只返回严格 JSON：{{\"send\": false, \"id\": \"\", \"confidence\": 0.0, \"reason\": \"\"}}\n\n用户消息：{}\n\n候选表情：{}",
        user_message.chars().take(1000).collect::<String>(),
        serde_json::to_string(&catalog)?,
    );
    let result = client
        .chat_stream(
            vec![
                ChatMessage::system("你是表情包发送决策器，只输出 JSON，不输出解释。"),
                ChatMessage::plain("user", prompt),
            ],
            Vec::new(),
            |_| Ok(()),
        )
        .await?;
    let Some(json_text) = json_slice(&result.content) else {
        return Ok(None);
    };
    Ok(serde_json::from_str::<AutoSendDecision>(json_text).ok())
}

fn rank_memes(
    paths: &SaiPaths,
    library: &str,
    query: &str,
    tags: &[String],
    limit: usize,
) -> Result<Vec<(f32, LoadedMeme)>> {
    let mut scored = load_library(paths, library)?
        .into_iter()
        .filter_map(|meme| {
            let score = score_meme(&meme.item, query, tags);
            (score > 0.0).then_some((score, meme))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit.max(1));
    Ok(scored)
}

fn load_library(paths: &SaiPaths, library: &str) -> Result<Vec<LoadedMeme>> {
    let builtin_dir = builtin_library_dir(library);
    let user_dir = user_library_dir(paths, library);
    let builtin = load_index(&builtin_dir.join("index.json"))?.unwrap_or_default();
    let user = load_index(&user_dir.join("index.json"))?.unwrap_or_default();
    let disabled = user.disabled_ids;
    let mut user_ids = Vec::new();
    let mut result = Vec::new();
    for item in user.memes {
        if disabled.iter().any(|id| ids_match(id, &item.id)) {
            continue;
        }
        user_ids.push(item.id.clone());
        result.push(LoadedMeme {
            path: user_dir.join(&item.file),
            item,
            source: MemeSource::User,
        });
    }
    for item in builtin.memes {
        if disabled.iter().any(|id| ids_match(id, &item.id))
            || user_ids.iter().any(|id| ids_match(id, &item.id))
        {
            continue;
        }
        result.push(LoadedMeme {
            path: builtin_dir.join(&item.file),
            item,
            source: MemeSource::Builtin,
        });
    }
    Ok(result)
}

fn find_meme(paths: &SaiPaths, library: &str, id: &str) -> Result<Option<LoadedMeme>> {
    Ok(load_library(paths, library)?
        .into_iter()
        .find(|meme| ids_match(&meme.item.id, id)))
}

fn ids_match(stored: &str, requested: &str) -> bool {
    let stored = id_hash_part(stored);
    let requested = id_hash_part(requested);
    !requested.is_empty() && stored.starts_with(requested)
}

fn id_hash_part(value: &str) -> &str {
    let value = value.trim();
    value.strip_prefix("sha256:").unwrap_or(value)
}

fn load_index(path: &Path) -> Result<Option<MemeIndex>> {
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(&std::fs::read_to_string(path)?)?))
}

fn save_index(path: &Path, index: &MemeIndex) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(index)?)?;
    Ok(())
}

fn selected_library(args: &Value, config: &AppConfig) -> String {
    args.get("library")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(sanitize_library)
        .unwrap_or_else(|| {
            config
                .plugins
                .memes
                .library_for_persona(&config.prompt.active_persona)
        })
}

fn load_auto_meme_state(config: &AppConfig, paths: &SaiPaths) -> Result<AutoMemeState> {
    let path = auto_meme_state_path(config, paths);
    if !path.is_file() {
        return Ok(AutoMemeState::default());
    }
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn auto_meme_state_path(config: &AppConfig, paths: &SaiPaths) -> PathBuf {
    let library = config
        .plugins
        .memes
        .library_for_persona(&config.prompt.active_persona);
    paths
        .state_dir
        .join("memes")
        .join(sanitize_library(&library))
        .join("auto-send.json")
}

fn display_name(name: &Value) -> String {
    let zh = name.get("zh").and_then(Value::as_str).unwrap_or_default();
    let en = name.get("en").and_then(Value::as_str).unwrap_or_default();
    if !zh.trim().is_empty() {
        zh.to_string()
    } else if !en.trim().is_empty() {
        en.to_string()
    } else {
        "未命名表情".to_string()
    }
}

fn json_slice(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end >= start).then_some(&text[start..=end])
}

fn sanitize_library(value: &str) -> String {
    let value = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if value.is_empty() {
        "default".to_string()
    } else {
        value
    }
}

fn builtin_library_dir(library: &str) -> PathBuf {
    if let Some(path) = std::env::var_os("SAI_MEMES_DIR") {
        return PathBuf::from(path).join(library);
    }
    let dev = PathBuf::from("src/memes").join(library);
    if dev.is_dir() {
        return dev;
    }
    PathBuf::from(BUILTIN_MEMES_DIR).join(library)
}

fn user_library_dir(paths: &SaiPaths, library: &str) -> PathBuf {
    paths.data_dir.join("memes").join(sanitize_library(library))
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    let value = args
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if value.is_empty() {
        bail!("{key} is required")
    }
    Ok(value)
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn score_meme(item: &MemeItem, query: &str, tags: &[String]) -> f32 {
    let query = normalize(&format!("{query} {}", tags.join(" ")));
    if query.is_empty() {
        return 0.1;
    }
    let haystack = normalize(&format!(
        "{} {} {} {} {} {}",
        item.name.zh,
        item.name.en,
        item.description,
        item.usage,
        item.avoid,
        item.tags.join(" ")
    ));
    let mut score = 0.0;
    for term in query.split_whitespace() {
        if haystack.contains(term) {
            score += if item.tags.iter().any(|tag| normalize(tag).contains(term)) {
                2.0
            } else {
                1.0
            };
        }
    }
    if haystack.contains(&query) {
        score += 2.0;
    }
    score
}

fn normalize(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_punctuation() { ' ' } else { ch })
        .collect::<String>()
}

fn meme_print_size(args: &Value, config: &MemesPluginConfig) -> Option<String> {
    let width = args
        .get("width")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(160);
    let height = args
        .get("height")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(80);
    match (width, height) {
        (0, 0) => args
            .get("size")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| configured_meme_size(config)),
        (width, 0) => Some(format!("{width}x")),
        (0, height) => Some(format!("x{height}")),
        (width, height) => Some(format!("{width}x{height}")),
    }
}

fn configured_meme_size(config: &MemesPluginConfig) -> Option<String> {
    let (cols, rows) = crossterm::terminal::size().ok()?;
    let width = ((cols as u32 * config.width_percent as u32) / 100)
        .max(1)
        .min(160);
    let height = ((rows as u32 * config.height_percent as u32) / 100)
        .max(1)
        .min(80);
    Some(format!("{width}x{height}"))
}

fn expand_path(value: &str) -> PathBuf {
    if let Some(rest) = value.trim().strip_prefix("~/") {
        if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
            return home.join(rest);
        }
    }
    let path = Path::new(value.trim());
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        crate::runtime_cwd::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn image_ext(path: &Path) -> Result<&'static str> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => Ok("jpg"),
        "png" => Ok("png"),
        "webp" => Ok("webp"),
        "gif" => Ok("gif"),
        value => {
            bail!("unsupported image extension: {value}; supported: jpg, jpeg, png, webp, gif")
        }
    }
}

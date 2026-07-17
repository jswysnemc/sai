async fn search_meme(args: Value, config: &AppConfig, paths: &SaiPaths) -> Result<String> {
    let library = selected_library(&args, config);
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let tags = string_array(args.get("tags"));
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(6)
        .clamp(1, 20) as usize;
    let mut scored = load_library(paths, &library)?
        .into_iter()
        .filter_map(|meme| {
            let score = score_meme(&meme.item, query, &tags);
            (score > 0.0).then_some((score, meme))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let results = scored
        .into_iter()
        .take(limit)
        .map(|(score, meme)| {
            json!({
                "id": meme.item.id,
                "name": meme.item.name,
                "score": (score * 100.0).round() / 100.0,
                "description": meme.item.description,
                "usage": meme.item.usage,
                "avoid": meme.item.avoid,
                "tags": meme.item.tags,
                "animated": meme.item.animated,
                "source": source_label(meme.source),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "success": true, "library": library, "results": results }).to_string())
}

async fn show_meme(args: Value, config: &AppConfig, paths: &SaiPaths) -> Result<String> {
    let library = selected_library(&args, config);
    let id = required_str(&args, "id")?;
    let meme = find_meme(paths, &library, id)?.with_context(|| format!("meme not found: {id}"))?;
    let size = meme_print_size(&args, &config.plugins.memes);
    vision::print_image_file(&meme.path, size).await?;
    Ok(json!({
        "success": true,
        "library": library,
        "id": meme.item.id,
        "name": meme.item.name,
        "description": meme.item.description,
        "animated": meme.item.animated,
        "animation_note": if meme.item.animated && !config.plugins.memes.allow_gif_animation { Some("GIF was rendered as a static terminal preview; animation is disabled by default.") } else { None },
    })
    .to_string())
}

async fn recent_meme(config: &AppConfig, paths: &SaiPaths) -> Result<String> {
    let state = load_auto_meme_state(config, paths)?;
    Ok(match state.last {
        Some(event) => json!({ "success": true, "recent": event }).to_string(),
        None => json!({ "success": false, "message": "当前人格/表情库还没有自动发送过表情" })
            .to_string(),
    })
}

pub(crate) async fn plan_auto_meme_before_reply(
    config: &AppConfig,
    paths: &SaiPaths,
    client: &OpenAiCompatibleClient,
    user_message: &str,
) -> Result<Option<AutoMemePlan>> {
    let meme_config = &config.plugins.memes;
    if !meme_config.enabled
        || !meme_config.auto_send_enabled
        || user_message.trim().is_empty()
        || meme_config.auto_send_probability <= 0.0
    {
        return Ok(None);
    }
    if rand::random::<f32>() > meme_config.auto_send_probability.clamp(0.0, 1.0) {
        return Ok(None);
    }
    let library = meme_config.library_for_persona(&config.prompt.active_persona);
    let mut candidates = rank_memes(paths, &library, user_message, &[], 12)?;
    if candidates.is_empty() {
        candidates = rank_memes(paths, &library, "", &[], 12)?;
    }
    if candidates.is_empty() {
        return Ok(None);
    }
    let decision = decide_auto_send(client, user_message, &candidates).await?;
    let Some(decision) = decision else {
        return Ok(None);
    };
    if !decision.send || decision.confidence < meme_config.auto_send_min_confidence.clamp(0.0, 1.0)
    {
        return Ok(None);
    }
    let Some((_, meme)) = candidates
        .drain(..)
        .find(|(_, meme)| ids_match(&meme.item.id, &decision.id))
    else {
        return Ok(None);
    };
    let event = AutoMemeEvent {
        library,
        id: meme.item.id,
        name: serde_json::to_value(&meme.item.name)?,
        description: meme.item.description,
        usage: meme.item.usage,
        reason: decision.reason,
        sent_at: Utc::now().to_rfc3339(),
    };
    let reminder = format!(
        "<system-reminder>\n本轮回复发送后，程序会自动发送一张表情包。你在回复文字时应该自然地知道这件事，让语气和表情一致，但不要直白说“我将发送表情包”。\n计划发送表情：{}\n表情描述：{}\n适用场景：{}\n选择原因：{}\n</system-reminder>",
        display_name(&event.name),
        event.description,
        event.usage,
        event.reason,
    );
    Ok(Some(AutoMemePlan { event, reminder }))
}

pub(crate) async fn render_auto_meme(
    config: &AppConfig,
    paths: &SaiPaths,
    event: &AutoMemeEvent,
) -> Result<()> {
    let meme = find_meme(paths, &event.library, &event.id)?
        .with_context(|| format!("meme not found: {}", event.id))?;
    vision::print_image_file(&meme.path, configured_meme_size(&config.plugins.memes)).await
}

pub(crate) fn record_auto_meme_event(
    config: &AppConfig,
    paths: &SaiPaths,
    event: &AutoMemeEvent,
) -> Result<()> {
    let state = AutoMemeState {
        last: Some(event.clone()),
    };
    let path = auto_meme_state_path(config, paths);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(&state)?))?;
    Ok(())
}

pub(crate) fn last_auto_meme_reminder(
    config: &AppConfig,
    paths: &SaiPaths,
) -> Result<Option<String>> {
    let Some(event) = load_auto_meme_state(config, paths)?.last else {
        return Ok(None);
    };
    Ok(Some(format!(
        "<system-reminder>\n上一轮回复文字发送后，程序自动补发了一张表情包。你需要自然地记得自己已经发过这张表情；如果用户提到“刚才那张/你发的表情”，按这个信息回答，不要说不知道。\n表情库：{}\n表情名：{}\n表情描述：{}\n适用场景：{}\n发送原因：{}\n发送时间：{}\n</system-reminder>",
        event.library,
        display_name(&event.name),
        event.description,
        event.usage,
        event.reason,
        event.sent_at,
    )))
}

async fn add_meme(args: Value, config: &AppConfig, paths: &SaiPaths) -> Result<String> {
    let library = selected_library(&args, config);
    let source = expand_path(required_str(&args, "image")?);
    let metadata = std::fs::metadata(&source)
        .with_context(|| format!("failed to stat image {}", source.display()))?;
    if !metadata.is_file() {
        bail!("image path is not a file: {}", source.display())
    }
    let max_bytes = config
        .plugins
        .memes
        .max_image_mb
        .saturating_mul(1024 * 1024);
    if metadata.len() > max_bytes {
        bail!(
            "image too large: {} bytes; limit is {} MiB",
            metadata.len(),
            config.plugins.memes.max_image_mb
        )
    }
    let bytes = std::fs::read(&source)
        .with_context(|| format!("failed to read image {}", source.display()))?;
    let digest = Sha256::digest(&bytes);
    let hash = format!("{digest:x}");
    let id = format!("sha256:{hash}");
    if let Some(existing) = find_meme(paths, &library, &id)? {
        return Ok(json!({
            "success": true,
            "already_exists": true,
            "library": library,
            "id": id,
            "name": existing.item.name,
            "path": existing.path,
        })
        .to_string());
    }
    let ext = image_ext(&source)?;
    let mime_type = mime_from_ext(ext)?;
    let animated = ext == "gif";
    let user_dir = user_library_dir(paths, &library);
    let images_dir = user_dir.join("images");
    std::fs::create_dir_all(&images_dir)?;
    let target_file = format!("{}.{}", &hash[..16], ext);
    let target = images_dir.join(&target_file);
    std::fs::copy(&source, &target).with_context(|| {
        format!(
            "failed to copy image {} to {}",
            source.display(),
            target.display()
        )
    })?;
    let mut item = if has_supplied_metadata(&args) {
        item_from_args(
            &args,
            id.clone(),
            format!("images/{target_file}"),
            mime_type,
            animated,
        )?
    } else {
        match describe_meme_image(config, paths, &source).await {
            Ok(metadata) => item_from_metadata(
                id.clone(),
                format!("images/{target_file}"),
                mime_type,
                animated,
                metadata,
            )?,
            Err(err) => {
                let _ = std::fs::remove_file(&target);
                return Ok(json!({
                    "success": false,
                    "needs_user_info": true,
                    "message": "vision metadata generation failed; ask the user what the image shows and when to use it, then call add_meme again with metadata fields",
                    "error": err.to_string(),
                })
                .to_string());
            }
        }
    };
    item.file = format!("images/{target_file}");
    let mut index = load_index(&user_dir.join("index.json"))?.unwrap_or_else(|| MemeIndex {
        library: library.clone(),
        version: 2,
        memes: Vec::new(),
        disabled_ids: Vec::new(),
    });
    index.library = library.clone();
    index.version = 2;
    index.disabled_ids.retain(|value| !ids_match(value, &id));
    index.memes.retain(|meme| !ids_match(&meme.id, &id));
    index.memes.push(item.clone());
    save_index(&user_dir.join("index.json"), &index)?;
    Ok(json!({
        "success": true,
        "library": library,
        "id": item.id,
        "name": item.name,
        "path": target,
        "metadata": item,
    })
    .to_string())
}

async fn update_meme(args: Value, config: &AppConfig, paths: &SaiPaths) -> Result<String> {
    let library = selected_library(&args, config);
    let id = required_str(&args, "id")?;
    let existing =
        find_meme(paths, &library, id)?.with_context(|| format!("meme not found: {id}"))?;
    let id = existing.item.id.clone();
    let user_dir = user_library_dir(paths, &library);
    let mut index = load_index(&user_dir.join("index.json"))?.unwrap_or_else(|| MemeIndex {
        library: library.clone(),
        version: 2,
        memes: Vec::new(),
        disabled_ids: Vec::new(),
    });
    index.library = library.clone();
    index.version = 2;
    let mut item = existing.item;
    apply_updates(&mut item, &args);
    if !index.memes.iter().any(|meme| ids_match(&meme.id, &id)) {
        index.memes.push(item.clone());
    } else {
        for meme in &mut index.memes {
            if ids_match(&meme.id, &id) {
                *meme = item.clone();
                break;
            }
        }
    }
    if let Some(enabled) = args.get("enabled").and_then(Value::as_bool) {
        if enabled {
            index.disabled_ids.retain(|value| !ids_match(value, &id));
        } else if !index.disabled_ids.iter().any(|value| ids_match(value, &id)) {
            index.disabled_ids.push(id.clone());
        }
    }
    save_index(&user_dir.join("index.json"), &index)?;
    Ok(json!({ "success": true, "library": library, "id": id, "metadata": item }).to_string())
}

async fn delete_meme(args: Value, config: &AppConfig, paths: &SaiPaths) -> Result<String> {
    let library = selected_library(&args, config);
    let requested_id = required_str(&args, "id")?;
    let user_dir = user_library_dir(paths, &library);
    let index_path = user_dir.join("index.json");
    let mut index = load_index(&index_path)?.unwrap_or_else(|| MemeIndex {
        library: library.clone(),
        version: 2,
        memes: Vec::new(),
        disabled_ids: Vec::new(),
    });
    index.library = library.clone();
    index.version = 2;
    if let Some(pos) = index
        .memes
        .iter()
        .position(|meme| ids_match(&meme.id, requested_id))
    {
        let item = index.memes.remove(pos);
        let id = item.id.clone();
        let path = user_dir.join(&item.file);
        if path.is_file() {
            if args
                .get("hard_delete")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                std::fs::remove_file(&path)?;
            } else {
                trash::delete(&path)?;
            }
        }
        index.disabled_ids.retain(|value| !ids_match(value, &id));
        save_index(&index_path, &index)?;
        return Ok(
            json!({ "success": true, "library": library, "id": id, "action": "deleted_user_meme" })
                .to_string(),
        );
    }
    if let Some(meme) = find_meme(paths, &library, requested_id)? {
        let id = meme.item.id;
        if !index.disabled_ids.iter().any(|value| ids_match(value, &id)) {
            index.disabled_ids.push(id.clone());
        }
        save_index(&index_path, &index)?;
        return Ok(json!({ "success": true, "library": library, "id": id, "action": "disabled_builtin_meme" }).to_string());
    }
    bail!("meme not found: {requested_id}")
}


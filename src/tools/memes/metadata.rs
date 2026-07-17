fn mime_from_ext(ext: &str) -> Result<String> {
    Ok(match ext {
        "jpg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        value => bail!("unsupported image extension: {value}"),
    }
    .to_string())
}

fn has_supplied_metadata(args: &Value) -> bool {
    [
        "name_zh",
        "name_en",
        "description",
        "usage",
        "avoid",
        "tags",
    ]
    .iter()
    .any(|key| args.get(*key).is_some())
}

fn item_from_args(
    args: &Value,
    id: String,
    file: String,
    mime_type: String,
    animated: bool,
) -> Result<MemeItem> {
    let name = LocalizedName {
        zh: args
            .get("name_zh")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        en: args
            .get("name_en")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
    };
    let description = args
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let usage = args
        .get("usage")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if name.zh.is_empty() || description.is_empty() || usage.is_empty() {
        bail!("name_zh, description, and usage are required when supplying metadata manually")
    }
    Ok(MemeItem {
        id,
        name,
        file,
        mime_type,
        animated,
        description,
        usage,
        avoid: args
            .get("avoid")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        tags: string_array(args.get("tags")),
    })
}

fn item_from_metadata(
    id: String,
    file: String,
    mime_type: String,
    animated: bool,
    metadata: Value,
) -> Result<MemeItem> {
    let name = metadata.get("name").cloned().unwrap_or_default();
    let item = MemeItem {
        id,
        name: LocalizedName {
            zh: name
                .get("zh")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string(),
            en: name
                .get("en")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string(),
        },
        file,
        mime_type,
        animated,
        description: metadata
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        usage: metadata
            .get("usage")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        avoid: metadata
            .get("avoid")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        tags: string_array(metadata.get("tags")),
    };
    if item.name.zh.is_empty() || item.description.is_empty() || item.usage.is_empty() {
        bail!("vision metadata is incomplete")
    }
    Ok(item)
}

fn apply_updates(item: &mut MemeItem, args: &Value) {
    if let Some(value) = args
        .get("name_zh")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        item.name.zh = value.to_string();
    }
    if let Some(value) = args
        .get("name_en")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        item.name.en = value.to_string();
    }
    if let Some(value) = args
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        item.description = value.to_string();
    }
    if let Some(value) = args
        .get("usage")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        item.usage = value.to_string();
    }
    if let Some(value) = args.get("avoid").and_then(Value::as_str).map(str::trim) {
        item.avoid = value.to_string();
    }
    if args.get("tags").is_some() {
        item.tags = string_array(args.get("tags"));
    }
}

fn source_label(source: MemeSource) -> &'static str {
    match source {
        MemeSource::Builtin => "builtin",
        MemeSource::User => "user",
    }
}


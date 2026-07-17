fn local_image_data_url(path: &Path, size_bytes: usize) -> Result<String> {
    if size_bytes > 10 * 1024 * 1024 {
        bail!("image too large for vision screening: {size_bytes} bytes")
    }
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mime = detect_image_mime(&bytes, "", &path.display().to_string())
        .context("failed to detect image mime for vision screening")?;
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

fn clean_url(value: &str) -> String {
    html_unescape(value.trim())
}

fn clean_text(value: &str, max_chars: usize) -> String {
    let text = html_unescape(value)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if text.chars().count() <= max_chars {
        text
    } else {
        format!("{}...", text.chars().take(max_chars).collect::<String>())
    }
}

fn html_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn host_from_url(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    Some(rest.split('/').next()?.to_ascii_lowercase())
}

fn extension_for_mime(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => ".png",
        "image/gif" => ".gif",
        "image/webp" => ".webp",
        "image/bmp" => ".bmp",
        _ => ".jpg",
    }
}

fn format_bytes(size: usize) -> String {
    let mut value = size as f64;
    for unit in ["B", "KB", "MB", "GB"] {
        if value < 1024.0 || unit == "GB" {
            return if unit == "B" {
                format!("{size} B")
            } else {
                format!("{value:.1} {unit}")
            };
        }
        value /= 1024.0;
    }
    format!("{value:.1} GB")
}


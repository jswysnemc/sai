async fn download_and_store_images(
    config: &AppConfig,
    paths: &SaiPaths,
    client: &Client,
    cache_dir: &Path,
    query: &str,
    candidates: Vec<ImageCandidate>,
    count: usize,
    max_bytes: usize,
    progress: ToolProgress,
) -> Result<DownloadResult> {
    std::fs::create_dir_all(cache_dir)
        .with_context(|| format!("failed to create {}", cache_dir.display()))?;
    let mut stored = Vec::new();
    let mut seen_hashes = HashSet::new();
    let mut rejected_by_vision = 0;
    for candidate in candidates
        .into_iter()
        .take(image_download_probe_limit(count))
    {
        if stored.len() >= count {
            break;
        }
        progress.report(format!(
            "{} {}/{}",
            t("downloading images", "正在下载图片"),
            stored.len() + 1,
            count
        ));
        let Some(mut item) = download_candidate(client, cache_dir, candidate, max_bytes).await?
        else {
            continue;
        };
        if !seen_hashes.insert(item.sha256.clone()) {
            continue;
        }
        if vision_screening_available(config) {
            progress.report(format!(
                "{} {}/{}",
                t("reviewing images", "正在审核图片"),
                stored.len() + 1,
                count
            ));
        }
        item.vision = screen_image_with_vision(config, paths, query, &item).await;
        if item.vision.status == "success" && !item.vision.accepted {
            rejected_by_vision += 1;
            progress.report(format!(
                "{} {}",
                t("image rejected by review", "图片审核已拒绝"),
                rejected_by_vision
            ));
            continue;
        }
        stored.push(item);
        progress.report(format!(
            "{} {}/{}",
            t("accepted images", "已通过图片"),
            stored.len(),
            count
        ));
    }
    if stored.is_empty() {
        bail!("image search found candidates, but no image could be downloaded")
    }
    Ok(DownloadResult {
        images: stored,
        rejected_by_vision,
    })
}

async fn download_candidate(
    client: &Client,
    cache_dir: &Path,
    mut candidate: ImageCandidate,
    max_bytes: usize,
) -> Result<Option<StoredImage>> {
    let urls =
        if candidate.thumbnail_url.is_empty() || candidate.thumbnail_url == candidate.image_url {
            vec![(candidate.image_url.clone(), false)]
        } else {
            vec![
                (candidate.image_url.clone(), false),
                (candidate.thumbnail_url.clone(), true),
            ]
        };
    for (url, used_thumbnail) in urls {
        let Ok((bytes, final_url, content_type)) =
            download_image_bytes(client, &url, max_bytes).await
        else {
            continue;
        };
        let Some(mime_type) = detect_image_mime(&bytes, &content_type, &final_url) else {
            continue;
        };
        let (width, height) = detect_image_dimensions(&bytes, &mime_type);
        if width > 0 && height > 0 {
            candidate.width = width;
            candidate.height = height;
        }
        let sha256 = hex::encode(Sha256::digest(&bytes));
        let ext = extension_for_mime(&mime_type);
        let local_path = cache_dir.join(format!("webimg-{sha256}{ext}"));
        if !local_path.exists() {
            std::fs::write(&local_path, &bytes)
                .with_context(|| format!("failed to write {}", local_path.display()))?;
        }
        return Ok(Some(StoredImage {
            candidate,
            local_path,
            mime_type,
            size_bytes: bytes.len(),
            sha256,
            used_thumbnail,
            vision: VisionScreening::not_requested(),
        }));
    }
    Ok(None)
}

async fn download_image_bytes(
    client: &Client,
    url: &str,
    max_bytes: usize,
) -> Result<(Vec<u8>, String, String)> {
    let response = client
        .get(url)
        .headers(image_headers(""))
        .send()
        .await?
        .error_for_status()?;
    if response.content_length().unwrap_or(0) > max_bytes as u64 {
        bail!("image exceeds size limit")
    }
    let final_url = response.url().to_string();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let bytes = response.bytes().await?.to_vec();
    if bytes.is_empty() {
        bail!("image is empty")
    }
    if bytes.len() > max_bytes {
        bail!("image exceeds size limit")
    }
    Ok((bytes, final_url, content_type))
}

fn image_headers(referer: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::USER_AGENT, USER_AGENT.parse().unwrap());
    headers.insert(
        reqwest::header::ACCEPT,
        "text/html,application/json,text/javascript,image/avif,image/webp,image/apng,image/*,*/*;q=0.8"
            .parse()
            .unwrap(),
    );
    headers.insert(
        reqwest::header::ACCEPT_LANGUAGE,
        "zh-CN,zh;q=0.9,en;q=0.8".parse().unwrap(),
    );
    if !referer.is_empty() {
        headers.insert(reqwest::header::REFERER, referer.parse().unwrap());
    }
    headers
}

fn detect_image_mime(bytes: &[u8], content_type: &str, url: &str) -> Option<String> {
    let header = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if matches!(
        header.as_str(),
        "image/jpeg" | "image/jpg" | "image/png" | "image/gif" | "image/webp" | "image/bmp"
    ) {
        return Some(header);
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return Some("image/jpeg".to_string());
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png".to_string());
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif".to_string());
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Some("image/webp".to_string());
    }
    if bytes.starts_with(b"BM") {
        return Some("image/bmp".to_string());
    }
    let path = url.to_ascii_lowercase();
    if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        return Some("image/jpeg".to_string());
    }
    if path.ends_with(".png") {
        return Some("image/png".to_string());
    }
    if path.ends_with(".gif") {
        return Some("image/gif".to_string());
    }
    if path.ends_with(".webp") {
        return Some("image/webp".to_string());
    }
    if path.ends_with(".bmp") {
        return Some("image/bmp".to_string());
    }
    None
}

fn detect_image_dimensions(bytes: &[u8], mime_type: &str) -> (u32, u32) {
    match mime_type {
        "image/png" if bytes.len() >= 24 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") => (
            u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
            u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
        ),
        "image/gif"
            if bytes.len() >= 10
                && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) =>
        {
            (
                u16::from_le_bytes(bytes[6..8].try_into().unwrap()) as u32,
                u16::from_le_bytes(bytes[8..10].try_into().unwrap()) as u32,
            )
        }
        "image/bmp" if bytes.len() >= 26 && bytes.starts_with(b"BM") => (
            i32::from_le_bytes(bytes[18..22].try_into().unwrap()).unsigned_abs(),
            i32::from_le_bytes(bytes[22..26].try_into().unwrap()).unsigned_abs(),
        ),
        "image/webp"
            if bytes.len() >= 30
                && bytes.starts_with(b"RIFF")
                && bytes.get(8..12) == Some(b"WEBP") =>
        {
            detect_webp_dimensions(bytes)
        }
        "image/jpeg" | "image/jpg" if bytes.starts_with(b"\xff\xd8") => {
            detect_jpeg_dimensions(bytes)
        }
        _ => (0, 0),
    }
}

fn detect_webp_dimensions(bytes: &[u8]) -> (u32, u32) {
    match bytes.get(12..16) {
        Some(b"VP8X") if bytes.len() >= 30 => {
            let width = 1 + u32::from_le_bytes([bytes[24], bytes[25], bytes[26], 0]);
            let height = 1 + u32::from_le_bytes([bytes[27], bytes[28], bytes[29], 0]);
            (width, height)
        }
        Some(b"VP8 ") if bytes.len() >= 30 => {
            let width = u16::from_le_bytes([bytes[26], bytes[27]]) as u32 & 0x3fff;
            let height = u16::from_le_bytes([bytes[28], bytes[29]]) as u32 & 0x3fff;
            (width, height)
        }
        Some(b"VP8L") if bytes.len() >= 25 => {
            let width = 1 + (((bytes[22] as u32 & 0x3f) << 8) | bytes[21] as u32);
            let height = 1
                + (((bytes[24] as u32 & 0x0f) << 10)
                    | ((bytes[23] as u32) << 2)
                    | ((bytes[22] as u32 & 0xc0) >> 6));
            (width, height)
        }
        _ => (0, 0),
    }
}

fn detect_jpeg_dimensions(bytes: &[u8]) -> (u32, u32) {
    let mut index = 2;
    while index + 9 < bytes.len() {
        if bytes[index] != 0xff {
            index += 1;
            continue;
        }
        while index < bytes.len() && bytes[index] == 0xff {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        let marker = bytes[index];
        index += 1;
        if matches!(marker, 0xd8 | 0xd9 | 0x01) || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        if marker == 0xda || index + 2 > bytes.len() {
            break;
        }
        let length = u16::from_be_bytes([bytes[index], bytes[index + 1]]) as usize;
        if length < 2 || index + length > bytes.len() {
            break;
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) && index + 7 <= bytes.len()
        {
            let height = u16::from_be_bytes([bytes[index + 3], bytes[index + 4]]) as u32;
            let width = u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]) as u32;
            return (width, height);
        }
        index += length;
    }
    (0, 0)
}


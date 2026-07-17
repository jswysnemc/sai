fn rank_candidates(query: &str, candidates: &mut [ImageCandidate]) {
    candidates.sort_by(|left, right| {
        score_candidate(query, right)
            .partial_cmp(&score_candidate(query, left))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn score_candidate(query: &str, candidate: &ImageCandidate) -> f32 {
    let metadata = format!(
        "{} {} {}",
        candidate.title, candidate.page_url, candidate.image_url
    )
    .to_ascii_lowercase();
    let mut score = 0.0;
    for term in image_query_terms(query) {
        if candidate.title.to_ascii_lowercase().contains(&term) {
            score += 24.0;
        } else if metadata.contains(&term) {
            score += 10.0;
        }
    }
    let short = candidate.width.min(candidate.height);
    let area = candidate.width.saturating_mul(candidate.height);
    score += if short >= 900 {
        28.0
    } else if short >= 600 {
        24.0
    } else if short >= 300 {
        18.0
    } else if short >= 100 {
        4.0
    } else {
        -8.0
    };
    if area >= 1_000_000 {
        score += 7.0;
    }
    let noisy = [
        "thumb",
        "thumbnail",
        "sprite",
        "placeholder",
        "banner",
        "advert",
        "favicon",
    ];
    if noisy.iter().any(|term| metadata.contains(term)) {
        score -= 8.0;
    }
    if metadata.contains("avatar")
        && !query.contains("头像")
        && !query.to_ascii_lowercase().contains("avatar")
    {
        score -= 8.0;
    }
    score
}

fn image_query_terms(query: &str) -> Vec<String> {
    let generic = [
        "图片",
        "照片",
        "高清",
        "壁纸",
        "photo",
        "image",
        "images",
        "picture",
        "wallpaper",
        "hd",
        "4k",
    ];
    query
        .split(|ch: char| ch.is_whitespace() || ch.is_ascii_punctuation())
        .map(|term| term.trim().to_ascii_lowercase())
        .filter(|term| term.len() >= 2 && !generic.contains(&term.as_str()))
        .collect()
}

fn dedupe_candidates(candidates: Vec<ImageCandidate>) -> Vec<ImageCandidate> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for candidate in candidates {
        let key = candidate
            .image_url
            .split('?')
            .next()
            .unwrap_or(&candidate.image_url)
            .to_ascii_lowercase();
        if seen.insert(key) {
            deduped.push(candidate);
        }
    }
    deduped
}

fn image_candidate_pool_limit(count: usize) -> usize {
    count.max((count * 4).max(count + 8).min(30))
}

fn image_download_probe_limit(count: usize) -> usize {
    count.max((count * 4).max(count + 6).min(16))
}

fn candidate_json(candidate: ImageCandidate) -> Value {
    json!({
        "title": candidate.title,
        "page_url": candidate.page_url,
        "image_url": candidate.image_url,
        "thumbnail_url": candidate.thumbnail_url,
        "source": candidate.source,
        "width": candidate.width,
        "height": candidate.height,
        "search_description": candidate.search_description,
    })
}

fn stored_json(item: StoredImage) -> Value {
    json!({
        "title": item.candidate.title,
        "page_url": item.candidate.page_url,
        "image_url": item.candidate.image_url,
        "thumbnail_url": item.candidate.thumbnail_url,
        "source": item.candidate.source,
        "local_path": item.local_path,
        "mime_type": item.mime_type,
        "width": item.candidate.width,
        "height": item.candidate.height,
        "size_bytes": item.size_bytes,
        "size_human": format_bytes(item.size_bytes),
        "sha256": item.sha256,
        "used_thumbnail": item.used_thumbnail,
        "search_description": item.candidate.search_description,
        "vision": {
            "status": item.vision.status,
            "accepted": item.vision.accepted,
            "description": item.vision.description,
            "reason": item.vision.reason,
            "provider_id": item.vision.provider_id,
            "model": item.vision.model,
            "error": item.vision.error,
        },
    })
}


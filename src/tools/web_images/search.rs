async fn search_images(
    client: &Client,
    query: &str,
    count: usize,
    safe_search: bool,
) -> Result<Vec<ImageCandidate>> {
    let limit = image_candidate_pool_limit(count);
    let mut candidates = search_ddg_images(client, query, limit, safe_search)
        .await
        .unwrap_or_default();
    if candidates.len() < count {
        let fallback = search_bing_images(client, query, limit, safe_search)
            .await
            .unwrap_or_default();
        candidates.extend(fallback);
    }
    let mut candidates = dedupe_candidates(candidates);
    rank_candidates(query, &mut candidates);
    if candidates.is_empty() {
        bail!("image search returned no results")
    }
    Ok(candidates.into_iter().take(limit).collect())
}

async fn search_ddg_images(
    client: &Client,
    query: &str,
    limit: usize,
    safe_search: bool,
) -> Result<Vec<ImageCandidate>> {
    let page_url = format!(
        "https://duckduckgo.com/?q={}&iax=images&ia=images",
        urlencoding::encode(query)
    );
    let html = client
        .get("https://duckduckgo.com/")
        .query(&[("q", query), ("iax", "images"), ("ia", "images")])
        .headers(image_headers(""))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let vqd = extract_ddg_vqd(&html).context("DuckDuckGo image page did not return vqd")?;
    let response = client
        .get("https://duckduckgo.com/i.js")
        .query(&[
            ("q", query),
            ("o", "json"),
            ("p", if safe_search { "1" } else { "-1" }),
            ("s", "0"),
            ("u", "bing"),
            ("f", ",,"),
            ("l", "us-en"),
            ("vqd", vqd.as_str()),
        ])
        .headers(image_headers(&page_url))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    parse_ddg_results(&response, limit)
}

fn extract_ddg_vqd(html: &str) -> Option<String> {
    for marker in ["vqd=\"", "vqd='", "vqd:\"", "vqd: '"] {
        if let Some(start) = html.find(marker) {
            let rest = &html[start + marker.len()..];
            let value: String = rest
                .chars()
                .take_while(|ch| ch.is_ascii_digit() || *ch == '-')
                .collect();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    if let Some(start) = html.find("\"vqd\":\"") {
        let rest = &html[start + "\"vqd\":\"".len()..];
        let value: String = rest
            .chars()
            .take_while(|ch| ch.is_ascii_digit() || *ch == '-')
            .collect();
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

fn parse_ddg_results(text: &str, limit: usize) -> Result<Vec<ImageCandidate>> {
    let data: Value = serde_json::from_str(text)?;
    let results = data
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut candidates = Vec::new();
    for item in results.into_iter().take(limit) {
        if let Some(candidate) = build_candidate(
            item.get("title")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            item.get("url").and_then(Value::as_str).unwrap_or_default(),
            item.get("image")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            item.get("thumbnail")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            "DuckDuckGo Images",
            item.get("width").and_then(Value::as_u64).unwrap_or(0),
            item.get("height").and_then(Value::as_u64).unwrap_or(0),
            "",
        ) {
            candidates.push(candidate);
        }
    }
    Ok(candidates)
}

async fn search_bing_images(
    client: &Client,
    query: &str,
    limit: usize,
    safe_search: bool,
) -> Result<Vec<ImageCandidate>> {
    let mut request = client
        .get("https://www.bing.com/images/search")
        .query(&[("q", query), ("first", "1")])
        .headers(image_headers(""));
    if safe_search {
        request = request.query(&[("safeSearch", "Strict")]);
    }
    let html = request.send().await?.error_for_status()?.text().await?;
    Ok(parse_bing_results(&html, limit))
}

fn parse_bing_results(html: &str, limit: usize) -> Vec<ImageCandidate> {
    let mut candidates = Vec::new();
    let mut rest = html;
    while let Some(pos) = rest.find("<a") {
        rest = &rest[pos..];
        let Some(iusc_pos) = rest.find("class=\"iusc\"") else {
            if rest.len() <= 2 {
                break;
            }
            rest = &rest[2..];
            continue;
        };
        rest = &rest[iusc_pos..];
        let Some(m_pos) = rest.find("m=\"") else {
            rest = &rest[1..];
            continue;
        };
        let start = m_pos + 3;
        let Some(end) = rest[start..].find('"') else {
            break;
        };
        let raw = html_unescape(&rest[start..start + end]);
        if let Ok(data) = serde_json::from_str::<Value>(&raw) {
            if let Some(candidate) = build_candidate(
                data.get("t")
                    .or_else(|| data.get("desc"))
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                data.get("purl").and_then(Value::as_str).unwrap_or_default(),
                data.get("murl").and_then(Value::as_str).unwrap_or_default(),
                data.get("turl").and_then(Value::as_str).unwrap_or_default(),
                "Bing Images",
                data.get("w")
                    .or_else(|| data.get("expw"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                data.get("h")
                    .or_else(|| data.get("exph"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                data.get("desc").and_then(Value::as_str).unwrap_or_default(),
            ) {
                candidates.push(candidate);
            }
        }
        if candidates.len() >= limit {
            break;
        }
        rest = &rest[start + end..];
    }
    candidates
}

fn build_candidate(
    title: &str,
    page_url: &str,
    image_url: &str,
    thumbnail_url: &str,
    source: &str,
    width: u64,
    height: u64,
    extra_description: &str,
) -> Option<ImageCandidate> {
    let image_url = clean_url(image_url);
    if !image_url.starts_with("http://") && !image_url.starts_with("https://") {
        return None;
    }
    let title = clean_text(title, 180);
    let page_url = clean_url(page_url);
    let thumbnail_url = clean_url(thumbnail_url);
    let mut description_parts = vec![title.clone(), clean_text(extra_description, 180)];
    if let Some(host) = host_from_url(&page_url) {
        description_parts.push(format!("来源页面: {host}"));
    }
    let search_description = clean_text(
        &description_parts
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("；"),
        420,
    );
    Some(ImageCandidate {
        title,
        page_url,
        image_url,
        thumbnail_url,
        source: source.to_string(),
        width: width.min(u32::MAX as u64) as u32,
        height: height.min(u32::MAX as u64) as u32,
        search_description,
    })
}


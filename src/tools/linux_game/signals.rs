async fn gather_linux_game_compatibility_signals(args: Value) -> Result<String> {
    let game = required(&args, "game")?;
    let candidates = game_candidates(&game);
    let search_game = candidates.first().cloned().unwrap_or_else(|| game.clone());
    let issue = args
        .get("issue")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("sai-linux-game-compatibility/0.1")
        .build()?;
    let (steam, steam_attempts) = steam_search_candidates(&client, &candidates).await;
    let appid = steam["appid"].as_u64();
    let steam_name = steam["name"].as_str().unwrap_or(&game).to_string();
    let mut slug_candidates = slug_candidates(&candidates);
    if appid.is_some() {
        slug_candidates.insert(0, slugify(&steam_name));
    }
    slug_candidates.sort();
    slug_candidates.dedup();
    let protondb = if let Some(appid) = appid {
        fetch_json(
            &client,
            &format!("https://www.protondb.com/api/v1/reports/summaries/{appid}.json"),
        )
        .await
        .ok()
    } else {
        None
    };
    let can_i_play_result = fetch_first_text(&client, &slug_candidates, |slug| {
        format!("https://caniplayonlinux.com/games/{slug}/")
    })
    .await;
    let anticheat_result = fetch_first_text(&client, &slug_candidates, |slug| {
        format!("https://areweanticheatyet.com/game/{slug}")
    })
    .await;
    let can_i_play = can_i_play_result.text.as_deref();
    let anticheat = anticheat_result.text.as_deref();
    let verdict = verdict(&protondb, can_i_play, anticheat, &issue);
    let confidence = compatibility_confidence(appid, &protondb, can_i_play, anticheat, &verdict);
    let needs_followup = confidence["needs_followup"].as_bool().unwrap_or(true);
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "game_query": game,
        "search_query": search_game,
        "query_candidates": candidates,
        "matched_name": steam_name,
        "steam": steam,
        "source_attempts": {
            "steam": steam_attempts,
            "can_i_play_on_linux": can_i_play_result.attempts,
            "are_we_anticheat_yet": anticheat_result.attempts,
        },
        "verdict": verdict,
        "confidence": confidence,
        "needs_followup": needs_followup,
        "protondb": protondb,
        "can_i_play_on_linux": can_i_play.map(extract_can_i_play_summary),
        "are_we_anticheat_yet": anticheat.map(extract_anticheat_summary),
        "sources": {
            "steam": appid.map(|id| format!("https://store.steampowered.com/app/{id}/")),
            "protondb": appid.map(|id| format!("https://www.protondb.com/app/{id}")),
            "can_i_play_on_linux": can_i_play_result.url,
            "are_we_anticheat_yet": anticheat_result.url,
        },
        "methodology": "If ProtonDB exists, use ProtonDB reports/comments as the primary practical playability signal. If ProtonDB is missing or insufficient, continue with web_search/web_fetch outside this tool. Keep final answer concise and include 调查结果, 依据, 怎么玩, 注意事项.",
    }))?)
}

#[derive(Default)]
struct TextFetchResult {
    text: Option<String>,
    url: Option<String>,
    attempts: Vec<Value>,
}

fn game_candidates(game: &str) -> Vec<String> {
    let normalized = normalize_game_query(game);
    let mut candidates = vec![normalized];
    candidates.retain(|candidate| !candidate.trim().is_empty());
    candidates.sort();
    candidates.dedup();
    candidates
}

fn slug_candidates(candidates: &[String]) -> Vec<String> {
    let mut slugs = candidates
        .iter()
        .map(|candidate| slugify(candidate))
        .filter(|slug| !slug.is_empty())
        .collect::<Vec<_>>();
    slugs.sort();
    slugs.dedup();
    slugs
}

fn normalize_game_query(game: &str) -> String {
    let compact = game
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    if compact.contains("赛博朋克2077")
        || compact.contains("电驭叛客2077")
        || compact.contains("cyberpunk2077")
    {
        return "Cyberpunk 2077".to_string();
    }
    if compact.contains("原神") || compact.contains("genshinimpact") {
        return "Genshin Impact".to_string();
    }
    game.trim().to_string()
}

async fn steam_search_candidates(
    client: &reqwest::Client,
    candidates: &[String],
) -> (Value, Vec<Value>) {
    let mut attempts = Vec::new();
    for candidate in candidates {
        match steam_search(client, candidate).await {
            Ok(value) => {
                attempts.push(json!({"query": candidate, "ok": true, "appid": value["appid"], "name": value["name"]}));
                return (value, attempts);
            }
            Err(err) => {
                attempts.push(json!({"query": candidate, "ok": false, "error": err.to_string()}))
            }
        }
    }
    (Value::Null, attempts)
}

async fn fetch_first_text<F>(
    client: &reqwest::Client,
    slugs: &[String],
    url_for_slug: F,
) -> TextFetchResult
where
    F: Fn(&str) -> String,
{
    let mut result = TextFetchResult::default();
    for slug in slugs {
        let url = url_for_slug(slug);
        match fetch_text(client, &url).await {
            Ok(text) => {
                result
                    .attempts
                    .push(json!({"slug": slug, "url": url, "ok": true}));
                result.url = Some(url);
                result.text = Some(text);
                return result;
            }
            Err(err) => result
                .attempts
                .push(json!({"slug": slug, "url": url, "ok": false, "error": err.to_string()})),
        }
    }
    result
}

async fn steam_search(client: &reqwest::Client, game: &str) -> Result<Value> {
    let value: Value = client
        .get("https://store.steampowered.com/api/storesearch/")
        .query(&[("term", game), ("l", "english"), ("cc", "US")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let item = value["items"]
        .as_array()
        .and_then(|items| items.first())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Steam app not found for {game}"))?;
    Ok(json!({"appid": item["id"], "name": item["name"], "url": item["tiny_image"]}))
}

async fn fetch_json(client: &reqwest::Client, url: &str) -> Result<Value> {
    Ok(client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String> {
    Ok(client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?)
}

fn verdict(
    protondb: &Option<Value>,
    can_i_play: Option<&str>,
    anticheat: Option<&str>,
    issue: &str,
) -> Value {
    let issue_lower = issue.to_ascii_lowercase();
    let multiplayer_sensitive = issue_lower.contains("multi")
        || issue_lower.contains("online")
        || issue.contains("联机")
        || issue.contains("多人")
        || issue.contains("反作弊");
    let anticheat_denied = anticheat
        .map(|text| text.contains("Denied") || text.contains("Broken"))
        .unwrap_or(false);
    if multiplayer_sensitive && anticheat_denied {
        return json!({"traffic_light":"🔴", "label":"不可玩", "reason":"anti-cheat denied or broken for multiplayer/online use"});
    }
    if can_i_play
        .map(|text| text.contains("Broken"))
        .unwrap_or(false)
    {
        return json!({"traffic_light":"🔴", "label":"不可玩", "reason":"Can I Play on Linux marks it broken"});
    }
    let tier = protondb
        .as_ref()
        .and_then(|value| value["tier"].as_str())
        .unwrap_or_default();
    if matches!(tier, "platinum" | "gold")
        || can_i_play
            .map(|text| text.contains("Works"))
            .unwrap_or(false)
    {
        return json!({"traffic_light":"🟢", "label":"可玩", "reason":"ProtonDB/Can I Play on Linux indicate it works"});
    }
    if matches!(tier, "silver" | "bronze")
        || can_i_play
            .map(|text| text.contains("Partial"))
            .unwrap_or(false)
    {
        return json!({"traffic_light":"🟡", "label":"不一定能玩", "reason":"partial or lower confidence compatibility"});
    }
    json!({"traffic_light":"🟡", "label":"不一定能玩", "reason":"insufficient compatibility data"})
}

fn compatibility_confidence(
    appid: Option<u64>,
    protondb: &Option<Value>,
    can_i_play: Option<&str>,
    anticheat: Option<&str>,
    verdict: &Value,
) -> Value {
    let tier = protondb
        .as_ref()
        .and_then(|value| value["tier"].as_str())
        .unwrap_or_default();
    let has_protondb = protondb.is_some();
    let has_can_i_play = can_i_play.is_some();
    let has_anticheat = anticheat.is_some();
    let can_i_play_works = can_i_play
        .map(|text| text.contains("Works"))
        .unwrap_or(false);
    let can_i_play_partial = can_i_play
        .map(|text| text.contains("Partial"))
        .unwrap_or(false);
    let reason = verdict["reason"].as_str().unwrap_or_default();
    let mut reasons = Vec::new();
    if appid.is_none() {
        reasons.push("Steam app id was not found");
    }
    if !has_protondb {
        reasons.push("ProtonDB data is missing");
    }
    if !has_can_i_play {
        reasons.push("Can I Play on Linux data is missing");
    }
    if !has_anticheat {
        reasons.push("AreWeAntiCheatYet data is missing");
    }
    if reason.contains("insufficient") {
        reasons.push("compatibility data is insufficient");
    }

    let confidence = if appid.is_some()
        && matches!(tier, "platinum" | "gold")
        && can_i_play_works
        && has_anticheat
    {
        "high"
    } else if matches!(tier, "platinum" | "gold" | "silver" | "bronze")
        || can_i_play_partial
        || can_i_play_works
    {
        "medium"
    } else {
        "low"
    };
    let needs_followup =
        confidence == "low" || reason.contains("insufficient") || !reasons.is_empty();
    json!({
        "level": confidence,
        "needs_followup": needs_followup,
        "followup_reason": if reasons.is_empty() { Value::Null } else { json!(reasons.join("; ")) },
        "source_coverage": {
            "steam_appid": appid.is_some(),
            "protondb": has_protondb,
            "can_i_play_on_linux": has_can_i_play,
            "are_we_anticheat_yet": has_anticheat
        },
        "suggested_followup_queries": [
            "ProtonDB game compatibility latest reports",
            "PCGamingWiki Linux Proton known issues",
            "Steam Community Linux Proton performance issues"
        ]
    })
}

fn extract_can_i_play_summary(html: &str) -> Value {
    let text = html2text::from_read(html.as_bytes(), 120);
    json!({
        "works": text.contains("Works"),
        "partial": text.contains("Partial"),
        "broken": text.contains("Broken"),
        "source_recommended_proton": value_after_label(&text, "Recommended Proton"),
        "steam_deck_verified": text.contains("Steam Deck Verified"),
        "known_issues": section_excerpt(&text, "Known issues", "Fixes", 1200),
        "fixes": section_excerpt(&text, "Fixes", "Verdict", 1200),
        "text_excerpt": excerpt(&text, 2000),
    })
}

fn extract_anticheat_summary(html: &str) -> Value {
    let text = html2text::from_read(html.as_bytes(), 120);
    let status = ["Supported", "Running", "Planned", "Broken", "Denied"]
        .into_iter()
        .find(|status| text.contains(status));
    json!({
        "status": status,
        "mentions_eac": text.contains("Easy Anti-Cheat"),
        "mentions_battleye": text.contains("BattlEye"),
        "text_excerpt": excerpt(&text, 1600),
    })
}

fn value_after_label(text: &str, label: &str) -> Option<String> {
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    while let Some(line) = lines.next() {
        if line == label {
            return lines.next().map(|value| value.chars().take(120).collect());
        }
    }
    None
}

fn section_excerpt(text: &str, start: &str, end: &str, max_chars: usize) -> Option<String> {
    let after = text.split(start).nth(1)?;
    let section = after.split(end).next().unwrap_or(after);
    Some(excerpt(section, max_chars))
}

fn excerpt(text: &str, max_chars: usize) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_chars)
        .collect()
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in value.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn required(args: &Value, key: &str) -> Result<String> {
    let value = args
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if value.is_empty() {
        bail!("missing required argument: {key}")
    }
    Ok(value.to_string())
}


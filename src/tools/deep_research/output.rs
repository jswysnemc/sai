fn normalize_final_answer(draft: &str, state: &Arc<Mutex<ResearchState>>) -> Result<String> {
    let diagnostics = reference_diagnostics(draft, state);
    let mut answer = strip_reference_section(draft).trim().to_string();
    if !diagnostics.is_empty() {
        answer.push_str("\n\n## 引用校验提示\n");
        for item in diagnostics {
            answer.push_str(&format!("- {item}\n"));
        }
    }
    answer.push_str("\n\n## 参考资料\n");
    let state = state.lock().expect("deep research state lock");
    if state.references.is_empty() {
        answer.push_str("- 本次研究没有注册外部参考资料。\n");
    } else {
        for item in &state.references {
            let source = if !item.url.is_empty() {
                format!("[{}]({})", item.title, item.url)
            } else if !item.path.is_empty() {
                format!("{} ({})", item.title, item.path)
            } else {
                item.title.clone()
            };
            answer.push_str(&format!("- [{}] {}\n", item.marker, source));
        }
    }
    Ok(answer)
}

fn reference_diagnostics(draft: &str, state: &Arc<Mutex<ResearchState>>) -> Vec<String> {
    let state = state.lock().expect("deep research state lock");
    let known = state
        .references
        .iter()
        .map(|item| item.marker.as_str())
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    for marker in extract_markers(draft) {
        if !known.iter().any(|item| *item == marker) {
            diagnostics.push(format!("正文引用了未注册来源 [{marker}]。"));
        }
    }
    if draft.contains("http://") || draft.contains("https://") {
        diagnostics.push("正文中存在裸 URL；建议注册为 W 类型参考资料后使用编号引用。".to_string());
    }
    diagnostics
}

fn extract_markers(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in value.split('[').skip(1) {
        let Some(end) = part.find(']') else { continue };
        let marker = &part[..end];
        if marker.len() >= 2
            && matches!(marker.as_bytes()[0], b'R' | b'K' | b'W')
            && marker[1..].chars().all(|ch| ch.is_ascii_digit())
        {
            out.push(marker.to_string());
        }
    }
    out
}

fn strip_reference_section(value: &str) -> String {
    for heading in ["\n## 参考资料", "\n# 参考资料"] {
        if let Some(index) = value.find(heading) {
            return value[..index].to_string();
        }
    }
    value.to_string()
}

fn write_report(
    plugin: &DeepResearchPluginConfig,
    paths: &SaiPaths,
    topic: &str,
    final_answer: &str,
    state: &Arc<Mutex<ResearchState>>,
    stop_reason: &str,
    iterations: usize,
    state_for_stats: &Arc<Mutex<ResearchState>>,
) -> Result<PathBuf> {
    let output_dir = expand_output_dir(&plugin.output_dir, paths);
    std::fs::create_dir_all(&output_dir)?;
    let title = topic_title(state, topic);
    let filename = unique_report_filename(&output_dir, &title);
    let path = output_dir.join(filename);
    let stats = public_stats(state_for_stats);
    let report = format!(
        "---\ntopic: {}\ntopic_title: {}\ncreated_at: {}\nstop_reason: {}\niterations_used: {}\ntool_calls: {}\ntool_ok: {}\ntool_errors: {}\ntoken_estimate: {}\ntoken_estimate_method: {}\ntoken_estimate_is_actual: {}\n---\n\n{}\n",
        topic,
        title,
        Local::now().to_rfc3339(),
        stop_reason,
        iterations,
        stats["tool_calls"].as_u64().unwrap_or(0),
        stats["tool_ok"].as_u64().unwrap_or(0),
        stats["tool_errors"].as_u64().unwrap_or(0),
        stats["token_estimate"].as_u64().unwrap_or(0),
        stats["token_estimate_method"].as_str().unwrap_or("rough_char_estimate"),
        stats["token_estimate_is_actual"].as_bool().unwrap_or(false),
        final_answer.trim_end()
    );
    std::fs::write(&path, report)?;
    Ok(path)
}

fn public_sources(state: &Arc<Mutex<ResearchState>>) -> Vec<Value> {
    let state = state.lock().expect("deep research state lock");
    state.references.iter().map(|item| json!({"ref": item.marker, "type": item.kind, "title": item.title, "url": item.url, "path": item.path})).collect()
}

fn public_stats(state: &Arc<Mutex<ResearchState>>) -> Value {
    let state = state.lock().expect("deep research state lock");
    json!({
        "tool_calls": state.stats.tool_calls,
        "tool_ok": state.stats.tool_ok,
        "tool_errors": state.stats.tool_errors,
        "prompt_tokens": state.stats.prompt_tokens,
        "completion_tokens": state.stats.completion_tokens,
        "total_tokens": state.stats.total_tokens,
        "token_estimate": state.stats.token_estimate,
        "token_estimate_method": token_estimate_method_label(state.stats.token_estimate_method),
        "token_estimate_is_actual": state.stats.token_estimate_method == TokenEstimateMethod::ProviderUsage,
        "references": state.references.len(),
    })
}

fn token_estimate_method_label(method: TokenEstimateMethod) -> &'static str {
    match method {
        TokenEstimateMethod::ProviderUsage => "provider_usage",
        TokenEstimateMethod::ProviderUsagePlusEstimate => "provider_usage_plus_estimate",
        TokenEstimateMethod::RoughCharEstimate | TokenEstimateMethod::None => "rough_char_estimate",
    }
}

fn topic_title(state: &Arc<Mutex<ResearchState>>, topic: &str) -> String {
    let state = state.lock().expect("deep research state lock");
    if state.topic_title.trim().is_empty() {
        sanitize_title(topic, 40)
    } else {
        state.topic_title.clone()
    }
}

fn normalized_reference_kind(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "r" | "record" | "deep_record" => "R".to_string(),
        "k" | "knowledge" => "K".to_string(),
        _ => "W".to_string(),
    }
}

fn depth_default_revisions(depth: &str) -> usize {
    match depth {
        "minimal" => 1,
        "low" => 2,
        "medium" => 3,
        "xhigh" => usize::MAX,
        _ => 3,
    }
}

fn depth_default_tool_steps(depth: &str) -> usize {
    match depth {
        "minimal" => 8,
        "low" => 14,
        "medium" => 24,
        "xhigh" => 0,
        _ => 40,
    }
}

fn estimate_tokens(texts: &[&str]) -> u64 {
    crate::token_estimate::estimate_texts_tokens(texts)
}

fn format_token_count(tokens: u64, estimated: bool) -> String {
    let prefix = if estimated { "≈" } else { "" };
    if tokens >= 1_000_000 {
        format!("{prefix}{:.2}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{prefix}{:.1}K", tokens as f64 / 1_000.0)
    } else {
        format!("{prefix}{tokens}")
    }
}

fn clip_inline(value: &str, max_chars: usize) -> String {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.chars().count() <= max_chars {
        value
    } else {
        format!(
            "{}...",
            value
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect::<String>()
        )
    }
}

fn sanitize_title(value: &str, max_chars: usize) -> String {
    let title = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let title = title
        .trim_matches(|ch: char| ch == '#' || ch == '*' || ch == '`')
        .trim();
    let clipped = title.chars().take(max_chars).collect::<String>();
    if clipped.trim().is_empty() {
        "深度研究".to_string()
    } else {
        clipped
    }
}

fn sanitize_filename(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric()
            || matches!(ch, '-' | '_')
            || ('\u{4e00}'..='\u{9fff}').contains(&ch)
        {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('-');
        }
    }
    if out.is_empty() {
        "deep-research".to_string()
    } else {
        out.chars().take(80).collect()
    }
}

fn unique_report_filename(output_dir: &PathBuf, title: &str) -> String {
    let stem = sanitize_filename(&strip_title_date_prefix(title));
    let suffix = format!(
        "{}_{}",
        report_date_suffix(title).unwrap_or_else(|| Local::now().format("%Y%m%d").to_string()),
        Local::now().format("%H%M")
    );
    let filename = format!("{stem}_{suffix}.md");
    if !output_dir.join(&filename).exists() {
        return filename;
    }
    let seconds = Local::now().format("%S").to_string();
    format!("{stem}_{suffix}{seconds}.md")
}

fn report_date_suffix(value: &str) -> Option<String> {
    chinese_date_suffix(value).or_else(|| ascii_date_suffix(value))
}

fn chinese_date_suffix(value: &str) -> Option<String> {
    let chars = value.chars().collect::<Vec<_>>();
    let year_index = chars.iter().position(|ch| *ch == '年')?;
    let month_rel = chars[year_index + 1..].iter().position(|ch| *ch == '月')?;
    let month_index = year_index + 1 + month_rel;
    let day_rel = chars[month_index + 1..]
        .iter()
        .position(|ch| *ch == '日' || *ch == '号')?;
    let day_index = month_index + 1 + day_rel;
    if year_index != 4 || !chars[..year_index].iter().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let year = chars[..year_index].iter().collect::<String>();
    let month = chars[year_index + 1..month_index]
        .iter()
        .collect::<String>();
    let day = chars[month_index + 1..day_index].iter().collect::<String>();
    if month.is_empty()
        || day.is_empty()
        || !month.chars().all(|ch| ch.is_ascii_digit())
        || !day.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    Some(format!("{year}{:0>2}{:0>2}", month, day))
}

fn ascii_date_suffix(value: &str) -> Option<String> {
    let chars = value.chars().collect::<Vec<_>>();
    for start in 0..chars.len().saturating_sub(9) {
        if chars[start..start + 4].iter().all(|ch| ch.is_ascii_digit())
            && matches!(chars[start + 4], '-' | '/' | '.')
            && chars[start + 5..start + 7]
                .iter()
                .all(|ch| ch.is_ascii_digit())
            && matches!(chars[start + 7], '-' | '/' | '.')
            && chars[start + 8..start + 10]
                .iter()
                .all(|ch| ch.is_ascii_digit())
        {
            let year = chars[start..start + 4].iter().collect::<String>();
            let month = chars[start + 5..start + 7].iter().collect::<String>();
            let day = chars[start + 8..start + 10].iter().collect::<String>();
            return Some(format!("{year}{month}{day}"));
        }
    }
    None
}

fn strip_title_date_prefix(value: &str) -> String {
    let mut title = value.trim().to_string();
    title = strip_leading_ascii_date(&title);
    title = strip_leading_chinese_date(&title);
    title = strip_leading_weekday(&title);
    let title = title.trim_matches(|ch: char| {
        ch.is_whitespace() || matches!(ch, '-' | '_' | '，' | ',' | '：' | ':' | '|' | '｜')
    });
    if title.is_empty() {
        value.trim().to_string()
    } else {
        title.to_string()
    }
}

fn strip_leading_ascii_date(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() >= 10
        && chars[0..4].iter().all(|ch| ch.is_ascii_digit())
        && matches!(chars[4], '-' | '/' | '.')
        && chars[5..7].iter().all(|ch| ch.is_ascii_digit())
        && matches!(chars[7], '-' | '/' | '.')
        && chars[8..10].iter().all(|ch| ch.is_ascii_digit())
    {
        chars[10..].iter().collect()
    } else {
        value.to_string()
    }
}

fn strip_leading_chinese_date(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let Some(year_index) = chars.iter().position(|ch| *ch == '年') else {
        return value.to_string();
    };
    let Some(month_rel) = chars[year_index + 1..].iter().position(|ch| *ch == '月') else {
        return value.to_string();
    };
    let month_index = year_index + 1 + month_rel;
    let Some(day_rel) = chars[month_index + 1..]
        .iter()
        .position(|ch| *ch == '日' || *ch == '号')
    else {
        return value.to_string();
    };
    let day_index = month_index + 1 + day_rel;
    if year_index == 4
        && chars[..year_index].iter().all(|ch| ch.is_ascii_digit())
        && chars[year_index + 1..month_index]
            .iter()
            .all(|ch| ch.is_ascii_digit())
        && chars[month_index + 1..day_index]
            .iter()
            .all(|ch| ch.is_ascii_digit())
    {
        chars[day_index + 1..].iter().collect()
    } else {
        value.to_string()
    }
}

fn strip_leading_weekday(value: &str) -> String {
    let weekdays = [
        "星期一",
        "星期二",
        "星期三",
        "星期四",
        "星期五",
        "星期六",
        "星期日",
        "星期天",
        "周一",
        "周二",
        "周三",
        "周四",
        "周五",
        "周六",
        "周日",
        "周天",
    ];
    let mut title = value.trim_start();
    loop {
        let Some(weekday) = weekdays.iter().find(|weekday| title.starts_with(**weekday)) else {
            break;
        };
        title = title[weekday.len()..].trim_start();
    }
    title.to_string()
}

fn expand_output_dir(value: &str, paths: &SaiPaths) -> PathBuf {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
            return home.join(rest);
        }
    }
    if value.is_empty() {
        return paths.config_dir.join("deep-research");
    }
    PathBuf::from(value)
}


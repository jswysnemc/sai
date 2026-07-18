/// 将一轮对话提炼为可检索的日记摘要，避免整段原文入库。

/// 从用户消息与助手回复生成日记条目。
///
/// 参数:
/// - `created_at`: 事件时间（RFC3339 或可读时间）
/// - `user_message`: 用户原文
/// - `assistant_message`: 助手原文
///
/// 返回:
/// - `Some(摘要)` 值得记住；`None` 表示寒暄/噪声，应跳过
pub(super) fn summarize_episode(
    created_at: &str,
    user_message: &str,
    assistant_message: &str,
) -> Option<String> {
    let task = distill_user_task(user_message)?;
    let outcome = distill_assistant_outcome(assistant_message);
    let time = short_timestamp(created_at);
    let content = if outcome.is_empty() {
        format!("{time}：{task}")
    } else {
        format!("{time}：{task} → {outcome}")
    };
    let content = truncate_chars(&content, 320);
    if content.chars().count() < 8 {
        return None;
    }
    Some(content)
}

/// 提炼用户意图：去掉礼貌填充，保留任务主语。
fn distill_user_task(raw: &str) -> Option<String> {
    let text = compact_line(raw);
    if text.is_empty() {
        return None;
    }
    if is_trivial_user_message(&text) {
        return None;
    }
    let cleaned = strip_leading_fillers(&text);
    if cleaned.is_empty() || is_trivial_user_message(&cleaned) {
        return None;
    }
    Some(truncate_chars(&cleaned, 120))
}

/// 提炼助手结果：优先结论句，去掉大段代码与清单噪声。
fn distill_assistant_outcome(raw: &str) -> String {
    let stripped = strip_code_fences(raw);
    let stripped = strip_markdown_noise(&stripped);
    let text = compact_line(&stripped);
    if text.is_empty() {
        return String::new();
    }
    // 1. 优先抓带结论语气的句子
    if let Some(sentence) = find_outcome_sentence(&text) {
        return truncate_chars(&sentence, 180);
    }
    // 2. 否则取前两句有实质内容的句子
    let sentences = split_sentences(&text);
    let mut picked: Vec<String> = Vec::new();
    for sentence in sentences {
        let sentence = sentence.trim().to_string();
        if sentence.chars().count() < 4 {
            continue;
        }
        if looks_like_noise_sentence(&sentence) {
            continue;
        }
        picked.push(sentence);
        if picked.len() >= 2 {
            break;
        }
    }
    if picked.is_empty() {
        return truncate_chars(&text, 160);
    }
    truncate_chars(&picked.join("；"), 180)
}

fn is_trivial_user_message(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let compact: String = lower
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '！' && *ch != '!' && *ch != '？' && *ch != '?')
        .collect();
    matches!(
        compact.as_str(),
        "你好"
            | "您好"
            | "在吗"
            | "在"
            | "嗨"
            | "哈喽"
            | "hello"
            | "hi"
            | "hey"
            | "早上好"
            | "中午好"
            | "晚上好"
            | "谢谢"
            | "多谢"
            | "好的"
            | "ok"
            | "okay"
            | "嗯"
            | "哦"
            | "收到"
            | "继续"
    ) || compact.chars().count() <= 1
}

fn strip_leading_fillers(text: &str) -> String {
    let mut out = text.trim().to_string();
    let prefixes = [
        "你好，",
        "你好,",
        "您好，",
        "您好,",
        "嗨，",
        "嗨,",
        "请你",
        "请",
        "麻烦你",
        "麻烦",
        "帮我",
        "帮忙",
        "能不能",
        "可以",
        "你来",
        "你在搞啥,",
        "你在搞啥，",
        "你在搞啥",
    ];
    let mut changed = true;
    while changed {
        changed = false;
        for prefix in prefixes {
            if let Some(rest) = out.strip_prefix(prefix) {
                out = rest.trim_start().to_string();
                changed = true;
            }
        }
    }
    out
}

fn strip_code_fences(text: &str) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if !in_fence {
                out.push_str("[代码]");
                out.push(' ');
            }
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn strip_markdown_noise(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // 跳过纯标题装饰与分隔线
        if trimmed
            .chars()
            .all(|ch| matches!(ch, '#' | '-' | '=' | '*' | ' '))
        {
            continue;
        }
        let mut line = trimmed.to_string();
        while line.starts_with('#') {
            line = line.trim_start_matches('#').trim_start().to_string();
        }
        // 去掉加粗/行内代码标记
        line = line.replace("**", "").replace('`', "");
        // 列表前缀
        if let Some(rest) = line.strip_prefix("- ") {
            line = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("* ") {
            line = rest.to_string();
        }
        out.push_str(&line);
        out.push(' ');
    }
    out
}

fn find_outcome_sentence(text: &str) -> Option<String> {
    let keywords = [
        "已完成",
        "完成",
        "已改",
        "已修复",
        "修复",
        "提交",
        "原因",
        "定位到",
        "结果",
        "通过",
        "失败",
        "错误",
        "改为",
        "实现",
        "新增",
        "done",
        "fixed",
        "commit",
    ];
    for sentence in split_sentences(text) {
        let sentence = sentence.trim();
        if sentence.chars().count() < 6 || looks_like_noise_sentence(sentence) {
            continue;
        }
        let lower = sentence.to_ascii_lowercase();
        if keywords
            .iter()
            .any(|kw| lower.contains(&kw.to_ascii_lowercase()))
        {
            return Some(sentence.to_string());
        }
    }
    None
}

fn looks_like_noise_sentence(sentence: &str) -> bool {
    let lower = sentence.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.contains("```")
        || lower.contains("cargo:")
        || lower.contains("error[e")
        || sentence
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .count()
            < 2
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '。' | '！' | '？' | ';' | '；' | '\n') {
            let piece = current.trim().to_string();
            if !piece.is_empty() {
                parts.push(piece);
            }
            current.clear();
        }
    }
    let tail = current.trim();
    if !tail.is_empty() {
        parts.push(tail.to_string());
    }
    parts
}

fn short_timestamp(raw: &str) -> String {
    // RFC3339: 2026-07-18T05:15:48... → 2026-07-18 05:15
    if let Some((date, rest)) = raw.split_once('T') {
        let time = rest.get(..5).unwrap_or(rest);
        return format!("{date} {time}");
    }
    if raw.chars().count() > 16 {
        return raw.chars().take(16).collect();
    }
    raw.to_string()
}

#[cfg(test)]
mod episode_summary_tests {
    use super::*;

    #[test]
    fn skips_greeting_only_turns() {
        assert!(
            summarize_episode("2026-07-18T05:00:00Z", "你好", "你好！有什么需要帮忙的？").is_none()
        );
    }

    #[test]
    fn distills_task_and_outcome_without_raw_dump() {
        let user = "完成提交";
        let assistant = "## 提交完成\n\n- 提交：`3a85e86`\n- 信息：`feat: isolate subagents`\n\n```\ngit status\nclean\n```\n工作区已干净";
        let summary = summarize_episode("2026-07-18T04:59:52Z", user, assistant).unwrap();
        assert!(summary.contains("完成提交"));
        assert!(!summary.contains("git status"));
        assert!(
            summary.contains("3a85e86") || summary.contains("提交") || summary.contains("干净")
        );
        assert!(summary.chars().count() < 280);
    }

    #[test]
    fn strips_fillers_from_user_request() {
        let summary = summarize_episode(
            "2026-07-18T05:15:00Z",
            "你在搞啥,你来写一个新的todo啊,完成后再写一个",
            "已创建计划 A 并全部完成，随后创建计划 B。",
        )
        .unwrap();
        assert!(summary.contains("todo") || summary.contains("计划"));
        assert!(!summary.contains("你在搞啥"));
    }
}

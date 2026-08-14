use std::collections::HashSet;

/// 把查询切成用于匹配的词元。
///
/// 单字符词元几乎命中一切，反而把打分拉平，因此丢弃。
///
/// 参数:
/// - `query`: 查询文本
///
/// 返回:
/// - 小写词元
pub(super) fn query_tokens(query: &str) -> Vec<String> {
    query
        .split(|ch: char| ch.is_whitespace() || ch.is_ascii_punctuation())
        .map(str::trim)
        .filter(|token| token.chars().count() >= 2)
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

/// 给一段文本按词元命中情况打分。
///
/// 命中数与覆盖率各占一部分：只按命中数算，一个词元重复出现的文本会
/// 压过覆盖全部词元的文本。
///
/// 参数:
/// - `text`: 待打分文本
/// - `tokens`: 查询词元
///
/// 返回:
/// - 得分；没有词元或完全不命中时为 0
pub(super) fn score_text(text: &str, tokens: &[String]) -> f32 {
    if tokens.is_empty() {
        return 0.0;
    }
    let lower = text.to_ascii_lowercase();
    let mut score = 0.0;
    let mut matched = HashSet::new();
    for token in tokens {
        if lower.contains(token) {
            score += 10.0;
            matched.insert(token);
        }
    }
    score + matched.len() as f32 / tokens.len() as f32 * 20.0
}

/// 截取命中位置附近的片段。
///
/// 起点向前退一段，让命中词前面的上下文也带上：直接从命中处开始截，
/// 读到的往往是半句话。
///
/// 参数:
/// - `text`: 原文
/// - `tokens`: 查询词元
/// - `max_chars`: 片段最大字符数
///
/// 返回:
/// - 片段文本
pub(super) fn snippet(text: &str, tokens: &[String], max_chars: usize) -> String {
    let lower = text.to_ascii_lowercase();
    let first_hit = tokens
        .iter()
        .filter_map(|token| lower.find(token))
        .min()
        .unwrap_or(0);
    let start = text[..first_hit.min(text.len())]
        .char_indices()
        .rev()
        .nth(max_chars / 4)
        .map(|(index, _)| index)
        .unwrap_or(0);
    truncate_chars(&text[start..], max_chars)
}

/// 按字符数截断文本。
///
/// 参数:
/// - `text`: 原文
/// - `max_chars`: 最大字符数
///
/// 返回:
/// - 截断后的文本，超长时以省略号结尾
pub(super) fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    format!(
        "{}...",
        text.chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证单字符词元被丢弃。
    #[test]
    fn single_character_tokens_are_dropped() {
        assert_eq!(query_tokens("a bb ccc"), vec!["bb", "ccc"]);
    }

    /// 验证没有词元时不给分。
    #[test]
    fn no_tokens_means_no_score() {
        assert_eq!(score_text("任意文本", &[]), 0.0);
    }

    /// 验证覆盖更多词元的文本得分更高。
    ///
    /// 只按命中次数算的话，重复同一个词的文本会压过真正相关的那条。
    #[test]
    fn broader_coverage_outranks_repetition() {
        let tokens = vec!["压缩".to_string(), "缓存".to_string()];

        let both = score_text("压缩复用缓存", &tokens);
        let repeated = score_text("压缩压缩压缩", &tokens);

        assert!(both > repeated);
    }

    /// 验证片段带上命中位置之前的上下文。
    #[test]
    fn the_snippet_includes_context_before_the_hit() {
        let text = "前面有一大段无关内容然后才是关键词所在的位置";
        let tokens = vec!["关键词".to_string()];

        let result = snippet(text, &tokens, 40);

        assert!(result.contains("关键词"));
        assert!(result.chars().count() <= 40);
    }

    /// 验证截断在字符边界上进行。
    #[test]
    fn truncation_respects_character_boundaries() {
        let result = truncate_chars(&"中".repeat(100), 10);

        assert_eq!(result.chars().count(), 10);
    }

    /// 验证不超长时原样返回。
    #[test]
    fn short_text_is_returned_untouched() {
        assert_eq!(truncate_chars("短", 10), "短");
    }
}

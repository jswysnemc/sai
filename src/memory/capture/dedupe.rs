use super::super::model::{MemoryCandidate, MemoryItem};

/// 判定为同一条记忆的相似度下限。
const DUPLICATE_SIMILARITY: f64 = 0.82;

/// 候选与既有记忆的比对结论。
#[derive(Debug, Clone, PartialEq)]
pub enum DedupeOutcome {
    /// 库中没有等价记忆，按新记忆写入
    Insert,
    /// 命中既有记忆，更新它而不是新增
    Update { existing_id: i64 },
}

/// 在既有记忆中查找与候选等价的条目。
///
/// 同一个偏好在多轮对话里会被反复提到，每次都新增会让库里堆满近义重复，
/// 召回时又互相挤占名额。这里命中即更新，保持一条记忆一个事实。
///
/// 参数:
/// - `candidate`: 已归一化的候选记忆
/// - `existing`: 同类型同作用域下的既有记忆
///
/// 返回:
/// - 新增或更新的结论
pub fn evaluate(candidate: &MemoryCandidate, existing: &[MemoryItem]) -> DedupeOutcome {
    let candidate_tokens = tokenize(&candidate.content);
    if candidate_tokens.is_empty() {
        return DedupeOutcome::Insert;
    }
    let mut best: Option<(i64, f64)> = None;
    for item in existing {
        // 只在同类型同作用域内比对，跨类型的相似正文可能是不同性质的记忆
        if item.kind != candidate.kind || item.scope != candidate.scope {
            continue;
        }
        let similarity = jaccard(&candidate_tokens, &tokenize(&item.content));
        if similarity < DUPLICATE_SIMILARITY {
            continue;
        }
        if best.is_none_or(|(_, current)| similarity > current) {
            best = Some((item.id, similarity));
        }
    }
    match best {
        Some((existing_id, _)) => DedupeOutcome::Update { existing_id },
        None => DedupeOutcome::Insert,
    }
}

/// 把正文切成用于比对的词元集合。
///
/// 中英文混排下按空白切分不可靠，这里对非 ASCII 按字切分、ASCII 按词切分，
/// 兼顾两种书写系统。
///
/// 参数:
/// - `content`: 记忆正文
///
/// 返回:
/// - 去重后的词元集合
fn tokenize(content: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut ascii_word = String::new();
    for ch in content.chars() {
        if ch.is_ascii_alphanumeric() {
            ascii_word.push(ch.to_ascii_lowercase());
            continue;
        }
        if !ascii_word.is_empty() {
            tokens.push(std::mem::take(&mut ascii_word));
        }
        // 标点与空白不参与比对，其余非 ASCII 字符逐字成词
        if !ch.is_whitespace() && !ch.is_ascii_punctuation() && !is_cjk_punctuation(ch) {
            tokens.push(ch.to_string());
        }
    }
    if !ascii_word.is_empty() {
        tokens.push(ascii_word);
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

/// 判断字符是否为中文标点。
///
/// 参数:
/// - `ch`: 待判定字符
///
/// 返回:
/// - 属于中文标点时为 true
fn is_cjk_punctuation(ch: char) -> bool {
    matches!(ch, '\u{3000}'..='\u{303F}' | '\u{FF00}'..='\u{FFEF}')
}

/// 计算两个词元集合的 Jaccard 相似度。
///
/// 参数:
/// - `left`: 已排序去重的词元集合
/// - `right`: 已排序去重的词元集合
///
/// 返回:
/// - 相似度，取值 0.0 到 1.0
fn jaccard(left: &[String], right: &[String]) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.iter().filter(|token| right.contains(token)).count();
    let union = left.len() + right.len() - intersection;
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::model::{MemoryKind, MemoryScope};

    fn item(id: i64, content: &str) -> MemoryItem {
        MemoryItem {
            id,
            kind: MemoryKind::Preference,
            scope: MemoryScope::Global,
            content: content.to_string(),
            salience: 0.8,
            tags: Vec::new(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn candidate(content: &str) -> MemoryCandidate {
        MemoryCandidate {
            kind: MemoryKind::Preference,
            scope: MemoryScope::Global,
            content: content.to_string(),
            salience: 0.8,
            tags: Vec::new(),
        }
    }

    /// 验证措辞略有差异的同一偏好被判定为重复。
    #[test]
    fn near_identical_statements_update_instead_of_inserting() {
        let existing = vec![item(7, "用户在 Node 项目中一律使用 pnpm")];
        let outcome = evaluate(&candidate("用户在 Node 项目中一律使用 pnpm。"), &existing);
        assert_eq!(outcome, DedupeOutcome::Update { existing_id: 7 });
    }

    /// 验证不同内容按新增处理。
    #[test]
    fn distinct_statements_are_inserted() {
        let existing = vec![item(7, "用户在 Node 项目中一律使用 pnpm")];
        let outcome = evaluate(&candidate("用户的终端使用 zsh 并启用了 vi 模式"), &existing);
        assert_eq!(outcome, DedupeOutcome::Insert);
    }

    /// 验证跨类型的相似正文不会被误合并。
    #[test]
    fn similar_content_in_another_kind_is_not_merged() {
        let mut other = item(7, "用户在 Node 项目中一律使用 pnpm");
        other.kind = MemoryKind::Fact;
        let outcome = evaluate(&candidate("用户在 Node 项目中一律使用 pnpm"), &[other]);
        assert_eq!(outcome, DedupeOutcome::Insert);
    }

    /// 验证跨作用域的相同正文不会被误合并。
    #[test]
    fn same_content_in_another_scope_is_not_merged() {
        let mut other = item(7, "构建使用 vite 而非 webpack");
        other.scope = MemoryScope::from_stored(Some("/home/a"));
        let outcome = evaluate(&candidate("构建使用 vite 而非 webpack"), &[other]);
        assert_eq!(outcome, DedupeOutcome::Insert);
    }

    /// 验证空库直接新增。
    #[test]
    fn an_empty_store_always_inserts() {
        assert_eq!(
            evaluate(&candidate("任意内容都可以"), &[]),
            DedupeOutcome::Insert
        );
    }

    /// 验证中英文混排的正文可以切分比对。
    #[test]
    fn tokenizer_handles_mixed_scripts() {
        let tokens = tokenize("用户使用 pnpm 而非 npm");
        assert!(tokens.contains(&"pnpm".to_string()));
        assert!(tokens.contains(&"npm".to_string()));
        assert!(tokens.contains(&"用".to_string()));
    }
}

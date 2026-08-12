//! 行内精细差异：为配对的删除 / 新增行计算真正变化的片段。
//!
//! 算法取公共前缀 + 公共后缀（字符级），中间即变化区；边界回退到
//! 词边界，避免高亮块从单词中间开始。两行几乎完全不同时放弃行内
//! 高亮，整行底色本身已经表达了「这行被重写」。

use std::ops::Range;

/// 一对配对行的行内变化区（字符索引，半开区间）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct IntralinePair {
    /// 删除行中被替换的片段
    pub(super) old: Range<usize>,
    /// 新增行中替换后的片段
    pub(super) new: Range<usize>,
}

/// 公共部分（前缀 + 后缀）占比低于该百分比时放弃行内高亮。
const MIN_COMMON_PERCENT: usize = 25;

/// 计算配对行的行内变化区。
///
/// 参数:
/// - `old`: 删除行正文
/// - `new`: 新增行正文
///
/// 返回:
/// - 变化区间对；两行相同或相似度过低时返回 None
pub(super) fn intraline_pair(old: &str, new: &str) -> Option<IntralinePair> {
    if old == new {
        return None;
    }
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();

    // 1. 字符级公共前缀 / 后缀
    let mut prefix = 0usize;
    while prefix < old_chars.len()
        && prefix < new_chars.len()
        && old_chars[prefix] == new_chars[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix < old_chars.len().saturating_sub(prefix)
        && suffix < new_chars.len().saturating_sub(prefix)
        && old_chars[old_chars.len() - 1 - suffix] == new_chars[new_chars.len() - 1 - suffix]
    {
        suffix += 1;
    }

    // 2. 相似度门槛：公共部分太少说明整行被重写，行内高亮只会制造噪声
    let max_len = old_chars.len().max(new_chars.len());
    if max_len == 0 || (prefix + suffix) * 100 / max_len < MIN_COMMON_PERCENT {
        return None;
    }

    // 3. 边界回退到词边界：高亮块不从单词 / 数字中间开始
    let prefix = retreat_to_word_start(&old_chars, prefix);
    let suffix_limit = old_chars
        .len()
        .saturating_sub(prefix)
        .min(new_chars.len().saturating_sub(prefix));
    let suffix = advance_to_word_end(&old_chars, suffix.min(suffix_limit));

    let old_range = prefix..old_chars.len().saturating_sub(suffix).max(prefix);
    let new_range = prefix..new_chars.len().saturating_sub(suffix).max(prefix);
    if old_range.is_empty() && new_range.is_empty() {
        return None;
    }
    Some(IntralinePair {
        old: old_range,
        new: new_range,
    })
}

/// 词类字符：字母、数字与下划线视为同一词。
fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// 前缀边界落在单词中间时回退到词首。
///
/// 参数:
/// - `chars`: 参考行字符
/// - `prefix`: 字符级公共前缀长度
///
/// 返回:
/// - 对齐到词边界的前缀长度
fn retreat_to_word_start(chars: &[char], mut prefix: usize) -> usize {
    while prefix > 0
        && prefix < chars.len()
        && is_word_char(chars[prefix - 1])
        && is_word_char(chars[prefix])
    {
        prefix -= 1;
    }
    prefix
}

/// 后缀边界落在单词中间时缩短后缀（变化区向右扩展到词尾）。
///
/// 参数:
/// - `chars`: 参考行字符
/// - `suffix`: 字符级公共后缀长度
///
/// 返回:
/// - 对齐到词边界的后缀长度
fn advance_to_word_end(chars: &[char], mut suffix: usize) -> usize {
    while suffix > 0
        && suffix < chars.len()
        && is_word_char(chars[chars.len() - suffix])
        && is_word_char(chars[chars.len() - suffix - 1])
    {
        suffix -= 1;
    }
    suffix
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 中段替换：变化区覆盖被替换词，对齐到词边界。
    #[test]
    fn highlights_the_replaced_words_only() {
        let pair = intraline_pair(
            "let total = old_value + 1;",
            "let total = new_value + 1;",
        )
        .unwrap();

        let old_text: String = "let total = old_value + 1;"
            .chars()
            .skip(pair.old.start)
            .take(pair.old.len())
            .collect();
        let new_text: String = "let total = new_value + 1;"
            .chars()
            .skip(pair.new.start)
            .take(pair.new.len())
            .collect();
        assert_eq!(old_text, "old_value");
        assert_eq!(new_text, "new_value");
    }

    /// 纯插入：删除行变化区为空点，新增行覆盖插入片段。
    #[test]
    fn pure_insertion_marks_only_the_new_segment() {
        let pair = intraline_pair("call(a, c)", "call(a, b, c)").unwrap();

        assert!(pair.old.is_empty() || pair.old.len() < 4);
        assert!(!pair.new.is_empty());
    }

    /// 整行重写时不做行内高亮。
    #[test]
    fn rewritten_lines_skip_intraline_highlight() {
        assert_eq!(intraline_pair("completely different", "另一种写法"), None);
    }

    /// 相同行没有变化区。
    #[test]
    fn identical_lines_have_no_ranges() {
        assert_eq!(intraline_pair("same", "same"), None);
    }

    /// 词边界回退：公共前缀停在单词中间时整词纳入变化区。
    #[test]
    fn boundaries_snap_to_word_edges() {
        let old = "download them from GitHub";
        let new = "downloaded from official";
        if let Some(pair) = intraline_pair(old, new) {
            let old_chars: Vec<char> = old.chars().collect();
            // 变化区起点之前必须是词边界（行首或非词字符）
            if pair.old.start > 0 && pair.old.start < old_chars.len() {
                assert!(
                    !is_word_char(old_chars[pair.old.start - 1])
                        || !is_word_char(old_chars[pair.old.start])
                );
            }
        }
    }
}

use super::colors::{style_added_count, style_removed_count};
use super::model::preview_from_arguments;

use crate::llm::streamed_diff_counts;

/// 渲染实时增删统计的状态文本，样式与 diff 标题的增删计数一致。
///
/// 参数:
/// - `arguments`: 工具参数原始文本（可能是不完整的 JSON 前缀）
///
/// 返回:
/// - 形如 `+12 -3` 的 ANSI 着色文本；尚无可统计字段时返回空
pub(crate) fn streamed_diff_stat_status(arguments: &str) -> Option<String> {
    let (added, removed) = streamed_diff_counts(arguments)?;
    Some(format_diff_stat_status(added, removed))
}

/// 组装编辑工具状态行用的 `+N -M`：先取精确预览统计，再回退流式近似。
///
/// 预览与 diff 正文同源，数字必然和画出来的 +/- 行一致；参数流阶段 JSON
/// 未闭合、构建不出预览时才退回宽松扫描，让状态行随分片跳动。
///
/// 参数:
/// - `arguments`: 工具参数原文
///
/// 返回:
/// - 着色 `+N -M`；两边都算不出时返回空
pub(crate) fn edit_diff_stat_status(arguments: &str) -> Option<String> {
    preview_diff_stat_status(arguments).or_else(|| streamed_diff_stat_status(arguments))
}

/// 从可构建的 diff 预览统计增删行数。
///
/// 参数:
/// - `arguments`: 工具参数原文
///
/// 返回:
/// - 着色 `+N -M`；无法预览时返回空
pub(crate) fn preview_diff_stat_status(arguments: &str) -> Option<String> {
    let preview = preview_from_arguments(arguments).ok()?;
    let (added, removed) = preview.line_counts();
    Some(format_diff_stat_status(added, removed))
}

/// 将增删行数格式化为与 diff 标题一致的着色状态文本。
///
/// 参数:
/// - `added`: 新增行数
/// - `removed`: 删除行数
///
/// 返回:
/// - 形如 `+12 -3` 的 ANSI 着色文本
pub(crate) fn format_diff_stat_status(added: usize, removed: usize) -> String {
    format!(
        "{} {}",
        style_added_count(added),
        style_removed_count(removed)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 预览回退：完整 write 参数即使宽松扫描失败也能给出 +N -M。
    #[test]
    fn preview_stat_status_for_write_file() {
        let status = preview_diff_stat_status(r#"{"path":"a.rs","content":"l1\nl2\n"}"#).unwrap();
        assert!(
            status.contains("+2") || status.contains("+2\u{1b}"),
            "{status}"
        );
        assert!(status.contains("-0") || status.contains('0'), "{status}");
        let combined =
            edit_diff_stat_status(r#"{"path":"notes.md","content":"hello\nworld"}"#).unwrap();
        assert!(!combined.contains("run"), "{combined}");
        assert!(combined.contains('+'), "{combined}");
    }

    /// write_file 的 content 分片按新增行实时累计。
    ///
    /// 值一旦闭合，行数按 `lines()` 口径计：结尾换行不再补一行。
    #[test]
    fn counts_streamed_write_file_content() {
        // 未闭合：最后一行还在增长，按已到达的行数计
        assert_eq!(
            streamed_diff_counts(r#"{"path":"a.rs","content":"l1\nl2\nl3"#),
            Some((3, 0))
        );
        // 已闭合且以换行结尾：两行就是两行，不能报三行
        assert_eq!(
            streamed_diff_counts(r#"{"path":"a.rs","content":"l1\nl2\n"}"#),
            Some((2, 0))
        );
        // 已闭合且不以换行结尾
        assert_eq!(
            streamed_diff_counts(r#"{"path":"a.rs","content":"l1\nl2"}"#),
            Some((2, 0))
        );
    }

    /// str_replace 的 old/new 字段分别计删与增。
    #[test]
    fn counts_streamed_str_replace_fields() {
        assert_eq!(
            streamed_diff_counts(r#"{"path":"a.rs","old_string":"a\nb","new_string":"x\ny\nz"}"#),
            Some((3, 2))
        );
        // 结尾换行不补行，与 diff 正文画出的行数一致
        assert_eq!(
            streamed_diff_counts(r#"{"path":"a.rs","old_string":"a\nb\n","new_string":"x\n"}"#),
            Some((1, 2))
        );
        // new_string 尚未到达时先展示删除侧
        assert_eq!(
            streamed_diff_counts(r#"{"path":"a.rs","old_string":"a\nb"#),
            Some((0, 2))
        );
    }

    /// 字段尚未出现时不给出统计，让状态行回退到省略号。
    #[test]
    fn returns_none_before_any_countable_field() {
        assert_eq!(streamed_diff_counts(r#"{"path":"a.rs""#), None);
        assert_eq!(streamed_diff_counts(""), None);
    }

    /// 字面反斜杠的转义不误计为换行。
    #[test]
    fn escaped_backslash_is_not_a_newline() {
        // 内容为 `a\n`（字面反斜杠 + n），JSON 编码为 a\\n
        assert_eq!(
            streamed_diff_counts(r#"{"path":"a.rs","content":"a\\n"}"#),
            Some((1, 0))
        );
    }

    /// 空字符串值计 0 行。
    #[test]
    fn empty_value_counts_zero_lines() {
        assert_eq!(
            streamed_diff_counts(r#"{"path":"a.rs","content":""#),
            Some((0, 0))
        );
    }

    /// 状态文本使用与 diff 标题一致的增删配色。
    #[test]
    fn stat_status_uses_diff_count_colors() {
        let status = streamed_diff_stat_status(r#"{"path":"a.rs","content":"l1\nl2"#).unwrap();
        assert!(status.contains("\x1b[32m+2\x1b[0m"));
        assert!(status.contains("\x1b[31m-0\x1b[0m"));
    }
}

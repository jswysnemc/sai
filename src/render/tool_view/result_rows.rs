//! 结果正文的 ANSI 安全换行，续行保留列表和嵌套层级。

use crate::render::transcript::AnsiLine;

/// 【终端】【结果换行】按可见列数折行，样式序列不计宽度，续行与正文起点对齐。
/// 参数: `body` 为 ANSI 正文，`width` 为可用列数；返回: 已换行的正文行。
pub(super) fn wrap(body: &str, width: usize) -> Vec<String> {
    body.split('\n')
        .flat_map(|line| {
            let plain = crate::render::ansi_clip::strip_ansi(line);
            let leading = plain.bytes().take_while(|byte| *byte == b' ').count();
            let content = &plain[leading..];
            let digits = content.bytes().take_while(u8::is_ascii_digit).count();
            let marker = if digits > 0
                && (content[digits..].starts_with(". ") || content[digits..].starts_with(": "))
            {
                digits + 2
            } else {
                0
            };
            AnsiLine::wrap_block_with_continuation_indent(line, width, leading + marker)
                .into_iter()
                .map(|line| line.as_str().to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【字素回归】ZWJ 字形和组合字符保持完整，不在字素内部换行。
    /// 参数: 无；返回: 无。
    #[test]
    fn wrapping_preserves_grapheme_clusters() {
        let family = "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}";
        let text = format!("{}\x1b[33m{family}\x1b[0m", "a".repeat(36));
        let rows = wrap(&text, 38);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].contains(family));
        let combined = "e\u{301}";
        let rows = wrap(&format!("{}{}", "a".repeat(37), combined.repeat(2)), 38);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].contains(combined) && rows[1].contains(combined));
    }

    /// 【终端】【结果宽度测试】中文、ANSI 高亮和数字列表在窄屏仍保持安全宽度。
    /// 参数: 无；返回: 无。
    #[test]
    fn ansi_rows_keep_width_and_hanging_indent() {
        for width in [10, 38, 78, 118] {
            let text = format!("  12: \x1b[1;33m{}\x1b[0m", "中文匹配正文".repeat(20));
            let rows = wrap(&text, width);
            assert!(rows.len() > 1);
            for row in rows {
                assert!(
                    crate::render::ansi_clip::ansi_visible_width(&row) <= width,
                    "{row:?}"
                );
                assert!(
                    crate::render::ansi_clip::strip_ansi(&row).starts_with("      ")
                        || row.contains("12:")
                );
                assert!(!row.contains('\u{fffd}'));
            }
        }
    }
}

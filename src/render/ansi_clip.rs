use crate::render::ansi_style::{is_reset_sgr, update_active_sgr};
use unicode_width::UnicodeWidthChar;

/// 【终端】【ANSI 清除】移除文本中的所有 ANSI 转义序列，返回纯可视字符文本。
///
/// 参数:
/// - `text`: 包含 ANSI 转义序列的文本
///
/// 返回:
/// - 纯文本字符串
pub(crate) fn strip_ansi(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut index = 0usize;
    while index < text.len() {
        let ch = text[index..].chars().next().unwrap_or_default();
        if ch == '\x1b' {
            let end = crate::render::terminal_image::escape_sequence_end(text, index);
            index = end.max(index + ch.len_utf8());
            continue;
        }
        output.push(ch);
        index += ch.len_utf8();
    }
    output
}

/// 【终端】【ANSI 安全截断】在保留颜色样式的前提下按显示列数截断文本，超出时在末尾闭合样式。
///
/// 参数:
/// - `text`: 包含 ANSI 转义序列的原始文本
/// - `max_width`: 最大显示列数
/// - `ellipsis`: 截断时追加的省略符
///
/// 返回:
/// - 不超过最大列数且样式完整闭合的文本
pub(crate) fn clip_ansi_to_width(text: &str, max_width: usize, ellipsis: &str) -> String {
    let ellipsis_width = unicode_width::UnicodeWidthStr::width(ellipsis);
    let budget = max_width.saturating_sub(ellipsis_width);
    let mut visible_width = 0usize;
    let mut truncated = false;
    let mut output = String::new();
    let mut active_sgr = String::new();
    let mut index = 0usize;

    // 1. 【终端】【ANSI 扫描】丢弃孤立 ESC，完整序列更新样式且不计入列宽，避免截断后吞掉样式重置
    while index < text.len() {
        let ch = text[index..].chars().next().unwrap_or_default();
        if ch == '\x1b' {
            let end = crate::render::terminal_image::escape_sequence_end(text, index);
            if end == index + 1 {
                index = end;
                continue;
            }
            let sequence = &text[index..end];
            if sequence.ends_with('m') {
                update_active_sgr(&mut active_sgr, sequence);
            }
            output.push_str(sequence);
            index = end.max(index + ch.len_utf8());
            continue;
        }

        // 2. 【终端】【列数计算】检查单字符显示列数，达到预算上限即终止追加
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if visible_width.saturating_add(char_width) > budget {
            truncated = true;
            break;
        }

        output.push(ch);
        visible_width = visible_width.saturating_add(char_width);
        index += ch.len_utf8();
    }

    // 3. 【终端】【样式闭合】如果发生截断，补齐省略号并在存在未复位样式时重置
    if truncated {
        output.push_str(ellipsis);
        if !active_sgr.is_empty() && !is_reset_sgr(&active_sgr) {
            output.push_str("\x1b[0m");
        }
    }

    output
}

/// 【终端】【可见宽度】计算带 ANSI 转义序列文本的实际终端显示列数。
///
/// 参数:
/// - `text`: 输入文本
///
/// 返回:
/// - 纯可视列数
pub(crate) fn ansi_visible_width(text: &str) -> usize {
    let mut width = 0usize;
    let mut index = 0usize;
    while index < text.len() {
        let ch = text[index..].chars().next().unwrap_or_default();
        if ch == '\x1b' {
            let end = crate::render::terminal_image::escape_sequence_end(text, index);
            index = end.max(index + ch.len_utf8());
            continue;
        }
        width = width.saturating_add(UnicodeWidthChar::width(ch).unwrap_or(0));
        index += ch.len_utf8();
    }
    width
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【崩溃回归】复现字节位置 18 落在替换字符内部的输入，清除时保留可见正文。
    /// 参数：无；返回：无，UTF-8 切片越界或正文丢失时断言失败。
    #[test]
    fn strip_preserves_replacement_character_after_escape() {
        let text = "0123456789abcdef\x1b\u{fffd}tail";
        assert_eq!(strip_ansi(text), "0123456789abcdef\u{fffd}tail");
    }

    /// 【终端】【裁剪边界回归】混合样式、中文和损坏转义时，宽度计算及截断均保留完整字符。
    /// 参数：无；返回：无，可见列数或截断内容不符时断言失败。
    #[test]
    fn malformed_escape_is_safe_for_width_and_clipping() {
        let text = "\x1b[32m前\x1b\u{fffd}后\x1b[0m";
        assert_eq!(ansi_visible_width(text), 5);
        for (width, expected) in [(0, ""), (2, "前"), (3, "前\u{fffd}"), (5, "前\u{fffd}后")] {
            let clipped = clip_ansi_to_width(text, width, "");
            assert_eq!(strip_ansi(&clipped), expected);
            assert!(ansi_visible_width(&clipped) <= width);
            assert!(clipped.ends_with("\x1b[0m"));
        }
        let clipped = clip_ansi_to_width(text, 4, "…");
        assert_eq!(strip_ansi(&clipped), "前\u{fffd}…");
    }

    #[test]
    fn test_clip_ansi_preserves_style_and_resets() {
        let styled = "\x1b[32mHello, World!\x1b[0m";
        let clipped = clip_ansi_to_width(styled, 5, "");
        assert_eq!(clipped, "\x1b[32mHello\x1b[0m");
    }

    #[test]
    fn test_clip_ansi_short_text_remains_untouched() {
        let styled = "\x1b[31mError\x1b[0m";
        let clipped = clip_ansi_to_width(styled, 10, "...");
        assert_eq!(clipped, styled);
    }

    #[test]
    fn test_clip_ansi_with_cjk() {
        let styled = "\x1b[36m中文测试内容\x1b[0m";
        let clipped = clip_ansi_to_width(styled, 5, "");
        assert_eq!(clipped, "\x1b[36m中文\x1b[0m");
    }

    #[test]
    fn test_ansi_visible_width() {
        let styled = "\x1b[1;33mYellow\x1b[0m text";
        assert_eq!(ansi_visible_width(styled), 11);
    }

    #[test]
    fn test_strip_ansi() {
        let styled = "\x1b[1;32mSuccess:\x1b[0m done";
        assert_eq!(strip_ansi(styled), "Success: done");
    }
}

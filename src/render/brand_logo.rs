/// 终端原生 SAI 字标：等宽细线与圆角连接，四行内保持清晰轮廓。
/// 每行固定 21 列，使用标准箱线字符，无需图片协议或特殊图标字体。
const LOGO_LINES: [&str; 4] = [
    "╭────╴  ╭────╮  ╶─┬─╴",
    "╰────╮  │    │    │  ",
    "     │  ├────┤    │  ",
    "╶────╯  ╵    ╵  ╶─┴─╴",
];

/// 标志渲染所需的字符列数。
pub(crate) const LOGO_WIDTH: usize = 21;
/// 标志渲染所需的字符行数。
pub(crate) const LOGO_HEIGHT: usize = LOGO_LINES.len();

/// 【终端】【品牌标志】按行渲染 Sai 标志。
///
/// 参数:
/// - `style`: 字标线条使用的 ANSI 样式前缀
///
/// 返回:
/// - 每行等宽的 ANSI 文本，行数为 `LOGO_HEIGHT`
pub(crate) fn logo_lines(style: &str) -> Vec<String> {
    LOGO_LINES
        .iter()
        .map(|line| format!("{style}{line}\x1b[0m"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    /// 【终端】【品牌标志】验证标志行数、可见宽度与形状锚点。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn logo_lines_have_stable_shape_and_width() {
        let lines = logo_lines("\x1b[38;2;58;114;100m");

        assert_eq!(lines.len(), LOGO_HEIGHT);
        for line in &lines {
            let visible = strip_ansi(line);
            assert_eq!(
                UnicodeWidthStr::width(visible.as_str()),
                LOGO_WIDTH,
                "标志每行必须等宽"
            );
        }
        // 1. 圆角轮廓与同宽细线保持完整，避免退回断裂的半块字符
        assert!(strip_ansi(&lines[0]).contains("╭────╴"));
        assert!(strip_ansi(&lines[2]).contains("├────┤"));
        assert!(strip_ansi(&lines[3]).ends_with("╶─┴─╴"));
    }

    /// 【终端】【品牌标志】验证样式在每行结束后复位，不污染后续输出。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn logo_lines_reset_style_at_segment_end() {
        let lines = logo_lines("\x1b[36m");

        for line in &lines {
            // 每个样式起始都必须有配对的复位序列
            assert_eq!(
                line.matches("\x1b[36m").count(),
                line.matches("\x1b[0m").count(),
                "样式与复位序列必须配对: {line:?}"
            );
        }
    }

    /// 去除 ANSI 控制序列，仅保留可见字符。
    ///
    /// 参数:
    /// - `text`: 带样式的终端文本
    ///
    /// 返回:
    /// - 仅含可见字符的文本
    fn strip_ansi(text: &str) -> String {
        let mut output = String::new();
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\x1b' && chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
                continue;
            }
            output.push(ch);
        }
        output
    }
}

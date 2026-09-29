/// 终端原生 Sai 标志：单笔画圆角 S，右上方品牌绿圆点，与 Web 图标同形。
/// 每行固定 6 列、共 3 行，只用标准箱线字符与圆点，无需图片协议或图标字体。
const MARK_LINES: [&str; 3] = ["╭──╴ ", "╰──╮ ", "╶──╯ "];
/// 圆点所在行与列（相对标志左上角）。
const DOT_ROW: usize = 0;
const DOT: &str = "●";

/// 标志笔画颜色：比 Web --signal 提亮一档，浅灰与深色背景均可辨认。
pub(crate) const MARK_STYLE: &str = "\x1b[1m\x1b[38;2;94;196;168m";
/// 圆点颜色：品牌绿的高亮色。
const DOT_STYLE: &str = "\x1b[38;2;94;196;168m";

/// 标志渲染所需的字符列数。
pub(crate) const LOGO_WIDTH: usize = 6;
/// 标志渲染所需的字符行数。
pub(crate) const LOGO_HEIGHT: usize = MARK_LINES.len();

/// 【终端】【品牌标志】按行渲染 Sai 标志。
///
/// 参数:
/// - `style`: S 笔画使用的 ANSI 样式前缀
///
/// 返回:
/// - 每行等宽的 ANSI 文本，行数为 `LOGO_HEIGHT`
pub(crate) fn logo_lines(style: &str) -> Vec<String> {
    MARK_LINES
        .iter()
        .enumerate()
        .map(|(row, line)| {
            let stroke = line.trim_end();
            let padding = " ".repeat(LOGO_WIDTH - 1 - stroke.chars().count());
            // 1. 圆点只在首行最右列出现，其余行以空格补齐到固定宽度
            let tail = if row == DOT_ROW {
                format!("{DOT_STYLE}{DOT}\x1b[0m")
            } else {
                " ".to_string()
            };
            format!("{style}{stroke}\x1b[0m{padding}{tail}")
        })
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
        let lines = logo_lines(MARK_STYLE);

        assert_eq!(lines.len(), LOGO_HEIGHT);
        for line in &lines {
            let visible = strip_ansi(line);
            assert_eq!(
                UnicodeWidthStr::width(visible.as_str()),
                LOGO_WIDTH,
                "标志每行必须等宽"
            );
        }
        // 1. S 笔画上下两弧完整，圆点只在首行右端
        assert!(strip_ansi(&lines[0]).starts_with("╭──╴"));
        assert!(strip_ansi(&lines[0]).ends_with('●'));
        assert!(strip_ansi(&lines[1]).starts_with("╰──╮"));
        assert!(strip_ansi(&lines[2]).starts_with("╶──╯"));
        assert!(!strip_ansi(&lines[2]).contains('●'));
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
            // 每行以复位结尾或以空格补齐，样式不会延续到后续输出
            assert_eq!(
                line.matches("\x1b[36m").count(),
                1,
                "每行只使用一次笔画样式: {line:?}"
            );
            assert!(line.matches("\x1b[0m").count() >= 1);
            let last_reset = line.rfind("\x1b[0m").unwrap();
            assert!(line[last_reset..]
                .trim_start_matches("\x1b[0m")
                .trim()
                .is_empty());
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

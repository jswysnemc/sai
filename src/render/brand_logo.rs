/// Sai 品牌标志的静态文本行：半块字符（▀ ▄ █）拼出的 "Sai" 字母标。
///
/// 大写 S 全高，小写 a/i 取 x 字高，i 带点与衬线基座；取代旧的双箭头
/// 像素网格。每行固定 17 列，不足以空格补齐。Web 端 `sai-logo.tsx`
/// 使用同一套字符网格。
const LOGO_LINES: [&str; 4] = [
    "▄▀▀▀▀▄        ▄  ",
    "▀▄▄▄▄   ▀▀▀▀▄ ▄▄ ",
    "     █ ▄▀▀▀▀█  █ ",
    "▀▄▄▄▄▀ ▀▄▄▄██ ▄█▄",
];

/// 标志渲染所需的字符列数。
pub(crate) const LOGO_WIDTH: usize = 17;
/// 标志渲染所需的字符行数。
pub(crate) const LOGO_HEIGHT: usize = LOGO_LINES.len();

/// 【终端】【品牌标志】按行渲染 Sai 标志。
///
/// 参数:
/// - `style`: 实心块使用的 ANSI 样式前缀
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
        // 字母标锚点：S 上弧、i 的圆点、底部衬线基座
        assert_eq!(strip_ansi(&lines[0]), "▄▀▀▀▀▄        ▄  ");
        assert_eq!(strip_ansi(&lines[1]), "▀▄▄▄▄   ▀▀▀▀▄ ▄▄ ");
        assert_eq!(strip_ansi(&lines[2]), "     █ ▄▀▀▀▀█  █ ");
        assert_eq!(strip_ansi(&lines[3]), "▀▄▄▄▄▀ ▀▄▄▄██ ▄█▄");
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

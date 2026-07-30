/// Sai 品牌标志的单位网格：5 行 11 列，1 表示实心块。
///
/// 依次为字母 S、A、I（各占 3、3、1 列，字母间留 1 列间隙），
/// 末列是与字身分离的方形光标。与 Web 端 `sai-logo.tsx` 使用同一套
/// 网格坐标，两端形状严格一致。
const LOGO_GRID: [[u8; 11]; 5] = [
    [1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 0],
    [1, 0, 0, 0, 1, 0, 1, 0, 1, 0, 0],
    [1, 1, 1, 0, 1, 1, 1, 0, 1, 0, 0],
    [0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1],
    [1, 1, 1, 0, 1, 0, 1, 0, 1, 0, 1],
];

/// 每个网格单位在终端中占用的字符列数。
///
/// 字身已含 11 列，取 1 列可让整体宽度控制在窄终端也能容纳的范围内。
const CELL_COLUMNS: usize = 1;

/// 标志渲染所需的字符列数。
pub(crate) const LOGO_WIDTH: usize = LOGO_GRID[0].len() * CELL_COLUMNS;
/// 标志渲染所需的字符行数。
pub(crate) const LOGO_HEIGHT: usize = LOGO_GRID.len();

/// 【终端】【品牌标志】按行渲染 Sai 标志。
///
/// 参数:
/// - `style`: 实心块使用的 ANSI 样式前缀
///
/// 返回:
/// - 每行等宽的 ANSI 文本，行数为 `LOGO_HEIGHT`
pub(crate) fn logo_lines(style: &str) -> Vec<String> {
    LOGO_GRID
        .iter()
        .map(|row| render_logo_row(row, style))
        .collect()
}

/// 【终端】【品牌标志】渲染标志的单行。
///
/// 连续实心块合并到同一段样式内，避免逐格重复写入 ANSI 序列。
///
/// 参数:
/// - `row`: 单位网格的一行
/// - `style`: 实心块使用的 ANSI 样式前缀
///
/// 返回:
/// - 定宽的 ANSI 文本行
fn render_logo_row(row: &[u8; 11], style: &str) -> String {
    let mut output = String::new();
    let mut filled = false;
    for cell in row {
        // 1. 进入实心段时写入样式，离开时复位，保证行尾不残留颜色
        if *cell == 1 && !filled {
            output.push_str(style);
            filled = true;
        } else if *cell == 0 && filled {
            output.push_str("\x1b[0m");
            filled = false;
        }
        let glyph = if *cell == 1 { '█' } else { ' ' };
        for _ in 0..CELL_COLUMNS {
            output.push(glyph);
        }
    }
    if filled {
        output.push_str("\x1b[0m");
    }
    output
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
        // S 的中段缺口、A 的碗心以及 I 的独立竖笔共同构成可辨认的字身
        assert_eq!(strip_ansi(&lines[1]), "█   █ █ █  ");
        // 光标块位于末列，与 I 之间保留一列间隙
        assert_eq!(strip_ansi(&lines[3]), "  █ █ █ █ █");
    }

    /// 【终端】【品牌标志】验证样式在实心段结束后复位，不污染后续输出。
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

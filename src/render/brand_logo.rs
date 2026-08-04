/// Sai 品牌标志的单位网格：5 行 7 列，1 表示实心块。
///
/// 形态为双箭头（快进式提示符）加一枚落在基线上的实心光标块，
/// 取代旧的像素字母 SAI。箭头指向右表示推进，光标块点明终端属性；
/// 单格笔画保证低分辨率终端仍可识别。Web 端使用同一网格。
const LOGO_GRID: [[u8; 7]; 5] = [
    [1, 0, 1, 0, 0, 0, 0],
    [0, 1, 0, 1, 0, 0, 0],
    [0, 0, 1, 0, 1, 0, 0],
    [0, 1, 0, 1, 0, 1, 1],
    [1, 0, 1, 0, 0, 1, 1],
];

/// 每个网格单位在终端中占用的字符列数。
///
/// 字身已含 9 列，取 1 列可让整体宽度控制在启动面板也能容纳的范围内。
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
fn render_logo_row<const N: usize>(row: &[u8; N], style: &str) -> String {
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
        // 双箭头提示符：斜边逐行右移
        assert_eq!(strip_ansi(&lines[0]), "█ █    ");
        assert_eq!(strip_ansi(&lines[1]), " █ █   ");
        // 光标块只落在基线两行，保留终端品牌特征
        assert_eq!(strip_ansi(&lines[3]), " █ █ ██");
        assert_eq!(strip_ansi(&lines[4]), "█ █  ██");
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

//! 全屏正文的拖动选择：记录选区、渲染反色高亮、提取纯文本并写入剪贴板。
//!
//! 全屏开启了鼠标捕获，终端自带的拖选需要按住 Shift；这里在应用内实现选择，
//! 直接拖动即可选中并复制。

use crate::render::transcript::AnsiLine;
use base64::{engine::general_purpose, Engine as _};

/// 文档坐标：正文行号与显示列。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct TextPoint {
    pub(super) row: usize,
    pub(super) col: usize,
}

/// 拖动选区；`anchor` 为按下位置，`head` 为当前位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Selection {
    pub(super) anchor: TextPoint,
    pub(super) head: TextPoint,
}

/// 选中文字的反色开关。
const SELECT_ON: &str = "\x1b[7m";
const SELECT_OFF: &str = "\x1b[27m";

impl Selection {
    /// 返回按文档顺序排列的起止点，终点列为开区间。
    ///
    /// 返回:
    /// - `(起点, 终点)`
    pub(super) fn ordered(&self) -> (TextPoint, TextPoint) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    /// 返回指定行被选中的列区间 `[start, end)`。
    ///
    /// 参数:
    /// - `row`: 正文行号
    /// - `width`: 正文列数，整行选中时的右边界
    ///
    /// 返回:
    /// - 列区间；该行不在选区内时为 `None`
    pub(super) fn cols_on_row(&self, row: usize, width: usize) -> Option<(usize, usize)> {
        let (start, end) = self.ordered();
        if row < start.row || row > end.row {
            return None;
        }
        let from = if row == start.row { start.col } else { 0 };
        let to = if row == end.row { end.col } else { width };
        (to > from).then_some((from, to.min(width)))
    }

    /// 选区是否为空（按下后没有移动）。
    pub(super) fn is_empty(&self) -> bool {
        self.anchor == self.head
    }
}

/// 【全屏视图】【选区文本】按选区提取正文纯文本，行尾空白去除，行间用换行连接。
///
/// 参数:
/// - `lines`: 正文每行的 ANSI 文本，只读取选区覆盖的行
/// - `selection`: 选区
/// - `width`: 正文列数
///
/// 返回:
/// - 选中的文本
pub(super) fn selected_text(lines: &[AnsiLine], selection: &Selection, width: usize) -> String {
    let (start, end) = selection.ordered();
    let mut rows = Vec::new();
    let end_row = end.row.saturating_add(1).min(lines.len());
    let Some(selected_lines) = lines.get(start.row..end_row) else {
        return String::new();
    };
    // 1. 【全屏视图】【选区文本】先切片再清理样式，复制少量文本不扫描整份会话
    for (offset, line) in selected_lines.iter().enumerate() {
        let row = start.row + offset;
        let Some((from, to)) = selection.cols_on_row(row, width) else {
            continue;
        };
        let plain = crate::render::activity_animation::strip_ansi_for_test(line.as_str());
        // 2. 【全屏视图】【复制排版】去除正文基线空格，不裁掉代码或列表自身的相对缩进
        let margin = crate::render::content_indent::CONTENT_LEFT_INDENT;
        let from = if plain.starts_with(&" ".repeat(margin)) {
            from.max(margin)
        } else {
            from
        };
        rows.push(slice_cols(&plain, from, to).trim_end().to_string());
    }
    rows.join("\n")
}

/// 按显示列截取纯文本；宽字符只要起点落在区间内就整字保留。
///
/// 参数:
/// - `text`: 纯文本行
/// - `from`: 起始列
/// - `to`: 结束列（开区间）
///
/// 返回:
/// - 截取结果
pub(super) fn slice_cols(text: &str, from: usize, to: usize) -> String {
    let mut col = 0usize;
    let mut output = String::new();
    for ch in text.chars() {
        let width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if col >= to {
            break;
        }
        if col >= from || (width > 1 && col + width > from) {
            output.push(ch);
        }
        col += width;
    }
    output
}

/// 【全屏视图】【选区高亮】在一行 ANSI 文本的指定列区间加反色，保留原有样式与图片序列。
///
/// 原行中的 `\x1b[0m` 会清掉反色，因此每个转义序列之后都在区间内重新打开。
///
/// 参数:
/// - `line`: 已适配宽度的屏幕行
/// - `from`: 起始列
/// - `to`: 结束列（开区间）
///
/// 返回:
/// - 带高亮的屏幕行
pub(in crate::cli::repl_runtime) fn highlight_cols(line: &str, from: usize, to: usize) -> String {
    let mut output = String::with_capacity(line.len() + 16);
    let mut col = 0usize;
    let mut active = false;
    let mut index = 0usize;
    while index < line.len() {
        if line.as_bytes()[index] == 0x1b {
            let end = crate::render::terminal_image::escape_sequence_end(line, index)
                .max(index + 1)
                .min(line.len());
            output.push_str(&line[index..end]);
            if active {
                output.push_str(SELECT_ON);
            }
            index = end;
            continue;
        }
        let Some(ch) = line[index..].chars().next() else {
            break;
        };
        let width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        let inside = (col >= from || (width > 1 && col + width > from)) && col < to;
        if inside && !active {
            output.push_str(SELECT_ON);
            active = true;
        } else if !inside && active {
            output.push_str(SELECT_OFF);
            active = false;
        }
        output.push(ch);
        col += width;
        index += ch.len_utf8();
    }
    if active {
        output.push_str(SELECT_OFF);
    }
    output
}

/// 【全屏视图】【复制】生成 OSC 52 剪贴板写入序列。
///
/// 走终端协议而不是系统剪贴板库：SSH 远程会话里同样写到本地剪贴板，
/// 进程退出也不会带走剪贴板内容。
///
/// 参数:
/// - `text`: 待复制文本
///
/// 返回:
/// - OSC 52 序列
pub(super) fn osc52_copy(text: &str) -> String {
    format!(
        "\x1b]52;c;{}\x07",
        general_purpose::STANDARD.encode(text.as_bytes())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造选区。
    fn selection(anchor: (usize, usize), head: (usize, usize)) -> Selection {
        Selection {
            anchor: TextPoint {
                row: anchor.0,
                col: anchor.1,
            },
            head: TextPoint {
                row: head.0,
                col: head.1,
            },
        }
    }

    /// 验证跨行选区按行首、整行、行尾提取，反向拖动结果一致。
    #[test]
    fn extracts_multi_row_text_in_document_order() {
        let lines = vec![
            AnsiLine::new("alpha beta".to_string()),
            AnsiLine::new("gamma".to_string()),
            AnsiLine::new("delta epsilon".to_string()),
        ];
        let forward = selected_text(&lines, &selection((0, 6), (2, 5)), 20);
        assert_eq!(forward, "beta\ngamma\ndelta");
        let backward = selected_text(&lines, &selection((2, 5), (0, 6)), 20);
        assert_eq!(backward, forward);
    }

    /// 验证中文等宽字符按显示列截取。
    #[test]
    fn slices_wide_characters_by_display_columns() {
        let lines = vec![AnsiLine::new("中文ab测试".to_string())];
        assert_eq!(
            selected_text(&lines, &selection((0, 2), (0, 6)), 20),
            "文ab"
        );
    }

    /// 验证高亮跨过原有重置序列仍保持反色，区间外恢复。
    #[test]
    fn highlight_survives_embedded_resets() {
        let line = "a\x1b[31mbc\x1b[0mde";
        let output = highlight_cols(line, 1, 4);
        // 选区从 b 开始：原样式序列之后立即打开反色
        assert!(output.starts_with("a\x1b[31m\x1b[7mb"), "{output:?}");
        assert!(output.contains("\x1b[0m\x1b[7md"));
        assert!(output.ends_with("\x1b[27me"));
        assert_eq!(
            crate::render::activity_animation::strip_ansi_for_test(&output),
            "abcde"
        );
    }

    /// 验证空选区与区间计算。
    #[test]
    fn empty_selection_and_row_ranges() {
        assert!(selection((1, 3), (1, 3)).is_empty());
        let span = selection((1, 3), (3, 2));
        assert_eq!(span.cols_on_row(0, 10), None);
        assert_eq!(span.cols_on_row(1, 10), Some((3, 10)));
        assert_eq!(span.cols_on_row(2, 10), Some((0, 10)));
        assert_eq!(span.cols_on_row(3, 10), Some((0, 2)));
    }

    /// 验证 OSC 52 载荷为 Base64 编码。
    #[test]
    fn osc52_encodes_text() {
        assert_eq!(osc52_copy("hi"), "\x1b]52;c;aGk=\x07");
    }

    /// 【全屏视图】【选区文本】选中行去除 ANSI 样式并保留跨行空白语义。
    ///
    /// 参数: 无
    /// 返回: 无；样式泄漏或空行丢失时断言失败
    #[test]
    fn extracts_styled_rows_without_escape_sequences() {
        let lines = [
            AnsiLine::new("outside selection".to_string()),
            AnsiLine::new("\x1b[31malpha beta\x1b[0m  ".to_string()),
            AnsiLine::new(String::new()),
            AnsiLine::new("\x1b[1m中文ab\x1b[0m".to_string()),
        ];
        assert_eq!(
            selected_text(&lines, &selection((1, 6), (3, 4)), 20),
            "beta\n\n中文"
        );
    }

    /// 【全屏视图】【选区边界】空文档、过期行号和末尾越界不会产生无关文本或溢出。
    ///
    /// 参数: 无
    /// 返回: 无；越界或文本错误时断言失败
    #[test]
    fn clips_selection_to_existing_rows() {
        assert_eq!(selected_text(&[], &selection((0, 0), (1, 2)), 20), "");
        let lines = [AnsiLine::new("last line".to_string())];
        assert_eq!(selected_text(&lines, &selection((2, 0), (3, 2)), 20), "");
        assert_eq!(
            selected_text(&lines, &selection((0, 5), (usize::MAX, 2)), 20),
            "line"
        );
    }

    /// 【全屏视图】【布局复制】跨行和反向拖选移除两列布局边距，保留嵌套和代码缩进。
    /// 参数: 无；返回: 无，布局空格泄漏或内容缩进丢失时断言失败。
    #[test]
    fn copy_removes_layout_margin_but_preserves_content_indentation() {
        let lines = [
            AnsiLine::new("  1. heading".into()),
            AnsiLine::new("    child".into()),
            AnsiLine::new("      code".into()),
            AnsiLine::new("  last".into()),
        ];
        for span in [selection((0, 0), (3, 6)), selection((3, 6), (0, 0))] {
            assert_eq!(
                selected_text(&lines, &span, 30),
                "1. heading\n  child\n    code\nlast"
            );
        }
        assert_eq!(selected_text(&lines, &selection((2, 8), (2, 10)), 30), "de");
    }

    /// 【全屏视图】【宽字高亮】选区起点落在宽字符第二列时，整字高亮与复制结果一致。
    /// 参数: 无；返回: 无，中文字形缺失高亮时断言失败。
    #[test]
    fn highlight_includes_wide_character_overlapping_selection_start() {
        let text = highlight_cols("中文ab", 1, 4);
        assert!(text.starts_with("\x1b[7m中文\x1b[27ma"));
        assert_eq!(
            crate::render::activity_animation::strip_ansi_for_test(&text),
            "中文ab"
        );
    }
}

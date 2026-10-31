use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Glyph {
    columns: Range<usize>,
    bytes: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct InputLine {
    end: usize,
    glyphs: Vec<Glyph>,
}

/// 【终端】【输入命中】按实际可见窗口保存文字到屏幕的映射，软折行不改变复制内容。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::cli::repl_runtime) struct InputGeometry {
    pub(in crate::cli::repl_runtime) text: String,
    lines: Vec<InputLine>,
    first: usize,
    top: u16,
    left: usize,
    width: usize,
    visible: usize,
}

impl InputGeometry {
    /// 【终端】【底栏坐标】从已绘制行建立纯文本映射，保留行首空白和真实换行。
    /// 参数：lines 为底栏可见行，cols 为终端列数，top 为首行坐标；返回：可供拖选的文本映射。
    pub(in crate::cli::repl_runtime) fn from_rendered_lines(
        lines: &[String],
        cols: usize,
        top: u16,
    ) -> Self {
        let text = lines
            .iter()
            .map(|line| {
                crate::render::ansi_clip::strip_ansi(line)
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        // 1. 【终端】【物理行边界】已折行文本无需再次折行，满宽行也不能多生成一行光标占位
        Self::new(&text, cols.saturating_add(1), 0, top, 0, lines.len())
    }

    /// 【终端】【输入坐标】与输入折行使用相同列宽、制表位和满行光标规则。
    /// 参数: `input` 为草稿，`width` 为净宽，`first` 为首个视觉行，`top/left` 为屏幕原点，`visible` 为可见行数；返回: 坐标映射。
    pub(in crate::cli::repl_runtime) fn new(
        input: &str,
        width: usize,
        first: usize,
        top: u16,
        left: usize,
        visible: usize,
    ) -> Self {
        let text = crate::cli::repl_text::repl_input_lines(input).join("\n");
        let width = width.max(1);
        let mut lines = Vec::new();
        let mut glyphs: Vec<Glyph> = Vec::new();
        let mut column = 0;
        let mut end = 0;
        for (byte, ch) in text.char_indices() {
            if ch == '\n' {
                lines.push(InputLine {
                    end: byte,
                    glyphs: std::mem::take(&mut glyphs),
                });
                if column >= width {
                    lines.push(InputLine {
                        end: byte,
                        glyphs: Vec::new(),
                    });
                }
                column = 0;
                end = byte + 1;
                continue;
            }
            let cells = if ch == '\t' {
                ((column / 8 + 1) * 8)
                    .min(width.saturating_sub(1))
                    .saturating_sub(column)
            } else {
                crate::cli::repl_text::char_terminal_width(ch)
            };
            if cells > 0 && column + cells > width {
                lines.push(InputLine {
                    end: byte,
                    glyphs: std::mem::take(&mut glyphs),
                });
                column = 0;
            }
            end = byte + ch.len_utf8();
            if cells == 0 && !glyphs.is_empty() {
                glyphs.last_mut().unwrap().bytes.end = end;
            } else {
                glyphs.push(Glyph {
                    columns: column..column + cells,
                    bytes: byte..end,
                });
            }
            column += cells;
        }
        lines.push(InputLine { end, glyphs });
        if column >= width {
            lines.push(InputLine {
                end,
                glyphs: Vec::new(),
            });
        }
        let visible = visible.min(lines.len().saturating_sub(first));
        Self {
            text,
            lines,
            first,
            top,
            left,
            width,
            visible,
        }
    }

    /// 【终端】【光标命中】输入行内空白定位到行尾，空行定位到对应换行，区域外不命中。
    /// 参数: column/row 为屏幕坐标；返回: 输入文本的 UTF-8 字节边界。
    pub(in crate::cli::repl_runtime) fn cursor_hit(&self, column: u16, row: u16) -> Option<usize> {
        let offset = usize::from(row.checked_sub(self.top)?);
        let relative = usize::from(column).checked_sub(self.left)?;
        if offset >= self.visible || relative >= self.width {
            return None;
        }
        if self.text.is_empty() {
            return Some(0);
        }
        self.hit(column, row, true)
    }

    /// 【终端】【输入命中】拖动时夹到可见文字边界，首次按下必须落在真实文字区域。
    /// 参数: `column/row` 为屏幕坐标，`clamp` 为是否夹取边界；返回: UTF-8 字节边界。
    pub(in crate::cli::repl_runtime) fn hit(
        &self,
        column: u16,
        row: u16,
        clamp: bool,
    ) -> Option<usize> {
        if self.text.is_empty() || self.visible == 0 {
            return None;
        }
        let row = if clamp {
            row.clamp(self.top, self.top + self.visible as u16 - 1)
        } else {
            row
        };
        let offset = usize::from(row.checked_sub(self.top)?);
        if offset >= self.visible {
            return None;
        }
        let line = self.lines.get(self.first + offset)?;
        let column = if clamp {
            usize::from(column).saturating_sub(self.left)
        } else {
            usize::from(column).checked_sub(self.left)?
        };
        if !clamp && column >= line.glyphs.last().map_or(0, |glyph| glyph.columns.end) {
            return None;
        }
        Some(
            line.glyphs
                .iter()
                .find(|glyph| column < glyph.columns.end)
                .map_or(line.end, |glyph| glyph.bytes.start),
        )
    }

    /// 【终端】【选区投影】把源码选区映射到某一屏幕行，只覆盖真实文字。
    /// 参数: `row` 为屏幕行，`range` 为 UTF-8 字节选区；返回: 屏幕列区间。
    pub(in crate::cli::repl_runtime) fn columns(
        &self,
        row: u16,
        range: &Range<usize>,
    ) -> Option<Range<usize>> {
        let offset = usize::from(row.checked_sub(self.top)?);
        if offset >= self.visible {
            return None;
        }
        let line = self.lines.get(self.first + offset)?;
        let selected: Vec<_> = line
            .glyphs
            .iter()
            .filter(|glyph| glyph.bytes.start < range.end && glyph.bytes.end > range.start)
            .collect();
        Some(self.left + selected.first()?.columns.start..self.left + selected.last()?.columns.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【底栏映射回归】满宽行不插入视觉空行，中文和组合字符保持合法边界。
    /// 参数：无；返回：无，底栏行号偏移、样式泄漏或字符边界错误时断言失败。
    #[test]
    fn rendered_footer_rows_preserve_columns_and_physical_line_breaks() {
        let lines = [
            "\x1b[32mabcdefgh\x1b[0m".to_string(),
            " 中文e\u{301}  ".to_string(),
        ];
        let geometry = InputGeometry::from_rendered_lines(&lines, 8, 20);
        assert_eq!(geometry.text, "abcdefgh\n 中文e\u{301}");
        assert_eq!(geometry.hit(1, 21, false), Some(10));
        let start = geometry.hit(2, 21, false).unwrap();
        let end = geometry.hit(6, 21, true).unwrap();
        assert_eq!(&geometry.text[start..end], "中文e\u{301}");
        assert_eq!(geometry.columns(21, &(start..end)), Some(1..6));
        assert!(geometry.hit(30, 21, false).is_none());
    }

    /// 【终端】【输入映射回归】软折行复制不插入换行，真正的换行仍保留；无参数和返回值。
    #[test]
    fn copied_ranges_preserve_hard_breaks_not_visual_wraps() {
        let geometry = InputGeometry::new("abcdef\ngh", 3, 0, 10, 3, 5);
        let first = geometry.hit(3, 10, false).unwrap();
        let soft_end = geometry.hit(6, 11, true).unwrap();
        assert_eq!(&geometry.text[first..soft_end], "abcdef");
        let end = geometry.hit(5, 13, true).unwrap();
        assert_eq!(&geometry.text[first..end], "abcdef\ngh");
        assert!(geometry.hit(1, 10, false).is_none());
        assert!(geometry.hit(12, 13, false).is_none());
    }

    /// 【终端】【输入映射回归】中文、组合字符与裁切窗口不产生无效 UTF-8 边界；无参数和返回值。
    #[test]
    fn unicode_glyphs_and_scrolled_rows_keep_source_boundaries() {
        let geometry = InputGeometry::new("汉字e\u{301}测试", 4, 1, 10, 3, 2);
        let start = geometry.hit(3, 10, false).unwrap();
        let end = geometry.hit(4, 10, true).unwrap();
        assert_eq!(&geometry.text[start..end], "e\u{301}");
        assert_eq!(geometry.columns(10, &(start..end)), Some(3..4));
        let end = geometry.hit(40, 100, true).unwrap();
        assert_eq!(&geometry.text[start..end], "e\u{301}测试");
    }

    /// 【终端】【插入位置】空草稿、硬换行和软折行使用真实输入范围，外部坐标不夹进输入。
    /// 参数: 无；返回: 无。
    #[test]
    fn cursor_hits_blank_lines_soft_wraps_and_visible_window_only() {
        let empty = InputGeometry::new("", 20, 0, 10, 3, 1);
        assert_eq!(empty.cursor_hit(12, 10), Some(0));
        assert_eq!(empty.cursor_hit(2, 10), None);
        assert_eq!(empty.cursor_hit(23, 10), None);
        assert_eq!(empty.cursor_hit(12, 9), None);
        assert_eq!(empty.cursor_hit(12, 11), None);
        let wrapped = InputGeometry::new("abcdef\ngh", 3, 1, 10, 3, 3);
        assert_eq!(wrapped.cursor_hit(4, 10), Some(4));
        assert_eq!(wrapped.cursor_hit(3, 11), Some(6));
        assert_eq!(wrapped.cursor_hit(5, 12), Some(9));
        assert_eq!(wrapped.cursor_hit(10, 12), None);
    }
}

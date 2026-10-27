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
    visible: usize,
}

impl InputGeometry {
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
            visible,
        }
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
}

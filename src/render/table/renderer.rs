use super::{CellContent, TableAlign};
use crate::render::style::{RESET, TABLE_BORDER_STYLE};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const TOP_LEFT: char = '┌';
const TOP_MID: char = '┬';
const TOP_RIGHT: char = '┐';
const MID_LEFT: char = '├';
const MID_MID: char = '┼';
const MID_RIGHT: char = '┤';
const BOTTOM_LEFT: char = '└';
const BOTTOM_MID: char = '┴';
const BOTTOM_RIGHT: char = '┘';
const HORIZONTAL: char = '─';
const VERTICAL: char = '│';

/// 渲染单个表格数据行。
///
/// 参数:
/// - `row`: 当前数据行
/// - `widths`: 最终列宽
/// - `alignments`: 每列对齐方式
/// - `header`: 是否为表头
///
/// 返回:
/// - 带边框的表格行文本
pub(crate) fn render_table_row(
    row: &[CellContent],
    widths: &[usize],
    alignments: &[TableAlign],
    header: bool,
) -> String {
    let wrapped: Vec<Vec<String>> = widths
        .iter()
        .enumerate()
        .map(|(index, width)| {
            let cell = row.get(index);
            let is_image = cell.map(|item| item.is_image).unwrap_or(false);
            let cell_lines = cell.map(|item| item.lines.as_slice()).unwrap_or(&[]);
            let mut all_lines = Vec::new();
            for line in cell_lines {
                if is_image {
                    all_lines.push(line.clone());
                } else {
                    all_lines.extend(wrap_ansi_text(line, *width));
                }
            }
            if all_lines.is_empty() {
                all_lines.push(String::new());
            }
            all_lines
        })
        .collect();
    let row_height = wrapped.iter().map(Vec::len).max().unwrap_or(1);
    let mut output = String::new();
    for line_index in 0..row_height {
        push_vertical(&mut output);
        for (index, width) in widths.iter().enumerate() {
            let cell = row.get(index);
            let is_image = cell.map(|item| item.is_image).unwrap_or(false);
            let line = wrapped
                .get(index)
                .and_then(|lines| lines.get(line_index))
                .map(String::as_str)
                .unwrap_or("");
            let line = if header && !line.is_empty() {
                format!("\x1b[1m{line}\x1b[0m")
            } else {
                line.to_string()
            };
            output.push(' ');
            if is_image {
                push_image_cell_line(
                    &mut output,
                    &line,
                    cell.map(|item| item.width).unwrap_or(0),
                    *width,
                );
            } else {
                let content_width = visible_width(&line);
                output.push_str(&aligned_cell_with_width(
                    &line,
                    content_width,
                    *width,
                    alignments.get(index).copied().unwrap_or(TableAlign::Left),
                ));
            }
            output.push(' ');
            push_vertical(&mut output);
        }
        output.push('\n');
    }
    output
}

/// 写入图片单元格的一行内容。
///
/// 参数:
/// - `output`: 输出缓冲
/// - `line`: 图片协议载荷或字符降级行
/// - `image_width`: 图片声明宽度
/// - `column_width`: 最终列宽
pub(super) fn push_image_cell_line(
    output: &mut String,
    line: &str,
    image_width: usize,
    column_width: usize,
) {
    if line.is_empty() {
        output.push_str(&" ".repeat(column_width));
        return;
    }
    if is_graphics_protocol_line(line) {
        let _ = (image_width, column_width);
        output.push_str(line);
        return;
    }
    let content_width = visible_width(line);
    output.push_str(line);
    output.push_str(&" ".repeat(column_width.saturating_sub(content_width)));
}

/// 判断单行内容是否为终端图形协议载荷。
///
/// 参数:
/// - `line`: 单元格行文本
///
/// 返回:
/// - 是否为 Kitty、iTerm2 或 Sixel 协议
fn is_graphics_protocol_line(line: &str) -> bool {
    line.starts_with("\x1b_G") || line.starts_with("\x1b]1337;") || line.starts_with("\x1bP")
}

/// 渲染表格顶部边框。
///
/// 参数:
/// - `widths`: 每列宽度
///
/// 返回:
/// - 顶部边框
pub(crate) fn top_border(widths: &[usize]) -> String {
    table_border(widths, TOP_LEFT, TOP_MID, TOP_RIGHT)
}

/// 渲染表格行间边框。
///
/// 参数:
/// - `widths`: 每列宽度
///
/// 返回:
/// - 行间边框
pub(crate) fn middle_border(widths: &[usize]) -> String {
    table_border(widths, MID_LEFT, MID_MID, MID_RIGHT)
}

/// 渲染表格底部边框。
///
/// 参数:
/// - `widths`: 每列宽度
///
/// 返回:
/// - 底部边框
pub(crate) fn bottom_border(widths: &[usize]) -> String {
    table_border(widths, BOTTOM_LEFT, BOTTOM_MID, BOTTOM_RIGHT)
}

/// 计算去除终端转义序列后的显示宽度。
///
/// 参数:
/// - `text`: ANSI 文本
///
/// 返回:
/// - 终端列宽
pub(crate) fn visible_width(text: &str) -> usize {
    let mut width = 0;
    let mut visible_segment = String::new();
    let mut index = 0usize;
    while index < text.len() {
        let ch = text[index..].chars().next().unwrap_or_default();
        if ch == '\x1b' {
            width += text_display_width(&visible_segment);
            visible_segment.clear();
            let (_, next_index) = collect_ansi_sequence_at(text, index);
            index = next_index.max(index + ch.len_utf8());
            continue;
        }
        visible_segment.push(ch);
        index += ch.len_utf8();
    }
    width + text_display_width(&visible_segment)
}

/// 将含 ANSI 转义的文本按目标宽度换行。
///
/// 参数:
/// - `text`: 原始 ANSI 文本
/// - `width`: 目标列宽
///
/// 返回:
/// - 保持活动样式的文本行
pub(super) fn wrap_ansi_text(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let width = width.max(1);
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    let mut active_style = String::new();
    let mut index = 0usize;
    while index < text.len() {
        let ch = text[index..].chars().next().unwrap_or_default();
        if ch == '\x1b' {
            let (sequence, next_index) = collect_ansi_sequence_at(text, index);
            update_active_style(&sequence, &mut active_style);
            current.push_str(&sequence);
            index = next_index;
            continue;
        }
        let grapheme = text[index..].graphemes(true).next().unwrap_or("");
        let grapheme_width = text_display_width(grapheme);
        if current_width > 0 && current_width + grapheme_width > width {
            if !active_style.is_empty() {
                current.push_str(RESET);
            }
            lines.push(current);
            current = active_style.clone();
            current_width = 0;
        }
        current.push_str(grapheme);
        current_width += grapheme_width;
        index += grapheme.len();
    }
    lines.push(current);
    lines
}

/// 从指定位置收集完整终端转义序列。
///
/// 参数:
/// - `text`: 原始文本
/// - `start`: ESC 所在字节位置
///
/// 返回:
/// - 完整序列与下一个字节位置
fn collect_ansi_sequence_at(text: &str, start: usize) -> (String, usize) {
    let mut sequence = String::new();
    let mut chars = text[start..].char_indices();
    let Some((_, first)) = chars.next() else {
        return (sequence, start);
    };
    sequence.push(first);
    let mut next_index = start + first.len_utf8();
    let Some((offset, next)) = chars.next() else {
        return (sequence, next_index);
    };
    sequence.push(next);
    next_index = start + offset + next.len_utf8();
    if next == '[' {
        for (offset, ch) in chars.by_ref() {
            sequence.push(ch);
            next_index = start + offset + ch.len_utf8();
            if ('@'..='~').contains(&ch) {
                break;
            }
        }
    } else if next == ']' {
        for (offset, ch) in chars.by_ref() {
            sequence.push(ch);
            next_index = start + offset + ch.len_utf8();
            if ch == '\u{7}' {
                break;
            }
            if ch == '\x1b' {
                continue;
            }
            if sequence.ends_with("\x1b\\") {
                break;
            }
        }
    } else if next != '\\' {
        for (offset, ch) in chars.by_ref() {
            sequence.push(ch);
            next_index = start + offset + ch.len_utf8();
            if sequence.ends_with("\x1b\\") {
                break;
            }
        }
    }
    (sequence, next_index)
}

/// 根据 SGR 序列更新换行后需要恢复的活动样式。
///
/// 参数:
/// - `sequence`: ANSI 转义序列
/// - `active_style`: 活动样式缓冲
fn update_active_style(sequence: &str, active_style: &mut String) {
    if !sequence.starts_with("\x1b[") || !sequence.ends_with('m') {
        return;
    }
    if sequence == RESET || sequence == "\x1b[m" {
        active_style.clear();
    } else {
        active_style.push_str(sequence);
    }
}

/// 计算普通文本的终端显示宽度。
///
/// 参数:
/// - `text`: 不含终端转义序列的文本
///
/// 返回:
/// - Unicode 终端列宽
fn text_display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// 按对齐方式补齐单元格内容。
///
/// 参数:
/// - `cell`: 已渲染内容
/// - `content_width`: 内容显示宽度
/// - `column_width`: 目标列宽
/// - `align`: 对齐方式
///
/// 返回:
/// - 补齐后的单元格文本
fn aligned_cell_with_width(
    cell: &str,
    content_width: usize,
    column_width: usize,
    align: TableAlign,
) -> String {
    let padding = column_width.saturating_sub(content_width);
    match align {
        TableAlign::Left => format!("{cell}{}", " ".repeat(padding)),
        TableAlign::Right => format!("{}{cell}", " ".repeat(padding)),
        TableAlign::Center => {
            let left = padding / 2;
            let right = padding - left;
            format!("{}{cell}{}", " ".repeat(left), " ".repeat(right))
        }
    }
}

/// 渲染通用表格边框。
///
/// 参数:
/// - `widths`: 每列宽度
/// - `left`: 左侧连接符
/// - `mid`: 中间连接符
/// - `right`: 右侧连接符
///
/// 返回:
/// - 带样式的边框行
fn table_border(widths: &[usize], left: char, mid: char, right: char) -> String {
    let mut output = String::new();
    output.push_str(TABLE_BORDER_STYLE);
    output.push(left);
    for (index, width) in widths.iter().enumerate() {
        output.push_str(&HORIZONTAL.to_string().repeat(width + 2));
        output.push(if index + 1 == widths.len() {
            right
        } else {
            mid
        });
    }
    output.push_str(RESET);
    output.push('\n');
    output
}

/// 写入带样式的表格竖线。
///
/// 参数:
/// - `output`: 输出缓冲
fn push_vertical(output: &mut String) {
    output.push_str(TABLE_BORDER_STYLE);
    output.push(VERTICAL);
    output.push_str(RESET);
}

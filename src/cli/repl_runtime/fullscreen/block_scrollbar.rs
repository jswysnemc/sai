//! 输出窗口左侧的双色引导线：灰色轨道表示全文，青色滑块表示可见范围。

use super::sidebar_palette::{fg, Rgb, SidebarPalette};
use crate::render::ansi_style::update_active_sgr;
use crate::render::terminal_image::escape_sequence_end;
use crate::render::transcript::AnsiLine;
use std::ops::Range;

/// 【终端】【块滚动条】按可见正文占比和阅读位置设置已有引导线颜色。
/// 参数: `lines` 为窗口正文，`offset` 为全文内的首行偏移，`total` 为本段正文总行数。
/// 返回: 无，直接更新窗口行的引导线样式。
pub(super) fn decorate(lines: &mut [AnsiLine], offset: usize, total: usize) {
    let thumb = thumb_range(lines.len(), offset, total);
    let palette = SidebarPalette::current();
    for (row, line) in lines.iter_mut().enumerate() {
        let color = if thumb.contains(&row) {
            palette.accent
        } else {
            palette.muted
        };
        *line = tint_gutter(line, color);
    }
}

/// 【终端】【输出滚动条】为实际输出行复用缩进列绘制引导线，固定标题不计入轨道。
/// 参数: `lines` 为可见文档，`rows` 为输出行号和固定引导列，`offset` 为内部偏移，`max` 为最大偏移；返回: 无。
pub(super) fn decorate_output(
    lines: &mut [AnsiLine],
    rows: &[(usize, usize)],
    offset: usize,
    max: usize,
) {
    let thumb = thumb_range(rows.len(), offset, rows.len().saturating_add(max));
    let palette = SidebarPalette::current();
    for (index, (row, column)) in rows.iter().copied().enumerate() {
        let color = if thumb.contains(&index) {
            palette.accent
        } else {
            palette.muted
        };
        lines[row] = tint_gutter(&output_gutter(&lines[row], column), color);
    }
}

/// 【终端】【输出引导列】复用正文缩进中的一列，保留代码自身缩进、终端宽度及图片位置。
/// 参数: `line` 为已有排版和复制信息的输出行，`column` 为节点固定引导列；返回: 带竖线的等宽输出行。
fn output_gutter(line: &AnsiLine, column: usize) -> AnsiLine {
    let text = line.as_str();
    let mut index = 0;
    let mut spaces = Vec::new();
    while index < text.len() {
        if text.as_bytes()[index] == b'\x1b' {
            index = escape_sequence_end(text, index).max(index + 1);
            continue;
        }
        let ch = text[index..].chars().next().unwrap_or_default();
        if ch != ' ' {
            // 1. 【终端】【引导线归一】旧命令前缀中的折角或竖线让位给固定引导列，不触碰原始正文
            if matches!(ch, '│' | '└')
                && spaces.len() != column
                && spaces.len() < line.copy.content_start
            {
                let cleared = line.with_text(
                    format!("{} {}", &text[..index], &text[index + ch.len_utf8()..]),
                    0,
                );
                return output_gutter(&cleared, column);
            }
            if matches!(ch, '│' | '└') && spaces.len() == column {
                return if ch == '│' {
                    line.clone()
                } else {
                    line.with_text(
                        format!("{}│{}", &text[..index], &text[index + ch.len_utf8()..]),
                        0,
                    )
                };
            }
            break;
        }
        spaces.push(index);
        index += 1;
    }
    let Some(&index) = spaces.get(column).filter(|_| spaces.len() >= column + 2) else {
        return line.clone();
    };
    let mut result = line.with_text(format!("{}│{}", &text[..index], &text[index + 1..]), 0);
    result.copy.content_start = result.copy.content_start.max(column + 2);
    result
}

/// 【终端】【输出占比】将正文可见比例映射为滑块长度，滚动位置映射为滑块起点。
/// 参数: `visible` 为窗口行数，`offset` 为正文偏移，`total` 为正文总行数。
/// 返回: 滑块行范围；可滚动时至少占一行，全文可见时填满引导线。
fn thumb_range(visible: usize, offset: usize, total: usize) -> Range<usize> {
    if visible == 0 || total <= visible {
        return 0..visible;
    }
    // 1. 【终端】【输出占比】以窗口高度压缩全文，保留至少一行可见滑块
    let size = (visible.saturating_mul(visible) / total).max(1);
    let travel = visible - size;
    let max = total - visible;
    // 2. 【终端】【输出位置】按有效滚动距离映射，首尾阅读位置贴合轨道边界
    let top = offset.min(max).saturating_mul(travel) / max;
    top..top + size
}

/// 【终端】【引导线着色】只修改首个非空白字符为竖线的行，并恢复原有正文样式。
/// 参数: `line` 为原始 ANSI 正文行，`color` 为引导线前景色。
/// 返回: 保留宽度、正文颜色和图片元数据的终端行。
fn tint_gutter(line: &AnsiLine, color: Rgb) -> AnsiLine {
    let text = line.as_str();
    let mut index = 0;
    let mut active_sgr = String::new();
    // 1. 【终端】【引导线定位】跳过缩进和控制序列，记录正文原有样式
    while index < text.len() {
        if text.as_bytes()[index] == b'\x1b' {
            let end = escape_sequence_end(text, index).max(index + 1);
            let sequence = &text[index..end];
            if sequence.starts_with("\x1b[") && sequence.ends_with('m') {
                update_active_sgr(&mut active_sgr, sequence);
            }
            index = end;
            continue;
        }
        let ch = text[index..].chars().next().unwrap_or_default();
        if ch == ' ' {
            index += ch.len_utf8();
            continue;
        }
        if ch != '│' {
            break;
        }
        // 2. 【终端】【引导线着色】取消竖线暗淡效果，随后恢复正文已有样式
        let text = format!(
            "{}\x1b[22m{}│\x1b[0m{active_sgr}{}",
            &text[..index],
            fg(color),
            &text[index + ch.len_utf8()..],
        );
        return line.with_text(text, 0);
    }
    line.clone()
}

#[cfg(test)]
#[path = "block_scrollbar_tests.rs"]
mod tests;

use super::{visible_width, CellContent};
use unicode_segmentation::UnicodeSegmentation;

/// 【表格】【宽度预算】获取正文净宽，避免重复扣除缩进或扩大窄窗口
/// 返回: 当前渲染上下文或 CLI 正文的可用列数
pub(super) fn available_width() -> usize {
    crate::render::render_width::render_width_override()
        .unwrap_or_else(crate::render::content_indent::cli_content_width)
        .max(1)
}

/// 【表格】【列宽分配】按自然宽度、最长单词与字素下限分配列宽
/// 参数: rows 为已渲染的单元格
/// 返回: 适配正文的列宽；空列表表示需要窄屏降级
pub(crate) fn compute_table_widths(rows: &[Vec<CellContent>]) -> Vec<usize> {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if cols == 0 {
        return Vec::new();
    }
    let overhead = cols.saturating_mul(3).saturating_add(1);
    let budget = available_width().saturating_sub(overhead);
    let mut natural = vec![1; cols];
    let mut minimum = vec![1; cols];
    let mut preferred = vec![1; cols];
    // 1. 【表格】【列宽分配】自然宽度避免把短表撑满；中文字素下限防止单列只有一格却写入两格
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            natural[index] = natural[index].max(cell.width);
            for line in &cell.lines {
                if super::renderer::is_graphics_protocol_line(line) {
                    continue;
                }
                let plain = plain_text(line);
                let glyph = plain.graphemes(true).map(visible_width).max().unwrap_or(1);
                minimum[index] = minimum[index].max(glyph);
                let word = plain
                    .split_whitespace()
                    .map(visible_width)
                    .max()
                    .unwrap_or(1);
                preferred[index] = preferred[index].max(word.min(30)).max(glyph);
            }
            // 图片先争取可读宽度，最终仍由原有公式管线按实际列宽重新渲染
            if cell.is_image {
                preferred[index] = preferred[index].max(cell.width.min(30));
            }
        }
    }
    if minimum.iter().sum::<usize>() > budget {
        return Vec::new();
    }
    // 2. 【表格】【列宽分配】先满足单词宽度，再按自然宽度分配空间，采用 MiniMax 的比例分配思路
    let widths = distribute(&minimum, &preferred, budget);
    distribute(&widths, &natural, budget)
}

/// 【表格】【列宽分配】在预算内按各列增长需求分配剩余空间
/// 参数: base 为下限，target 为期望，budget 为总预算
/// 返回: 不超过预算且不小于下限的列宽
fn distribute(base: &[usize], target: &[usize], budget: usize) -> Vec<usize> {
    let mut widths = base.to_vec();
    let remaining = budget.saturating_sub(base.iter().sum());
    let growth: Vec<usize> = target
        .iter()
        .zip(base)
        .map(|(goal, start)| goal.saturating_sub(*start))
        .collect();
    let total: usize = growth.iter().sum();
    if total == 0 {
        return widths;
    }
    let extra = remaining.min(total);
    for (index, weight) in growth.iter().enumerate() {
        widths[index] += ((*weight as u128 * extra as u128) / total as u128) as usize;
    }
    let mut leftover = extra
        - widths
            .iter()
            .zip(base)
            .map(|(next, start)| next - start)
            .sum::<usize>();
    for (index, goal) in target.iter().enumerate() {
        if leftover == 0 {
            break;
        }
        if widths[index] < *goal {
            widths[index] += 1;
            leftover -= 1;
        }
    }
    widths
}

/// 【表格】【宽度测量】移除 ANSI 序列后分析文本字素和单词
/// 参数: text 为单元格行；返回: 普通可见文本
fn plain_text(text: &str) -> String {
    let mut output = String::new();
    let mut index = 0;
    while index < text.len() {
        if text[index..].starts_with('\x1b') {
            index = crate::render::terminal_image::escape_sequence_end(text, index).max(index + 1);
        } else {
            let ch = text[index..].chars().next().unwrap();
            output.push(ch);
            index += ch.len_utf8();
        }
    }
    output
}

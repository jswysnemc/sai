use super::CellContent;
use crossterm::terminal;

/// 根据列数量返回可读的最小列宽。
///
/// 参数:
/// - `cols`: 表格列数量
///
/// 返回:
/// - 每列最小宽度
pub(crate) fn readable_table_min_width(cols: usize) -> usize {
    match cols {
        0 => 0,
        1 => 16,
        2 => 14,
        3 | 4 => 10,
        _ => 8,
    }
}

/// 根据全部单元格内容计算最终列宽。
///
/// 参数:
/// - `rows`: 已渲染的表格行
///
/// 返回:
/// - 适配当前终端宽度后的列宽
pub(crate) fn compute_table_widths(rows: &[Vec<CellContent>]) -> Vec<usize> {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; cols];
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.width);
        }
    }
    let min_width = readable_table_min_width(cols);
    for width in &mut widths {
        *width = (*width).max(min_width);
    }
    bounded_table_widths(rows, widths)
}

/// 将表格列宽限制在当前终端范围内。
///
/// 参数:
/// - `rows`: 已渲染的表格行
/// - `widths`: 初始列宽
///
/// 返回:
/// - 压缩后的列宽
fn bounded_table_widths(rows: &[Vec<CellContent>], mut widths: Vec<usize>) -> Vec<usize> {
    if widths.is_empty() {
        return widths;
    }
    let terminal_width = terminal::size()
        .map(|(width, _)| usize::from(width))
        .unwrap_or(100)
        .saturating_sub(1)
        .max(20);
    let border_overhead = widths.len().saturating_mul(3).saturating_add(1);
    let available = terminal_width
        .saturating_sub(border_overhead)
        .max(widths.len());
    let image_mins = (0..widths.len())
        .map(|index| {
            rows.iter()
                .filter_map(|row| row.get(index))
                .filter(|cell| cell.is_image)
                .map(|cell| cell.width.max(1))
                .max()
                .unwrap_or(1)
        })
        .collect::<Vec<_>>();
    while widths.iter().sum::<usize>() > available {
        // 1. 优先压缩高于图片最小宽度的列
        let Some((index, width)) = widths
            .iter()
            .enumerate()
            .filter(|(index, width)| **width > image_mins[*index])
            .max_by_key(|(_, width)| **width)
            .map(|(index, width)| (index, *width))
        else {
            // 2. 图片下限仍无法容纳时，继续压缩当前最宽列
            let Some((index, width)) = widths
                .iter()
                .enumerate()
                .max_by_key(|(_, width)| **width)
                .map(|(index, width)| (index, *width))
            else {
                break;
            };
            if width <= 1 {
                break;
            }
            widths[index] -= 1;
            continue;
        };
        if width <= 1 {
            break;
        }
        widths[index] -= 1;
    }
    widths
}

mod fallback;
mod layout;
pub(crate) mod live_layout;
mod model;
mod parser;
mod renderer;
pub(crate) mod streaming;

#[cfg(test)]
mod regression_tests;
#[cfg(test)]
mod tests;

pub(crate) use layout::compute_table_widths;
pub(crate) use model::{CellContent, TableAlign};
pub(crate) use parser::{is_table_separator, looks_like_table_row, parse_table_row};
pub(crate) use renderer::{
    bottom_border, middle_border, render_table_row, top_border, visible_width,
};

#[cfg(test)]
use parser::split_table_cells;
#[cfg(test)]
use renderer::{push_image_cell_line, wrap_ansi_text};

/// 渲染完整 Markdown 表格。
///
/// 参数:
/// - `lines`: 表格原始行
/// - `render_cell`: 单元格内容渲染函数
///
/// 返回:
/// - 带细实线边框的终端表格文本
#[cfg(test)]
pub(crate) fn render_table<F>(lines: &[String], render_cell: F) -> String
where
    F: Fn(&str) -> CellContent + Copy,
{
    render_table_with_widths(lines, render_cell, None).0
}

/// 【表格】【稳定布局】使用已固定的列宽渲染，公式图片仍按最终列宽适配
/// 参数: lines 为源码行，render_cell 为单元格渲染器，fixed 为可选固定列宽
/// 返回: 表格文本与实际采用的列宽，空列宽表示纵向降级
fn render_table_with_widths<F>(
    lines: &[String],
    render_cell: F,
    fixed: Option<&[usize]>,
) -> (String, Vec<usize>)
where
    F: Fn(&str) -> CellContent + Copy,
{
    let rows = lines
        .iter()
        .enumerate()
        .filter(|(index, line)| *index != 1 || !is_table_separator(line))
        .map(|(_, line)| parse_table_row(line, render_cell))
        .collect::<Vec<_>>();
    let widths = fixed
        .map(<[usize]>::to_vec)
        .unwrap_or_else(|| compute_table_widths(&rows));
    if widths.is_empty() {
        return (
            fallback::render_stacked(&rows, layout::available_width()),
            widths,
        );
    }
    let mut rows = rows;
    refit_math_image_cells(&mut rows, &widths);

    let mut output = String::new();
    output.push_str(&top_border(&widths));
    for (row_index, row) in rows.iter().enumerate() {
        output.push_str(&render_table_row(row, &widths, row_index == 0));
        if row_index + 1 < rows.len() {
            output.push_str(&middle_border(&widths));
        }
    }
    output.push_str(&bottom_border(&widths));
    (output, widths)
}

/// 按最终列宽重新渲染过宽的公式图片单元格。
///
/// 参数:
/// - `rows`: 表格行
/// - `widths`: 最终列宽
fn refit_math_image_cells(rows: &mut [Vec<CellContent>], widths: &[usize]) {
    for row in rows.iter_mut() {
        for (index, cell) in row.iter_mut().enumerate() {
            let Some(target_width) = widths.get(index).copied() else {
                continue;
            };
            let Some(encoded) = cell.math_source.clone() else {
                continue;
            };
            if !cell.is_image || cell.width <= target_width {
                continue;
            }
            // 1. 列宽收缩后重新生成图片协议尺寸
            let (source, mixed) = crate::render::asset_block::decode_table_math_source(&encoded);
            let mut refitted = crate::render::asset_block::render_inline_math_table_cell(
                &source,
                target_width,
                mixed,
            );
            refitted.math_source = Some(encoded);
            *cell = refitted;
        }
    }
}

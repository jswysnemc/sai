use super::renderer::{is_graphics_protocol_line, wrap_ansi_text};
use super::{refit_math_image_cells, CellContent};

/// 【表格】【窄屏降级】边框与字素无法同时容纳时按字段纵向展示
/// 参数: rows 为完整表格，width 为正文净宽
/// 返回: 表头与内容逐字段对应的文本，保留公式图片
pub(super) fn render_stacked(rows: &[Vec<CellContent>], width: usize) -> String {
    let Some(header) = rows.first() else {
        return String::new();
    };
    let mut output = String::new();
    if rows.len() == 1 {
        for cell in header {
            push_cell(&mut output, cell, width);
        }
        return output;
    }
    for (row_index, row) in rows.iter().enumerate().skip(1) {
        if row_index > 1 {
            output.push('\n');
        }
        for index in 0..header.len().max(row.len()) {
            if let Some(label) = header.get(index) {
                push_cell(&mut output, label, width);
            }
            if let Some(cell) = row.get(index) {
                push_cell(&mut output, cell, width);
            }
        }
    }
    output
}

/// 【表格】【窄屏降级】按净宽重排单元格，图片继续走原有尺寸适配
/// 参数: output 为输出缓冲，cell 为单元格，width 为净宽
/// 返回: 无，追加完整内容
fn push_cell(output: &mut String, cell: &CellContent, width: usize) {
    let mut rows = vec![vec![cell.clone()]];
    refit_math_image_cells(&mut rows, &[width]);
    for line in &rows[0][0].lines {
        if is_graphics_protocol_line(line) {
            output.push_str(line);
            output.push('\n');
        } else {
            for wrapped in wrap_ansi_text(line, width) {
                output.push_str(&wrapped);
                output.push('\n');
            }
        }
    }
}

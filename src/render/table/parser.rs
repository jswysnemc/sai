use super::{CellContent, TableAlign};

/// 判断一行是否为 Markdown 表格分隔行。
///
/// 参数:
/// - `line`: Markdown 原始行
///
/// 返回:
/// - 是否为表格分隔行
pub(crate) fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim().trim_matches('|').trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .all(|ch| matches!(ch, '-' | ':' | '|' | ' '))
        && trimmed.contains('-')
}

/// 判断一行是否可能属于 Markdown 表格。
///
/// 参数:
/// - `line`: Markdown 原始行
///
/// 返回:
/// - 是否具有完整表格行边界
pub(crate) fn looks_like_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.matches('|').count() >= 2
}

/// 解析表格数据行。
///
/// 参数:
/// - `line`: 原始表格行
/// - `render_cell`: 单元格内容渲染函数
///
/// 返回:
/// - 已渲染的单元格列表
pub(crate) fn parse_table_row<F>(line: &str, render_cell: F) -> Vec<CellContent>
where
    F: Fn(&str) -> CellContent,
{
    split_table_cells(line)
        .into_iter()
        .map(|cell| render_cell(&cell))
        .collect()
}

/// 解析表格对齐标记。
///
/// 参数:
/// - `line`: Markdown 分隔行
///
/// 返回:
/// - 每列对齐方式
pub(crate) fn parse_table_alignments(line: &str) -> Vec<TableAlign> {
    split_table_cells(line)
        .into_iter()
        .map(|cell| {
            let cell = cell.trim();
            match (cell.starts_with(':'), cell.ends_with(':')) {
                (true, true) => TableAlign::Center,
                (false, true) => TableAlign::Right,
                _ => TableAlign::Left,
            }
        })
        .collect()
}

/// 按表格分隔符拆分单元格，忽略公式、代码与转义内容中的竖线。
///
/// 参数:
/// - `line`: 原始表格行
///
/// 返回:
/// - 去除首尾空白后的单元格文本
pub(crate) fn split_table_cells(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = if line.ends_with('|') && !line.ends_with("\\|") {
        &line[..line.len().saturating_sub(1)]
    } else {
        line
    };
    let chars = line.chars().collect::<Vec<_>>();
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut index = 0usize;
    let mut escaped = false;
    let mut in_inline_math = false;
    let mut in_display_math = false;
    let mut in_code = false;
    while index < chars.len() {
        let ch = chars[index];
        // 1. 反斜杠转义后的字符不参与分隔判断
        if escaped {
            current.push(ch);
            escaped = false;
            index += 1;
            continue;
        }
        if ch == '\\' {
            current.push(ch);
            escaped = true;
            index += 1;
            continue;
        }
        // 2. 显示公式内部的竖线属于公式内容
        if ch == '$' && chars.get(index + 1) == Some(&'$') {
            in_display_math = !in_display_math;
            if !in_display_math {
                in_inline_math = false;
            }
            current.push('$');
            current.push('$');
            index += 2;
            continue;
        }
        // 3. 行内公式内部的竖线属于公式内容
        if ch == '$' && !in_display_math {
            in_inline_math = !in_inline_math;
            current.push(ch);
            index += 1;
            continue;
        }
        // 4. 行内代码内部的竖线属于代码内容
        if ch == '`' && !in_inline_math && !in_display_math {
            in_code = !in_code;
            current.push(ch);
            index += 1;
            continue;
        }
        // 5. 仅在公式和代码之外拆分列
        if ch == '|' && !in_inline_math && !in_display_math && !in_code {
            cells.push(current.trim().to_string());
            current.clear();
            index += 1;
            continue;
        }
        current.push(ch);
        index += 1;
    }
    cells.push(current.trim().to_string());
    cells
}

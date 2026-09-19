use super::CellContent;

/// 判断一行是否为 Markdown 表格分隔行。
///
/// 参数:
/// - `line`: Markdown 原始行
///
/// 返回:
/// - 是否为表格分隔行
pub(crate) fn is_table_separator(line: &str) -> bool {
    let cells = split_table_cells(line);
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let rule = cell.trim().trim_start_matches(':').trim_end_matches(':');
            !rule.is_empty() && rule.chars().all(|ch| ch == '-')
        })
}

/// 判断一行是否可能属于 Markdown 表格。
///
/// 参数:
/// - `line`: Markdown 原始行
///
/// 返回:
/// - 是否含有效列分隔，兼容省略首尾竖线的 Markdown 表格
pub(crate) fn looks_like_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    (trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() > 1)
        || (trimmed.contains('|') && split_table_cells(trimmed).len() > 1)
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
    let mut code_ticks = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        // 1. 【表格】【单元格解析】反斜杠转义后的字符不参与分隔判断
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
        // 2. 【表格】【单元格解析】代码优先识别，代码里的美元符号不能改变公式状态
        if ch == '`' && !in_inline_math && !in_display_math {
            let count = chars[index..].iter().take_while(|ch| **ch == '`').count();
            if code_ticks == count {
                code_ticks = 0;
            } else if code_ticks == 0 {
                code_ticks = count;
            }
            current.extend(std::iter::repeat_n('`', count));
            index += count;
            continue;
        }
        // 3. 【表格】【单元格解析】显示公式内部的竖线属于公式内容
        if ch == '$' && code_ticks == 0 && chars.get(index + 1) == Some(&'$') {
            in_display_math = !in_display_math;
            if !in_display_math {
                in_inline_math = false;
            }
            current.push('$');
            current.push('$');
            index += 2;
            continue;
        }
        // 4. 【表格】【单元格解析】行内公式内部的竖线属于公式内容
        if ch == '$' && code_ticks == 0 && !in_display_math {
            in_inline_math = !in_inline_math;
            current.push(ch);
            index += 1;
            continue;
        }
        // 5. 【表格】【单元格解析】仅在公式和代码之外拆分列
        if ch == '|' && !in_inline_math && !in_display_math && code_ticks == 0 {
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

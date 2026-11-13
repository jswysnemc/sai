//! 工具正文的等宽引导列，内联与全屏共用，不移动源码、行号或图片位置。
use super::AnsiLine;
use crate::render::terminal_image::escape_sequence_end;

/// 【终端】【工具引导】参数为节点物理行，返回无值，标题保留原样，正文复用第一列缩进
pub(super) fn decorate_node(lines: &mut [AnsiLine]) {
    let start = lines
        .iter()
        .position(|line| !super::spacing::is_visual_blank(line))
        .map_or(lines.len(), |row| row + 1);
    for line in lines.iter_mut().skip(start) {
        *line = output_line(line, 0);
    }
}

/// 【终端】【输出引导列】复用正文缩进中的一列，保留代码自身缩进、终端宽度及图片位置。
/// 参数: `line` 为已有排版和复制信息的输出行，`column` 为节点固定引导列；返回: 带竖线的等宽输出行。
pub(crate) fn output_line(line: &AnsiLine, column: usize) -> AnsiLine {
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
                return output_line(&cleared, column);
            }
            if matches!(ch, '│' | '└') && spaces.len() == column {
                let mut result = if ch == '│' {
                    line.clone()
                } else {
                    line.with_text(
                        format!("{}│{}", &text[..index], &text[index + ch.len_utf8()..]),
                        0,
                    )
                };
                result.copy.content_start = result.copy.content_start.max(column + 2);
                return result;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【正文符号回归】无参数和返回值，公共引导列不能删除未标记正文中的原始符号
    #[test]
    fn tool_guide_preserves_untagged_content_symbols() {
        for text in ["    │ original", "    └ original", "    progress"] {
            let source = AnsiLine::new(text.into());
            let guided = output_line(&source, 0);
            let expected = format!("│{}", &text[1..]);
            assert_eq!(guided.as_str(), expected);
            assert_eq!(output_line(&guided, 0), guided);
        }
    }

    #[test]
    fn tool_guide_preserves_number_and_source_columns() {
        let source = AnsiLine::new("  120 -          source code".into());
        let guided = output_line(&source, 0);
        assert_eq!(
            crate::render::ansi_clip::ansi_visible_width(source.as_str()),
            crate::render::ansi_clip::ansi_visible_width(guided.as_str())
        );
        assert!(guided.as_str().starts_with("│ 120 -"));
        assert_eq!(
            source.as_str().find("source"),
            guided.as_str().chars().position(|ch| ch == 's')
        );
    }
}

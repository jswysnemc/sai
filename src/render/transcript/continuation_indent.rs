//! 公共折行前缀识别：正文列与引导线位置由原始行决定。

/// 【终端】【正文列识别】保留读取行号、补丁标记、字段标签和源码缩进。
/// 参数：不含 ANSI 的原始行；返回：续行正文起始列。
pub(super) fn column(line: &str) -> usize {
    let leading = line.len() - line.trim_start_matches([' ', '\t']).len();
    let mut start = leading;
    // 1. 【终端】【引导前缀】引导线属于布局，不把它当作源码正文
    while line[start..].starts_with('│') {
        start += '│'.len_utf8();
        start += line[start..].len() - line[start..].trim_start_matches([' ', '\t']).len();
    }
    let digits = line[start..].bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 {
        let end = start + digits;
        let rest = &line[end..];
        if let Some(body) = rest
            .strip_prefix(' ')
            .and_then(|rest| rest.strip_prefix('+').or_else(|| rest.strip_prefix('-')))
        {
            if body.starts_with("  ") {
                return whitespace_end_column(line, end + 2);
            }
        }
        if rest.starts_with("  ") || rest.starts_with('\t') {
            return whitespace_end_column(line, end);
        }
        if rest.starts_with(": ") {
            return whitespace_end_column(line, end + 2);
        }
    }
    // 2. 【终端】【字段前缀】路径与工具字段按值的起始列续行
    for label in [
        "File: ",
        "Path: ",
        "Command: ",
        "Directory: ",
        "文件：",
        "路径：",
        "命令：",
    ] {
        if line[start..].starts_with(label) {
            return whitespace_end_column(line, start + label.len());
        }
    }
    for marker in ["• ", "- ", "* "] {
        if line[start..].starts_with(marker) {
            return whitespace_end_column(line, start + marker.len());
        }
    }
    whitespace_end_column(line, start)
}

/// 【终端】【前缀列宽】制表符采用四列制表位，与 ANSI 折行器口径一致。
/// 参数：原始行、已经识别的前缀末端；返回：前缀和连续空白的总列宽。
fn whitespace_end_column(line: &str, start: usize) -> usize {
    let end = start + line[start..].len() - line[start..].trim_start_matches([' ', '\t']).len();
    line[..end].chars().fold(0, |column, character| {
        if character == '\t' {
            column + 4 - column % 4
        } else {
            column + unicode_width::UnicodeWidthChar::width(character).unwrap_or(0)
        }
    })
}

/// 【终端】【引导线定位】只复制正文之前的参考线，不复制行号或字段标签。
/// 参数：不含 ANSI 的原始行、正文起始列；返回：参考线所在的显示列。
pub(super) fn guide_columns(line: &str, body_column: usize) -> Vec<usize> {
    let mut column = 0;
    let mut guides = Vec::new();
    for character in line.chars() {
        if column >= body_column {
            break;
        }
        if character == '│' {
            guides.push(column);
        }
        column += if character == '\t' {
            4 - column % 4
        } else {
            unicode_width::UnicodeWidthChar::width(character).unwrap_or(0)
        };
    }
    guides
}

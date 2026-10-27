use crate::render::transcript::line::AnsiLine;

/// 【终端】【对齐折行】保留行号、补丁标记与代码自身缩进，续行从正文起始列展示。
/// 参数: `text` 为 ANSI 文本，`width` 为列宽；返回: 已对齐且保留颜色的物理行。
pub(super) fn wrap_aligned_block(text: &str, width: usize) -> Vec<AnsiLine> {
    let width = width.max(1);
    text.split('\n')
        .flat_map(|line| {
            let plain = crate::render::ansi_clip::strip_ansi(line);
            let indent = continuation_column(&plain).min(width.saturating_sub(4));
            AnsiLine::wrap_block_with_continuation_indent(line, width, indent)
        })
        .collect()
}

/// 【终端】【正文列识别】区分补丁标记和读取正文，避免把双空格读取行当作补丁上下文。
/// 参数: `line` 为可见文本；返回: 包含原始代码缩进的续行起始列。
fn continuation_column(line: &str) -> usize {
    // 1. 【终端】【行号前缀】行号为 ASCII 数字，读取与补丁上下文均保留全部正文前导空白
    let leading = line.len() - line.trim_start_matches(' ').len();
    let digits = line[leading..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count();
    if digits > 0 {
        let end = leading + digits;
        let rest = &line[end..];
        let marker = rest
            .strip_prefix(' ')
            .and_then(|rest| rest.strip_prefix('+').or_else(|| rest.strip_prefix('-')));
        if marker.is_some_and(|body| body.starts_with("  ")) {
            return whitespace_end_column(line, end + 2);
        }
        if rest.starts_with("  ") {
            return whitespace_end_column(line, end);
        }
    }
    // 2. 【终端】【列表前缀】保留列表标记和正文之间的缩进，其他行沿用源码前导空白
    let rest = &line[leading..];
    for marker in ["• ", "- ", "* "] {
        if rest.starts_with(marker) {
            return whitespace_end_column(line, leading + marker.len());
        }
    }
    whitespace_end_column(line, 0)
}

/// 【终端】【缩进列宽】按与 ANSI 折行器相同的四列制表位计算前缀及后续空白宽度。
/// 参数: `line` 为文本，`start` 为前缀末尾字节位置；返回: 正文首字符所在列。
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

#[cfg(test)]
#[path = "aligned_wrap_tests.rs"]
mod tests;

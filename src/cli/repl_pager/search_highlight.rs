use super::search_index::SourceSpan;
use crate::render::terminal_image::escape_sequence_end;

/// 【终端】【搜索高亮】仅装饰命中范围，保留原始颜色及图片协议
/// 参数: line 为 ANSI 行，row 为行号，hits 为命中范围，selected 为当前命中
/// 返回: 插入高亮样式后的完整行
pub(super) fn highlight(
    line: &str,
    row: usize,
    hits: &[Vec<SourceSpan>],
    selected: usize,
) -> String {
    let spans: Vec<_> = hits
        .iter()
        .enumerate()
        .flat_map(|(index, spans)| {
            spans
                .iter()
                .filter(move |span| span.row == row)
                .map(move |span| (index, span))
        })
        .collect();
    if spans.is_empty() {
        return line.to_string();
    }
    let mut output = String::new();
    let mut original_style = String::new();
    let mut active = None;
    let mut offset = 0;
    while offset < line.len() {
        if line.as_bytes()[offset] == 0x1b {
            let end = escape_sequence_end(line, offset);
            let sequence = &line[offset..end];
            output.push_str(sequence);
            if sequence.starts_with("\x1b[") && sequence.ends_with('m') {
                original_style.push_str(sequence);
                output.push_str(style(active, selected));
            }
            offset = end;
            continue;
        }
        let next = spans
            .iter()
            .find(|(_, span)| span.start <= offset && offset < span.end)
            .map(|(index, _)| *index);
        if next != active {
            if active.is_some() {
                output.push_str("\x1b[0m");
                output.push_str(&original_style);
            }
            active = next;
            output.push_str(style(active, selected));
        }
        let ch = line[offset..].chars().next().expect("character boundary");
        output.push(ch);
        offset += ch.len_utf8();
    }
    if active.is_some() {
        output.push_str("\x1b[0m");
        output.push_str(&original_style);
    }
    output
}

/// 返回命中样式；index 为可选命中序号，selected 为选中序号
fn style(index: Option<usize>, selected: usize) -> &'static str {
    match index {
        Some(index) if index == selected => "\x1b[1m\x1b[7m",
        Some(_) => "\x1b[4m",
        None => "",
    }
}

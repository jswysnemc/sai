use super::line::AnsiLine;
use crate::render::style::{MD_LIST_MARKER_STYLE, RESET};

/// 【终端】【列表换行】依据列表正文起始列恢复自动续行缩进，其他内容沿用原有缩进。
/// 参数: `rendered` 为 Markdown 渲染结果，`width` 为净宽，`default_indent` 为非列表续行缩进；返回: 物理行。
pub(in crate::render) fn wrap(
    rendered: &str,
    width: usize,
    default_indent: usize,
) -> Vec<AnsiLine> {
    rendered
        .split('\n')
        .flat_map(|line| {
            let Some(indent) = content_column(line) else {
                return AnsiLine::wrap_block_with_continuation_indent(line, width, default_indent);
            };
            // 1. 【终端】【列表换行】断点处的单个词间空格不进入续行，续行严格对齐正文列
            AnsiLine::wrap_list_item(line, width, indent)
        })
        .collect()
}

/// 【终端】【列表正文列】只识别 Markdown 结构标记，避免把代码中的圆点或数字当成列表。
/// 参数: `line` 为渲染后的逻辑行；返回: 列表正文起始列，非列表为空。
pub(in crate::render) fn content_column(line: &str) -> Option<usize> {
    let trimmed = line.trim_start_matches(' ');
    let styled = trimmed.strip_prefix(MD_LIST_MARKER_STYLE)?;
    let (marker, content) = styled.split_once(RESET)?;
    if marker.is_empty() {
        return Some(line.len() - trimmed.len());
    }
    if !content.starts_with(' ') {
        return None;
    }
    let ordered = marker
        .strip_suffix('.')
        .or_else(|| marker.strip_suffix(')'))
        .is_some_and(|number| {
            !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
        });
    if !matches!(marker, "•" | "-" | "·" | "–") && !ordered {
        return None;
    }
    Some(line.len() - trimmed.len() + unicode_width::UnicodeWidthStr::width(marker) + 1)
}

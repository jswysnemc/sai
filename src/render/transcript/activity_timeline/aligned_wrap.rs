use crate::render::transcript::line::AnsiLine;

/// 【终端】【对齐折行】保留行号、补丁标记与代码自身缩进，续行从正文起始列展示。
/// 参数: `text` 为 ANSI 文本，`width` 为列宽；返回: 已对齐且保留颜色的物理行。
pub(super) fn wrap_aligned_block(text: &str, width: usize) -> Vec<AnsiLine> {
    let width = width.max(1);
    // 1. 【终端图片】【占位连续性】图片必须整块排版，使首行来源覆盖后续占位行
    if crate::render::image_reference::has_marker(text) {
        return AnsiLine::wrap_block(text, width);
    }
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
    super::super::continuation_indent::column(line)
}

#[cfg(test)]
#[path = "aligned_wrap_tests.rs"]
mod tests;

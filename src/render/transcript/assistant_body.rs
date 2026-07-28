use super::line::AnsiLine;
use crate::render::content_indent::CONTENT_LEFT_INDENT;
use crate::render::style::{RESET, SECONDARY_STYLE};

const ASSISTANT_BODY_GUIDE_SYMBOL: &str = "•";
const DIM_STYLE: &str = "\x1b[2m";

/// 【终端】【正文引导】为已渲染的助手正文添加视觉引导区。
///
/// 参数:
/// - `rendered`: 已完成 Markdown 与 ANSI 样式渲染的正文
/// - `content_width`: 调用方已经扣除视觉引导区后的正文列数
///
/// 返回:
/// - 首行带引导符号、续行保持等宽缩进的预换行终端行
pub(super) fn display_lines(rendered: &str, content_width: usize) -> Vec<AnsiLine> {
    if rendered.is_empty() {
        return Vec::new();
    }

    // 【终端】【正文引导】1. 按调用方传入的正文净宽度完成折行
    let lines = AnsiLine::wrap_block(rendered, content_width);
    // 【终端】【正文引导】2. 首行绘制符号，后续物理行保留等宽空白引导区
    lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let prefix = if index == 0 {
                format!("{DIM_STYLE}{SECONDARY_STYLE}{ASSISTANT_BODY_GUIDE_SYMBOL}{RESET} ")
            } else {
                " ".repeat(CONTENT_LEFT_INDENT)
            };
            AnsiLine::new(format!("{prefix}{}", line.as_str()))
        })
        .collect()
}

use super::line::AnsiLine;
use crate::render::content_indent::CONTENT_LEFT_INDENT;

/// 【终端】【正文引导】为已渲染的助手正文添加视觉引导区。
///
/// 正文不再使用与工具行相同的 `•` 引导点——工具与思考各自有符号，
/// 助手回答只保留左侧等宽缩进，作为主内容层，扫读时不再和工具混成一串圆点。
///
/// 参数:
/// - `rendered`: 已完成 Markdown 与 ANSI 样式渲染的正文
/// - `content_width`: 调用方已经扣除视觉引导区后的正文列数
///
/// 返回:
/// - 全部物理行带等宽左侧缩进的预换行终端行
pub(super) fn display_lines(rendered: &str, content_width: usize) -> Vec<AnsiLine> {
    if rendered.is_empty() {
        return Vec::new();
    }

    // 1. 按调用方传入的正文净宽度完成折行
    let lines = AnsiLine::wrap_block(rendered, content_width);
    // 2. 每一行补上与引导区同宽的空白，落在工具/思考符号右侧的内容列
    let prefix = " ".repeat(CONTENT_LEFT_INDENT);
    lines
        .into_iter()
        .map(|line| AnsiLine::new(format!("{prefix}{}", line.as_str())))
        .collect()
}

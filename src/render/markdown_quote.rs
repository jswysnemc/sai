use super::content_indent::CONTENT_LEFT_INDENT;
use super::style::{MD_QUOTE_BAR_STYLE, RESET};
use super::transcript::AnsiLine;
use unicode_width::UnicodeWidthStr;

/// 【终端】【引用解析】读取连续引用标记，兼容相邻或以空格分隔的嵌套层级。
/// 参数: line 为去除前导缩进的 Markdown 行
/// 返回: 引用层数与正文；普通行返回空
pub(super) fn parse(line: &str) -> Option<(usize, &str)> {
    let mut depth = 0;
    let mut rest = line;
    while let Some(stripped) = rest.strip_prefix('>') {
        depth += 1;
        rest = stripped.strip_prefix(' ').unwrap_or(stripped);
    }
    (depth > 0).then_some((depth, rest))
}

/// 【终端】【引用布局】先为引用栏预留宽度，再折行正文并为每个物理行恢复引用栏。
/// 参数: indent 为源行缩进，depth 为引用层数，body 为已渲染行内样式的 ANSI 正文
/// 返回: 可包含换行的引用文本，正文保持正常对比度，行内强调可跨行延续
pub(super) fn render(indent: &str, depth: usize, body: &str) -> String {
    // 1. 【终端】【引用宽度】TUI 使用正文净宽，CLI 默认扣除外层引导区
    let width = super::render_width::render_width_override().unwrap_or_else(|| {
        super::markdown_blocks::horizontal_rule_width()
            .saturating_sub(CONTENT_LEFT_INDENT)
            .max(1)
    });
    // 2. 【终端】【引用层级】窄窗口压缩过深层级与缩进，为全角正文至少预留两列
    let gutter_budget = width.saturating_sub(2);
    let visible_depth = depth.min(gutter_budget / 2);
    let indent_width = UnicodeWidthStr::width(indent.replace('\t', "    ").as_str())
        .min(gutter_budget.saturating_sub(visible_depth * 2));
    let gutter = format!("{}{}", " ".repeat(indent_width), "│ ".repeat(visible_depth));
    let body_width = width
        .saturating_sub(indent_width + visible_depth * 2)
        .max(1);
    // 3. 【终端】【引用折行】只折正文，结构样式在正文前重置，避免引用线影响加粗与代码
    AnsiLine::wrap_block(body, body_width)
        .into_iter()
        .map(|line| format!("{MD_QUOTE_BAR_STYLE}{gutter}{RESET}{}", line.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

use crate::render::markdown::MarkdownStreamRenderer;

/// 助手 Markdown 的未换行源数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MarkdownCell {
    pub(crate) source: String,
}

/// 由原始 Markdown 源重新构造 ANSI 文本。
///
/// 参数:
/// - `cell`: Markdown 源数据
///
/// 返回:
/// - ANSI 文本块
pub(crate) fn render(cell: &MarkdownCell) -> String {
    let mut renderer = MarkdownStreamRenderer::new_stable();
    let mut output = renderer.push(&cell.source);
    output.push_str(&renderer.flush());
    output.trim_end_matches('\n').to_string()
}

/// 渲染流式过程中可展示的 Markdown 内容（供 TUI 全量重绘 live tail）。
///
/// 已闭合表格按全表列宽输出；末尾尚未闭合的表格输出当前最优列宽预览。
/// 不强制关闭未完成的代码块等其它结构。
///
/// 参数:
/// - `source`: 当前完整 Markdown 流式源
///
/// 返回:
/// - 可安全展示在 live 区的 ANSI 文本
pub(crate) fn render_completed(source: &str) -> String {
    let mut renderer = MarkdownStreamRenderer::new_source_preview();
    // 1. 推入已完成行（闭合表格会在后续非表格行/空行时 finish）
    let mut output = renderer.push(source);
    // 2. 末尾仍开放的表格：按当前行集合重算列宽预览
    output.push_str(&renderer.snapshot_open_structures());
    output.trim_end_matches('\n').to_string()
}

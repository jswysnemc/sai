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

/// 渲染流式过程中已经完成的 Markdown 行，不刷新未结束尾部。
///
/// 参数:
/// - `source`: 当前完整 Markdown 流式源
///
/// 返回:
/// - 可安全追加到历史区的 ANSI 文本
pub(crate) fn render_completed(source: &str) -> String {
    let mut renderer = MarkdownStreamRenderer::new_source_preview();
    renderer.push(source).trim_end_matches('\n').to_string()
}

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

/// 渲染完整 Markdown 快照，包括没有换行符的最后一行。
///
/// 参数:
/// - `source`: 当前完整 Markdown 正文
///
/// 返回:
/// - 可反复重绘的 ANSI 文本
pub(crate) fn render_completed(source: &str) -> String {
    render(&MarkdownCell {
        source: source.to_string(),
    })
}

/// 渲染流式 Markdown，并报告尾部是否仍有开放结构。
///
/// 开放表格的列宽会随后续行回溯变化，其已渲染行属于「随时会变」的
/// 临时内容；普通正文的前缀渲染稳定，可以安全滚入 scrollback。
///
/// 参数:
/// - `source`: 当前完整 Markdown 流式源
///
/// 返回:
/// - `(ANSI 文本, 是否存在开放结构)`
pub(crate) fn render_completed_parts(source: &str) -> (String, bool) {
    let mut renderer = MarkdownStreamRenderer::new_source_preview();
    // 1. 推入已完成行（闭合表格会在后续非表格行/空行时 finish）
    let mut output = renderer.push(source);
    // 2. 末尾仍开放的表格：按当前行集合重算列宽预览
    let open = renderer.snapshot_open_structures();
    let transient = !open.is_empty();
    output.push_str(&open);
    (output.trim_end_matches('\n').to_string(), transient)
}

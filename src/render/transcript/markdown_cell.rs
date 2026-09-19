use crate::render::markdown::MarkdownStreamRenderer;
use crate::render::table::live_layout::TableLayouts;

/// 助手 Markdown 的未换行源数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MarkdownCell {
    pub(crate) source: String,
    pub(crate) table_layouts: TableLayouts,
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
    renderer.set_table_layouts(cell.table_layouts.clone(), None);
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
        table_layouts: TableLayouts::default(),
    })
}

/// 【终端】【流式正文】渲染完整流式内容，长表格沿用已经固定的列宽
/// 参数: source 为完整源码，layouts 为布局记录，freeze_after 为可变预览行数预算
/// 返回: 可逐行滚入终端历史的 ANSI 文本
#[cfg(test)]
pub(crate) fn render_completed_parts(
    source: &str,
    layouts: &mut TableLayouts,
    freeze_after: usize,
) -> String {
    let mut renderer = MarkdownStreamRenderer::new_source_preview();
    renderer.set_table_layouts(std::mem::take(layouts), Some(freeze_after));
    // 1. 【终端】【流式正文】推入已完成行，闭合表格沿用对应序号的固定布局
    let mut output = renderer.push(source);
    // 2. 【终端】【流式正文】追加完整开放表格，保存其布局供下次预览和定稿复用
    let open = renderer.snapshot_open_structures();
    output.push_str(&open);
    *layouts = renderer.take_table_layouts();
    output.trim_end_matches('\n').to_string()
}

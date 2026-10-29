use super::ToolView;
use crate::render::{
    status_style::ToolHealth, tool_event_line::tool_status_line, ToolCallDisplayMode,
};
use serde_json::Value;

/// 【终端】【结果卡片】统一标题、详情折叠及权限审计，避免专用视图重复布局。
/// 参数: 工具视图、显示模式、标题、状态及正文；返回: ANSI 卡片。
pub(super) fn render(
    _view: &ToolView,
    mode: ToolCallDisplayMode,
    title: &str,
    health: ToolHealth,
    body: &str,
) -> String {
    let title = super::safe_text::plain(title).replace(['\n', '\r', '\t'], " ");
    let mut output = tool_status_line(&title, "", health);
    if !body.is_empty() && (mode == ToolCallDisplayMode::Full || health == ToolHealth::Err) {
        let width = crate::render::fold_text::terminal_wrap_width()
            .saturating_sub(2)
            .max(1);
        let wrapped = super::result_rows::wrap(body, width);
        for line in crate::render::fold_text::fold_preview_lines(&wrapped, false) {
            output.push('\n');
            output.push_str("  ");
            match line {
                crate::render::fold_text::FoldedDisplayLine::Line(text) => output.push_str(&text),
                crate::render::fold_text::FoldedDisplayLine::Omitted { omitted, .. } => output
                    .push_str(&crate::render::omitted_line::render_omitted_line(
                        omitted, true,
                    )),
            }
        }
    }
    output
}

/// 【终端】【结果字段】读取非空文本并去除终端控制序列，保留内容换行。
/// 参数: 数据对象和字段名；返回: 可显示文本。
pub(super) fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(super::safe_text::plain)
        .filter(|text| !text.trim().is_empty())
}

/// 【终端】【摘要文本】清理控制序列与换行，并按显示列数截取单行主题。
/// 参数: 原始文本和列宽；返回: 可安全放入标题的短文本。
pub(super) fn short(text: &str, width: usize) -> String {
    let text = super::safe_text::plain(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    crate::render::clip_to_width(&text, width, "…")
}

/// 【终端】【结果正文】复用完整 Markdown 渲染，不把结果中的标记原样显示。
/// 参数: 结果正文；返回: ANSI 文本。
pub(super) fn markdown(text: &str) -> String {
    let text = super::safe_text::plain(text);
    let mut renderer = crate::render::markdown::MarkdownStreamRenderer::new_stable();
    let mut output = renderer.push(&text);
    output.push_str(&renderer.flush());
    output.trim_end().to_string()
}

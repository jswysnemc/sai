use super::line::AnsiLine;
use super::welcome_cell::WelcomeCell;
use crate::render::content_indent::CONTENT_LEFT_INDENT;

/// 【终端】【简洁欢迎】组合原生品牌标志与紧凑会话信息，不增加边框或背景。
/// 参数: `cell` 为欢迎源数据，`width` 为终端宽度；返回: 预换行信息行。
pub(super) fn welcome_lines(cell: &WelcomeCell, width: usize) -> Vec<AnsiLine> {
    super::welcome_brand::render(cell, width)
}

/// 【终端】【简洁正文】首行使用轻量引导符，续行与现有正文列对齐。
/// 参数: `rendered` 为 Markdown 渲染结果，`width` 为正文净宽；返回: 带引导区的物理行。
pub(super) fn assistant_lines(rendered: &str, width: usize) -> Vec<AnsiLine> {
    let prefix = " ".repeat(CONTENT_LEFT_INDENT);
    let mut marked = false;
    super::list_wrap::wrap(rendered, width, CONTENT_LEFT_INDENT)
        .into_iter()
        .map(|line| {
            if !marked && !line.as_str().trim().is_empty() {
                marked = true;
                if super::list_wrap::content_column(line.as_str()).is_some() {
                    AnsiLine::new(format!("{prefix}{}", line.as_str()))
                } else {
                    AnsiLine::new(format!("\x1b[36m●\x1b[0m {}", line.as_str()))
                }
            } else {
                AnsiLine::new(format!("{prefix}{}", line.as_str()))
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "presentation_tests.rs"]
mod tests;

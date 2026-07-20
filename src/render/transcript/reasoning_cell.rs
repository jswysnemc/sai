use crate::render::work_status::STATUS_PULSE_FRAMES;
use crate::render::ReasoningDisplayMode;
use crate::token_counter;

/// reasoning 内容的原始 source 数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReasoningCell {
    pub(crate) source: String,
}

/// 依据当前展示模式渲染 reasoning 内容。
///
/// 参数:
/// - `cell`: reasoning 源数据
/// - `mode`: reasoning 展示模式
///
/// 返回:
/// - ANSI 文本块
pub(crate) fn render(cell: &ReasoningCell, mode: ReasoningDisplayMode) -> String {
    match mode {
        ReasoningDisplayMode::Hidden => String::new(),
        ReasoningDisplayMode::Summary => {
            let tokens = token_counter::count(&cell.source);
            format!("\x1b[2m\x1b[36m• thinking · {tokens} tokens\x1b[0m")
        }
        // Codex 风格：标题 + gutter 正文
        ReasoningDisplayMode::Full => render_expanded_thinking(&cell.source),
    }
}

/// 渲染流式阶段持续刷新的 reasoning 摘要。
///
/// 参数:
/// - `source`: 当前累计的 reasoning 原文
/// - `mode`: 当前 reasoning 展示模式
/// - `frame`: 跳动动画帧序号
///
/// 返回:
/// - 可直接显示的 ANSI 摘要行
pub(crate) fn render_live(source: &str, mode: ReasoningDisplayMode, frame: usize) -> String {
    if mode == ReasoningDisplayMode::Hidden || source.is_empty() {
        return String::new();
    }
    let pulse = STATUS_PULSE_FRAMES[frame % STATUS_PULSE_FRAMES.len()];
    let tokens = token_counter::count(source);
    format!("\x1b[2m\x1b[36m{pulse} thinking · {tokens} tokens\x1b[0m")
}

/// 展开思考正文为 Codex 输出 gutter 风格。
///
/// 参数:
/// - `source`: 思考原文
///
/// 返回:
/// - 带 gutter 的 ANSI 文本
fn render_expanded_thinking(source: &str) -> String {
    let tokens = token_counter::count(source);
    let mut output = format!("\x1b[1m\x1b[36m•\x1b[0m \x1b[1mthinking\x1b[0m \x1b[2m· {tokens} tokens\x1b[0m");
    let body = source.trim_end();
    if body.is_empty() {
        return output;
    }
    for (index, line) in body.lines().enumerate() {
        let prefix = if index == 0 { "  └ " } else { "    " };
        output.push_str(&format!("\n\x1b[2m\x1b[36m{prefix}{line}\x1b[0m"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_reasoning_reports_token_count() {
        let rendered = render_live("hello world", ReasoningDisplayMode::Summary, 0);
        assert!(rendered.contains("tokens"));
        assert!(rendered.contains("thinking"));
    }

    #[test]
    fn full_reasoning_uses_codex_gutter() {
        let rendered = render(
            &ReasoningCell {
                source: "line one\nline two".to_string(),
            },
            ReasoningDisplayMode::Full,
        );
        assert!(rendered.contains("thinking"));
        assert!(rendered.contains("└"));
        assert!(rendered.contains("line one"));
        assert!(rendered.contains("line two"));
        assert!(rendered.contains("tokens"));
    }
}

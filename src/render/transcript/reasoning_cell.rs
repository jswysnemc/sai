use crate::i18n::text as t;
use crate::render::work_status::STATUS_PULSE_FRAMES;
use crate::render::ReasoningDisplayMode;
use crate::token_counter;

/// 思考段落折叠时首尾各保留的行数（与命令输出预览一致）。
const THINKING_PREVIEW_LINES: usize = 5;
/// 折叠计数用的虚拟行宽（无换行长文也能折叠）。
const THINKING_WRAP_WIDTH: usize = 96;

/// reasoning 内容的原始 source 数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReasoningCell {
    pub(crate) source: String,
    /// 是否展开完整思考正文；默认折叠。
    pub(crate) expanded: bool,
}

impl ReasoningCell {
    /// 创建默认折叠的思考单元。
    ///
    /// 参数:
    /// - `source`: 原始 reasoning 文本
    ///
    /// 返回:
    /// - 思考单元
    pub(crate) fn new(source: String) -> Self {
        Self {
            source,
            expanded: false,
        }
    }

    /// 切换展开/折叠状态。
    ///
    /// 返回:
    /// - 无
    pub(crate) fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }
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
            format!(
                "\x1b[2m\x1b[36m• {} · {tokens} tokens\x1b[0m",
                t("thinking", "思考")
            )
        }
        // Full：标题 + gutter 正文，默认折叠，Ctrl+O 切换
        ReasoningDisplayMode::Full => render_thinking_body(&cell.source, cell.expanded, true),
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
    format!(
        "\x1b[2m\x1b[36m{pulse} {} · {tokens} tokens\x1b[0m",
        t("thinking", "思考")
    )
}

/// 将思考正文渲染为可折叠 gutter 块（CLI / TUI 共用）。
///
/// 参数:
/// - `source`: 思考原文
/// - `expanded`: 是否展开全部
/// - `show_expand_hint`: 是否显示 Ctrl+O 提示
///
/// 返回:
/// - 带 gutter 的 ANSI 文本
pub(crate) fn render_thinking_body(source: &str, expanded: bool, show_expand_hint: bool) -> String {
    let tokens = token_counter::count(source);
    let title = format!(
        "\x1b[1m\x1b[36m•\x1b[0m \x1b[1m{}\x1b[0m \x1b[2m· {tokens} tokens\x1b[0m",
        t("thinking", "思考")
    );
    let body = source.trim_end();
    if body.is_empty() {
        return title;
    }
    // 1. 按换行拆分，再把超长行切成虚拟行，避免“一整段无换行”永远不折叠
    let lines = visual_thinking_lines(body);
    let (visible, omitted) = if expanded || lines.len() <= THINKING_PREVIEW_LINES * 2 {
        (lines.clone(), 0usize)
    } else {
        let mut visible = Vec::with_capacity(THINKING_PREVIEW_LINES * 2 + 1);
        visible.extend_from_slice(&lines[..THINKING_PREVIEW_LINES]);
        visible.push("__OMITTED__".to_string());
        visible.extend_from_slice(&lines[lines.len() - THINKING_PREVIEW_LINES..]);
        (visible, lines.len() - THINKING_PREVIEW_LINES * 2)
    };

    let mut output = title;
    let mut content_index = 0usize;
    for line in visible {
        if line == "__OMITTED__" {
            let hint = if show_expand_hint {
                format!(
                    "… +{omitted} {} (Ctrl+O {})",
                    t("lines", "行"),
                    t("to expand", "展开")
                )
            } else {
                format!("… +{omitted} {}", t("lines", "行"))
            };
            output.push_str(&format!("\n\x1b[2m\x1b[36m  └ {hint}\x1b[0m"));
            continue;
        }
        let prefix = if content_index == 0 { "  └ " } else { "    " };
        content_index += 1;
        output.push_str(&format!("\n\x1b[2m\x1b[36m{prefix}{line}\x1b[0m"));
    }
    output
}

/// 将思考正文拆成用于折叠计数的显示行。
///
/// 参数:
/// - `body`: 去尾空白后的思考正文
///
/// 返回:
/// - 显示行列表
fn visual_thinking_lines(body: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for raw in body.lines() {
        if raw.is_empty() {
            lines.push(String::new());
            continue;
        }
        let chars: Vec<char> = raw.chars().collect();
        if chars.len() <= THINKING_WRAP_WIDTH {
            lines.push(raw.to_string());
            continue;
        }
        for chunk in chars.chunks(THINKING_WRAP_WIDTH) {
            lines.push(chunk.iter().collect());
        }
    }
    if lines.is_empty() {
        lines.push(body.to_string());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_reasoning_reports_token_count() {
        let rendered = render_live("hello world", ReasoningDisplayMode::Summary, 0);
        assert!(rendered.contains("tokens"));
        assert!(rendered.contains("thinking") || rendered.contains("思考"));
    }

    #[test]
    fn full_reasoning_uses_codex_gutter() {
        let rendered = render(
            &ReasoningCell {
                source: "line one\nline two".to_string(),
                expanded: true,
            },
            ReasoningDisplayMode::Full,
        );
        assert!(rendered.contains("thinking") || rendered.contains("思考"));
        assert!(rendered.contains("└"));
        assert!(rendered.contains("line one"));
        assert!(rendered.contains("line two"));
        assert!(rendered.contains("tokens"));
    }

    #[test]
    fn collapsed_long_reasoning_shows_expand_hint() {
        let source = (1..=12)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let collapsed = render(
            &ReasoningCell {
                source: source.clone(),
                expanded: false,
            },
            ReasoningDisplayMode::Full,
        );
        assert!(collapsed.contains("line 1"));
        assert!(collapsed.contains("line 12"));
        assert!(!collapsed.contains("line 6"));
        assert!(collapsed.contains("Ctrl+O"));
        assert!(collapsed.contains("+2"));

        let expanded = render(
            &ReasoningCell {
                source,
                expanded: true,
            },
            ReasoningDisplayMode::Full,
        );
        assert!(expanded.contains("line 6"));
        assert!(!expanded.contains("Ctrl+O"));
    }

    #[test]
    fn collapsed_long_single_line_reasoning_folds() {
        let source = "字".repeat(THINKING_WRAP_WIDTH * 12);
        let collapsed = render_thinking_body(&source, false, true);
        assert!(collapsed.contains("Ctrl+O"));
        assert!(collapsed.contains('…'));
        let expanded = render_thinking_body(&source, true, true);
        assert!(!expanded.contains("Ctrl+O"));
    }
}

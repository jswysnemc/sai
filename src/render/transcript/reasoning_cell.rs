use crate::i18n::text as t;
use crate::render::fold_text::{
    fold_display_lines, terminal_wrap_width, wrap_display_lines, FOLD_PREVIEW_LINES,
};
use crate::render::work_status::{format_elapsed, STATUS_PULSE_FRAMES};
use crate::render::ReasoningDisplayMode;
use crate::token_counter;
use std::time::Duration;

/// reasoning 内容的原始 source 数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReasoningCell {
    pub(crate) source: String,
    /// 是否展开完整思考正文；默认折叠。
    pub(crate) expanded: bool,
    /// 本段思考耗时；有值时在标题中展示。
    pub(crate) duration: Option<Duration>,
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
            duration: None,
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
                "\x1b[2m\x1b[36m• {}{}\x1b[0m",
                thinking_label(cell.duration),
                format_tokens_suffix(tokens)
            )
        }
        // Full：标题 + gutter 正文，默认折叠，Ctrl+O 切换
        ReasoningDisplayMode::Full => {
            render_thinking_body(&cell.source, cell.expanded, true, cell.duration)
        }
    }
}

/// 渲染流式阶段持续刷新的 reasoning 摘要。
///
/// 参数:
/// - `source`: 当前累计的 reasoning 原文
/// - `mode`: 当前 reasoning 展示模式
/// - `frame`: 跳动动画帧序号
/// - `elapsed`: 本段思考已持续时长
///
/// 返回:
/// - 可直接显示的 ANSI 摘要行
pub(crate) fn render_live(
    source: &str,
    mode: ReasoningDisplayMode,
    frame: usize,
    elapsed: Duration,
) -> String {
    if mode == ReasoningDisplayMode::Hidden || source.is_empty() {
        return String::new();
    }
    let pulse = STATUS_PULSE_FRAMES[frame % STATUS_PULSE_FRAMES.len()];
    let tokens = token_counter::count(source);
    format!(
        "\x1b[2m\x1b[36m{pulse} {}{}\x1b[0m",
        thinking_label(Some(elapsed)),
        format_tokens_suffix(tokens)
    )
}

/// 将思考正文渲染为可折叠 gutter 块（CLI / TUI 共用）。
///
/// 参数:
/// - `source`: 思考原文
/// - `expanded`: 是否展开全部
/// - `show_expand_hint`: 是否显示 Ctrl+O 提示
/// - `duration`: 可选思考耗时
///
/// 返回:
/// - 带 gutter 的 ANSI 文本
pub(crate) fn render_thinking_body(
    source: &str,
    expanded: bool,
    show_expand_hint: bool,
    duration: Option<Duration>,
) -> String {
    let tokens = token_counter::count(source);
    let title = format!(
        "\x1b[1m\x1b[36m•\x1b[0m \x1b[1m{}\x1b[0m\x1b[2m{}\x1b[0m",
        thinking_label(duration),
        format_tokens_suffix(tokens)
    );
    let body = source.trim_end();
    if body.is_empty() {
        return title;
    }
    // 1. 按终端实际显示宽度折行后计数，避免无换行长行挤占视野
    let lines = wrap_display_lines(body, terminal_wrap_width());
    let (visible, omitted) = fold_display_lines(&lines, FOLD_PREVIEW_LINES, expanded);

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

/// 生成思考标题（含可选耗时，与未开启正文时的状态行动效文案一致）。
///
/// 参数:
/// - `duration`: 可选耗时
///
/// 返回:
/// - 如 `thinking(12s)` / `思考(12秒)`
fn thinking_label(duration: Option<Duration>) -> String {
    let base = t("thinking", "思考");
    match duration {
        Some(elapsed) => format!("{base}({})", format_elapsed(elapsed)),
        None => base.to_string(),
    }
}

/// 生成 token 计数后缀。
///
/// 参数:
/// - `tokens`: token 数
///
/// 返回:
/// - 如 ` · 12 tokens`
fn format_tokens_suffix(tokens: usize) -> String {
    format!(" · {tokens} tokens")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_reasoning_reports_token_count() {
        let rendered = render_live(
            "hello world",
            ReasoningDisplayMode::Summary,
            0,
            Duration::from_secs(12),
        );
        assert!(rendered.contains("tokens"));
        assert!(rendered.contains("thinking") || rendered.contains("思考"));
        assert!(rendered.contains("12") || rendered.contains("s") || rendered.contains("秒"));
    }

    #[test]
    fn full_reasoning_uses_codex_gutter() {
        let rendered = render(
            &ReasoningCell {
                source: "line one\nline two".to_string(),
                expanded: true,
                duration: Some(Duration::from_secs(3)),
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
                duration: None,
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
                duration: None,
            },
            ReasoningDisplayMode::Full,
        );
        assert!(expanded.contains("line 6"));
        assert!(!expanded.contains("Ctrl+O"));
    }

    #[test]
    fn collapsed_long_single_line_reasoning_folds() {
        let source = "字".repeat(96 * 12);
        let collapsed = render_thinking_body(&source, false, true, None);
        assert!(collapsed.contains("Ctrl+O"));
        assert!(collapsed.contains('…'));
        let expanded = render_thinking_body(&source, true, true, None);
        assert!(!expanded.contains("Ctrl+O"));
    }
}

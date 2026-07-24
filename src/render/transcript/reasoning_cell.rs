use crate::i18n::text as t;
use crate::render::fold_text::{
    fold_display_lines, terminal_wrap_width, wrap_display_lines, FOLD_HEAD_LINES, FOLD_TAIL_LINES,
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

/// 思考 gutter 前缀显示宽度：`  └ ` / `    `
const THINKING_GUTTER_WIDTH: usize = 4;

/// 计算思考正文折行宽度（预留 gutter，避免拼前缀后超出终端列数）。
///
/// 参数:
/// - `terminal_cols`: 终端列数
///
/// 返回:
/// - 正文可用显示宽度
fn thinking_body_wrap_width(terminal_cols: usize) -> usize {
    terminal_cols
        .saturating_sub(THINKING_GUTTER_WIDTH)
        .max(8)
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
    render_thinking_body_with_cols(
        source,
        expanded,
        show_expand_hint,
        duration,
        terminal_wrap_width(),
    )
}

/// 按指定终端列数渲染思考正文（供测试固定宽度）。
///
/// 参数:
/// - `source`: 思考原文
/// - `expanded`: 是否展开全部
/// - `show_expand_hint`: 是否显示 Ctrl+O 提示
/// - `duration`: 可选思考耗时
/// - `terminal_cols`: 终端列数
///
/// 返回:
/// - 带 gutter 的 ANSI 文本
fn render_thinking_body_with_cols(
    source: &str,
    expanded: bool,
    show_expand_hint: bool,
    duration: Option<Duration>,
    terminal_cols: usize,
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
    // 1. 按「终端列数 - gutter」折行，再拼 `  └ `/`    `，保证最终行宽不超过终端
    let lines = wrap_display_lines(body, thinking_body_wrap_width(terminal_cols));
    let (visible, omitted) = fold_display_lines(&lines, FOLD_HEAD_LINES, FOLD_TAIL_LINES, expanded);

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
    let base = "thinking";
    match duration {
        // 固定英文：thinking (12s)，避免中文「秒」与英文标签混排
        Some(elapsed) => format!("{base} ({})", format_elapsed(elapsed)),
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
        assert!(rendered.contains("12s") || rendered.contains("12"));
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
        // 12 行、前 2 后 4，中间省略 6 行
        assert!(collapsed.contains("+6"));

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

    #[test]
    fn thinking_body_gutter_lines_fit_terminal_cols() {
        // 1. 用固定 80 列复现「先满宽折行再加 gutter」会溢出的场景
        let cols = 80usize;
        let source = "These completion events are just finish receipts for the background tools/commands I launched during the commit, push, and CI monitoring workflow.";
        let rendered = render_thinking_body_with_cols(&source, true, true, None, cols);
        let plain = strip_ansi(&rendered);
        let mut body_lines = plain.lines().skip(1);
        let first = body_lines.next().expect("first gutter line");
        assert!(first.starts_with("  └ "), "first body line should use tree gutter: {first}");
        for line in std::iter::once(first).chain(body_lines) {
            let width = visible_width(line);
            assert!(
                width <= cols,
                "cols={cols} width={width} line={line:?}"
            );
        }
        // 2. 续行保留 gutter 缩进，而不是被二次折行挤成无缩进碎片
        let continuation = plain.lines().nth(2).expect("continuation line");
        assert!(
            continuation.starts_with("    "),
            "continuation should keep gutter indent: {continuation}"
        );
    }

    #[test]
    fn thinking_body_wrap_width_reserves_gutter() {
        assert_eq!(thinking_body_wrap_width(80), 76);
        assert_eq!(thinking_body_wrap_width(10), 8);
        assert_eq!(thinking_body_wrap_width(4), 8);
    }

    /// 去掉 ANSI 转义，便于断言纯文本布局。
    fn strip_ansi(text: &str) -> String {
        let mut out = String::new();
        let mut escape = false;
        for ch in text.chars() {
            if ch == '\x1b' {
                escape = true;
                continue;
            }
            if escape {
                if ch == 'm' {
                    escape = false;
                }
                continue;
            }
            out.push(ch);
        }
        out
    }

    /// 计算纯文本显示宽度。
    fn visible_width(text: &str) -> usize {
        text.chars()
            .map(|ch| unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0))
            .sum()
    }
}

use crate::render::activity_animation::{render_activity_detail, render_activity_text};
use crate::render::fold_text::{
    fold_display_lines, terminal_wrap_width, wrap_display_lines, FOLD_HEAD_LINES, FOLD_TAIL_LINES,
};
use crate::render::work_status::format_elapsed;
use crate::render::ReasoningDisplayMode;
use crate::token_counter;
use std::time::Duration;

const THINKING_LABEL: &str = "Thinking";
const THOUGHT_LABEL: &str = "Thought";
/// 思考专用引导符：与工具行的 `•`、正文无符缩进区分开（CLI Summary / Full / 非流式共用）
pub(crate) const THINKING_MARKER: &str = "◦";

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
                "\x1b[2m\x1b[36m{THINKING_MARKER} {}{}\x1b[0m",
                thought_label(cell.duration),
                format_tokens_suffix(tokens)
            )
        }
        // Full：标题 + gutter 正文，默认折叠，Ctrl+O 切换
        ReasoningDisplayMode::Full => {
            render_thinking_body(&cell.source, cell.expanded, true, cell.duration)
        }
    }
}

/// 【终端】【思考状态】渲染流式阶段持续刷新的 reasoning 摘要。
///
/// 参数:
/// - `source`: 当前累计的 reasoning 原文
/// - `mode`: 当前 reasoning 展示模式
/// - `frame`: 文字扫光动画帧序号
/// - `elapsed`: 本段思考已持续时长
/// - `expanded`: 当前实时思考是否展开
///
/// 返回:
/// - 可直接显示的 ANSI 摘要行
pub(crate) fn render_live(
    source: &str,
    mode: ReasoningDisplayMode,
    frame: usize,
    elapsed: Duration,
    expanded: bool,
) -> String {
    if mode == ReasoningDisplayMode::Hidden || source.is_empty() {
        return String::new();
    }
    let tokens = token_counter::count(source);
    let detail = format!(
        "{}{}",
        duration_suffix(duration_label_value(elapsed)),
        format_tokens_suffix(tokens)
    )
    .trim_start()
    .to_string();
    // 1. 思考用 ◦ 引导 + 扫光标题，避免与工具行的 • 抢同一视觉层级
    let title = format!(
        "\x1b[2m\x1b[36m{THINKING_MARKER}\x1b[0m {}{}",
        render_activity_text(THINKING_LABEL, frame),
        if detail.is_empty() {
            String::new()
        } else {
            format!(" {}", render_activity_detail(&detail))
        }
    );
    match mode {
        ReasoningDisplayMode::Hidden => String::new(),
        ReasoningDisplayMode::Summary => title,
        // 2. 【终端】【思考流式】正文与定稿块共用折行、折叠和展开提示
        ReasoningDisplayMode::Full => {
            render_thinking_body_with_title(source, expanded, true, terminal_wrap_width(), title)
        }
    }
}

/// 将已持续时长转换为标签用耗时（零值视为无耗时）。
///
/// 参数:
/// - `elapsed`: 已持续时长
///
/// 返回:
/// - 非零时长；零值返回 None
pub(crate) fn duration_label_value(elapsed: Duration) -> Option<Duration> {
    (!elapsed.is_zero()).then_some(elapsed)
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
    // 下限保证极窄终端下正文还有可读宽度，但必须再夹回终端列数：
    // 只写 .max(8) 时 4 列终端会按 8 列折行，再拼上 4 列 gutter 就超出屏幕
    let cols = terminal_cols.max(1);
    terminal_cols
        .saturating_sub(THINKING_GUTTER_WIDTH)
        .max(8)
        .min(cols)
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
    // 定稿标题用弱化 ◦，避免与工具行的加粗 • 抢同一层级
    let title = format!(
        "\x1b[2m\x1b[36m{THINKING_MARKER}\x1b[0m \x1b[2m{}\x1b[0m\x1b[2m{}\x1b[0m",
        thought_label(duration),
        format_tokens_suffix(tokens)
    );
    render_thinking_body_with_title(source, expanded, show_expand_hint, terminal_cols, title)
}

/// 使用指定标题渲染统一的思考正文块。
///
/// 参数:
/// - `source`: 思考原文
/// - `expanded`: 是否展开全部
/// - `show_expand_hint`: 是否显示 Ctrl+O 提示
/// - `terminal_cols`: 终端列数
/// - `title`: 已完成样式处理的标题行
///
/// 返回:
/// - 带统一 gutter、折行和折叠状态的 ANSI 文本
fn render_thinking_body_with_title(
    source: &str,
    expanded: bool,
    show_expand_hint: bool,
    terminal_cols: usize,
    title: String,
) -> String {
    let body = source.trim_end();
    if body.is_empty() {
        return title;
    }
    // 1. 按「终端列数 - gutter」折行，再拼 `  └ `/`    `，保证最终行宽不超过终端
    let wrapped = wrap_display_lines(body, thinking_body_wrap_width(terminal_cols));
    // 2. 丢掉空段落：模型常在思考里插 `\n\n`，渲染成 gutter 空行会像「块内硬隔开」；
    //    思考与后续正文的间距由 cell / live 的前空行负责，工具块不加前空行
    let lines: Vec<String> = wrapped
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let (visible, omitted) = fold_display_lines(&lines, FOLD_HEAD_LINES, FOLD_TAIL_LINES, expanded);

    let mut output = title;
    let mut content_index = 0usize;
    for line in visible {
        if line == "__OMITTED__" {
            output.push('\n');
            output.push_str(&crate::render::omitted_line::render_omitted_line(
                omitted,
                show_expand_hint,
            ));
            continue;
        }
        let prefix = if content_index == 0 { "  └ " } else { "    " };
        content_index += 1;
        output.push_str(&format!("\n\x1b[2m\x1b[36m{prefix}{line}\x1b[0m"));
    }
    output
}

/// 生成流式思考标题（进行时）。
///
/// 参数:
/// - `duration`: 可选耗时
///
/// 返回:
/// - 如 `Thinking (12s)`；无耗时则仅 `Thinking`
pub(crate) fn thinking_label(duration: Option<Duration>) -> String {
    format!("{THINKING_LABEL}{}", duration_suffix(duration))
}

/// 生成定稿思考标题（完成后再用过去式）。
///
/// 参数:
/// - `duration`: 可选耗时
///
/// 返回:
/// - 如 `Thought (12s)`；无耗时则仅 `Thought`
pub(crate) fn thought_label(duration: Option<Duration>) -> String {
    format!("{THOUGHT_LABEL}{}", duration_suffix(duration))
}

/// 生成思考耗时后缀。
///
/// 参数:
/// - `duration`: 可选思考耗时
///
/// 返回:
/// - 如 ` (12s)`；无耗时则返回空字符串
fn duration_suffix(duration: Option<Duration>) -> String {
    match duration {
        // 固定英文格式，避免中文时间单位与英文状态文案混排
        Some(elapsed) => format!(" ({})", format_elapsed(elapsed)),
        None => String::new(),
    }
}

/// 生成 token 计数后缀（CLI / TUI 各阶段共用的唯一实现）。
///
/// 参数:
/// - `tokens`: token 数
///
/// 返回:
/// - 如 ` · 12 tokens`
pub(crate) fn format_tokens_suffix(tokens: usize) -> String {
    // 与底栏/会话总结一致的 k 单位：思考 token 动辄数千，原始数字读起来费劲
    format!(
        " · {} tokens",
        crate::render::session_summary::format_k(tokens)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test;

    #[test]
    fn live_reasoning_reports_token_count() {
        let rendered = render_live(
            "hello world",
            ReasoningDisplayMode::Summary,
            0,
            Duration::from_secs(12),
            false,
        );
        let plain = strip_ansi_for_test(&rendered);
        assert!(plain.contains("tokens"));
        assert!(plain.contains("Thinking (12s)"));
    }

    /// 【终端】【思考统计】token 后缀超过千位用 k 单位，与底栏风格一致。
    #[test]
    fn tokens_suffix_uses_k_unit() {
        assert_eq!(format_tokens_suffix(42), " · 42 tokens");
        assert_eq!(format_tokens_suffix(1_500), " · 1.5k tokens");
        assert_eq!(format_tokens_suffix(12_345), " · 12k tokens");
    }

    /// 【终端】【思考时态】定稿标题用过去式，流式标题保持进行时。
    #[test]
    fn finalized_reasoning_title_uses_past_tense() {
        assert_eq!(thought_label(Some(Duration::from_secs(3))), "Thought (3s)");
        assert_eq!(
            thinking_label(Some(Duration::from_secs(3))),
            "Thinking (3s)"
        );
        let rendered = render(
            &ReasoningCell {
                source: "done".to_string(),
                expanded: true,
                duration: Some(Duration::from_secs(3)),
            },
            ReasoningDisplayMode::Summary,
        );
        let plain = strip_ansi_for_test(&rendered);
        assert!(plain.contains("Thought (3s)"), "{plain}");
        assert!(!plain.contains("Thinking"));
    }

    #[test]
    fn live_reasoning_omits_zero_duration() {
        // 零耗时与 CLI live 行一致：仅 Thinking，不显示 (0s)
        let rendered = render_live(
            "hello",
            ReasoningDisplayMode::Summary,
            0,
            Duration::ZERO,
            false,
        );
        let plain = strip_ansi_for_test(&rendered);
        assert!(plain.contains("Thinking"));
        assert!(!plain.contains("(0s)"));
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
        assert!(rendered.contains("Thought"));
        assert!(!rendered.contains("Thinking"));
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
        let plain = strip_ansi_for_test(&rendered);
        let mut body_lines = plain.lines().skip(1);
        let first = body_lines.next().expect("first gutter line");
        assert!(
            first.starts_with("  └ "),
            "first body line should use tree gutter: {first}"
        );
        for line in std::iter::once(first).chain(body_lines) {
            let width = visible_width(line);
            assert!(width <= cols, "cols={cols} width={width} line={line:?}");
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
        // 极窄终端：下限不能把宽度顶到超过终端列数，否则拼上 gutter 会溢出
        assert_eq!(thinking_body_wrap_width(4), 4);
    }

    #[test]
    fn thinking_body_drops_blank_paragraphs() {
        let rendered = render_thinking_body(
            "第一段说明能力。\n\n我先检查工作区再回复。",
            true,
            false,
            None,
        );
        let plain = strip_ansi_for_test(&rendered);
        let body: Vec<&str> = plain.lines().skip(1).collect();
        assert_eq!(
            body.len(),
            2,
            "blank paragraph must not become a gutter gap: {body:?}"
        );
        assert!(body[0].contains("第一段"));
        assert!(body[1].contains("我先检查"));
        assert!(body.iter().all(|line| !line.trim().is_empty()));
    }

    /// 计算纯文本显示宽度。
    fn visible_width(text: &str) -> usize {
        text.chars()
            .map(|ch| unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0))
            .sum()
    }
}

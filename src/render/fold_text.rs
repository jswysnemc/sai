use crate::config::{DisplayConfig, FoldPreviewMode};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// 折叠预览：默认保留前 2 行与后 4 行。
pub(crate) const FOLD_HEAD_LINES: usize = 2;
pub(crate) const FOLD_TAIL_LINES: usize = 4;
/// 兼容旧名：对称折叠时取 head（优先使用 FOLD_HEAD/TAIL）。
#[allow(dead_code)]
pub(crate) const FOLD_PREVIEW_LINES: usize = FOLD_HEAD_LINES;

/// 当前折叠预览：低 8 位为模式，次 8 位为开头行数，再次 8 位为结尾行数。
///
/// 默认 `ends` + 开头 2 行 + 结尾 4 行。
const DEFAULT_PACKED: u32 = 1 | (FOLD_HEAD_LINES as u32) << 8 | (FOLD_TAIL_LINES as u32) << 16;

#[cfg(not(test))]
static FOLD_PREVIEW: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(DEFAULT_PACKED);
#[cfg(test)]
thread_local! {
    static FOLD_PREVIEW: std::cell::Cell<u32> = const { std::cell::Cell::new(DEFAULT_PACKED) };
}

/// 当前折叠预览边界。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FoldPreviewBounds {
    pub(crate) head: usize,
    pub(crate) tail: usize,
    pub(crate) omit_all: bool,
}

/// 【终端显示】【折叠偏好】按显示配置更新折叠预览边界。
///
/// 参数:
/// - `config`: 显示配置
pub(crate) fn configure_fold_preview(config: &DisplayConfig) {
    let mode = match config.fold_preview {
        FoldPreviewMode::Head => 0u32,
        FoldPreviewMode::Ends => 1,
        FoldPreviewMode::Hidden => 2,
    };
    let packed = mode
        | (DisplayConfig::clamp_head(config.fold_head_lines) as u32) << 8
        | (DisplayConfig::clamp_tail(config.fold_tail_lines) as u32) << 16;
    #[cfg(not(test))]
    FOLD_PREVIEW.store(packed, std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    FOLD_PREVIEW.set(packed);
}

/// 【终端显示】【折叠边界】读取当前折叠预览保留行数。
///
/// 返回:
/// - 开头、结尾行数与是否全部省略
pub(crate) fn fold_preview_bounds() -> FoldPreviewBounds {
    #[cfg(not(test))]
    let packed = FOLD_PREVIEW.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    let packed = FOLD_PREVIEW.get();
    let mode = packed & 0xff;
    let head = DisplayConfig::clamp_head(((packed >> 8) & 0xff) as usize);
    let tail = DisplayConfig::clamp_tail(((packed >> 16) & 0xff) as usize);
    match mode {
        0 => FoldPreviewBounds {
            head,
            tail: 0,
            omit_all: false,
        },
        2 => FoldPreviewBounds {
            head: 0,
            tail: 0,
            omit_all: true,
        },
        _ => FoldPreviewBounds {
            head,
            tail,
            omit_all: false,
        },
    }
}

/// 【终端显示】【折叠预览】按当前显示配置折叠显示行。
///
/// 参数:
/// - `lines`: 显示行
/// - `expanded`: 是否展开
///
/// 返回:
/// - 折叠后的显示条目
pub(crate) fn fold_preview_lines(lines: &[String], expanded: bool) -> Vec<FoldedDisplayLine> {
    fold_preview_with(lines, expanded, fold_display_lines)
}

/// 【终端显示】【折叠预览】按当前显示配置折叠，并保留被省略的原始行。
///
/// 参数:
/// - `lines`: 显示行
/// - `expanded`: 是否展开
///
/// 返回:
/// - 折叠后的显示条目
pub(crate) fn fold_preview_lines_tracked(
    lines: &[String],
    expanded: bool,
) -> Vec<FoldedDisplayLine> {
    fold_preview_with(lines, expanded, fold_display_lines_tracked)
}

/// 按当前折叠边界调用指定折叠函数。
///
/// 参数:
/// - `lines`: 显示行
/// - `expanded`: 是否展开
/// - `fold`: 具体折叠实现
///
/// 返回:
/// - 折叠后的显示条目
fn fold_preview_with(
    lines: &[String],
    expanded: bool,
    fold: fn(&[String], usize, usize, bool) -> Vec<FoldedDisplayLine>,
) -> Vec<FoldedDisplayLine> {
    let bounds = fold_preview_bounds();
    if bounds.omit_all && !crate::render::render_expand::resolve_expanded(expanded) {
        if lines.is_empty() {
            return Vec::new();
        }
        return vec![FoldedDisplayLine::Omitted {
            omitted: lines.len(),
            skipped: lines.to_vec(),
        }];
    }
    fold(lines, bounds.head, bounds.tail, expanded)
}

/// 将纯文本按显示宽度拆成虚拟显示行（忽略 ANSI，用于折叠计数）。
///
/// 参数:
/// - `text`: 原始文本
/// - `wrap_width`: 终端列宽预算（至少 8）
///
/// 返回:
/// - 显示行列表
pub(crate) fn wrap_display_lines(text: &str, wrap_width: usize) -> Vec<String> {
    let width = wrap_width.max(8);
    let mut lines = Vec::new();
    for raw in text.lines() {
        if raw.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        let mut current_width = 0usize;
        // 按字素簇而不是逐字符计宽：ZWJ 家族 emoji（👨‍👩‍👧）与组合音标
        // （e + U+0301）的真实宽度是 0，逐字符再用 .max(1) 兜底会把它们
        // 各算成 1 列，含 emoji 的命令输出因此提前约 30% 折行，
        // 「… +N 行」的省略计数也与实际显示对不上
        for grapheme in raw.graphemes(true) {
            let grapheme_width = UnicodeWidthStr::width(grapheme);
            if current_width > 0 && current_width.saturating_add(grapheme_width) > width {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            current.push_str(grapheme);
            current_width = current_width.saturating_add(grapheme_width);
        }
        if !current.is_empty() || raw.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() && !text.is_empty() {
        lines.push(text.to_string());
    }
    lines
}

/// 对显示行做首尾折叠，中间插入省略标记。
///
/// 参数:
/// - `lines`: 显示行
/// - `head`: 头部保留行数
/// - `tail`: 尾部保留行数
/// - `expanded`: 是否展开
///
/// 返回:
/// - 显示行与结构化省略条目，正文中的同名文本不会误判为折叠符号
pub(crate) fn fold_display_lines(
    lines: &[String],
    head: usize,
    tail: usize,
    expanded: bool,
) -> Vec<FoldedDisplayLine> {
    fold_display_lines_tracked(lines, head, tail, expanded)
}

/// 折行折叠后的单个显示条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FoldedDisplayLine {
    /// 正常显示行
    Line(String),
    /// 折叠占位；`skipped` 为被省略的原始显示行，供跨行高亮状态推进
    Omitted {
        omitted: usize,
        skipped: Vec<String>,
    },
}

/// 对显示行做首尾折叠，并保留被省略的原始行。
///
/// 与 [`fold_display_lines`] 的区别：折叠处返回被省略的行本身，
/// 调用方可据此推进跨行语法高亮状态，保证尾部行的引号/注释
/// 上下文与省略前一致。
///
/// 参数:
/// - `lines`: 显示行
/// - `head`: 头部保留行数
/// - `tail`: 尾部保留行数
/// - `expanded`: 是否展开
///
/// 返回:
/// - 折叠后的显示条目序列
pub(crate) fn fold_display_lines_tracked(
    lines: &[String],
    head: usize,
    tail: usize,
    expanded: bool,
) -> Vec<FoldedDisplayLine> {
    let expanded = crate::render::render_expand::resolve_expanded(expanded);
    let keep = head.saturating_add(tail);
    if expanded || keep == 0 || lines.len() <= keep {
        return lines.iter().cloned().map(FoldedDisplayLine::Line).collect();
    }
    let omitted = lines.len() - keep;
    let tail_start = lines.len().saturating_sub(tail);
    let mut entries = Vec::with_capacity(keep + 1);
    entries.extend(lines[..head].iter().cloned().map(FoldedDisplayLine::Line));
    entries.push(FoldedDisplayLine::Omitted {
        omitted,
        skipped: lines[head..tail_start].to_vec(),
    });
    entries.extend(
        lines[tail_start..]
            .iter()
            .cloned()
            .map(FoldedDisplayLine::Line),
    );
    entries
}

/// 查询当前渲染宽度：优先使用渲染上下文注入值，否则实时查询终端。
///
/// 返回:
/// - 可用列宽（失败时回退 96）
pub(crate) fn terminal_wrap_width() -> usize {
    if let Some(width) = crate::render::render_width::render_width_override() {
        return width;
    }
    crossterm::terminal::size()
        .map(|(cols, _)| cols as usize)
        .unwrap_or(96)
        .max(8)
}

/// 命令预览行首固定装饰占用的列数（不含标题）。
///
/// 组成为：引导符、空格、标题后空格、`$ ` 两列。此前用的是一个写死
/// 六列的常量，而 `• Ran $ ` 实际占八列：首行因此比终端宽两列，被终端
/// 硬换行到第 0 列——那正是视觉引导线所在列。
const COMMAND_FIXED_PREFIX_COLUMNS: usize = 5;

/// 计算指定标题下命令正文的起始列。
///
/// 折行续行必须缩进到这一列才能与首行正文对齐；标题长度不同
/// （`Ran` 与 `Background`）时该列也随之变化。
///
/// 参数:
/// - `title`: 命令块标题
///
/// 返回:
/// - 命令正文起始列
pub(crate) fn command_body_column(title: &str) -> usize {
    COMMAND_FIXED_PREFIX_COLUMNS.saturating_add(display_columns(title))
}

/// 计算指定标题下命令预览的折行宽度。
///
/// 参数:
/// - `title`: 命令块标题
///
/// 返回:
/// - 扣除行首装饰与标题后剩余的列数
pub(crate) fn command_wrap_width_for_title(title: &str) -> usize {
    // 同 thinking_body_wrap_width：下限保证可读，但必须夹回终端列数，
    // 否则窄终端上「标题列 + 下限宽度」会一起超出屏幕，
    // 外层再硬折一次就把左侧装饰折成参差的碎片
    let cols = terminal_wrap_width().max(1);
    terminal_wrap_width()
        .saturating_sub(command_body_column(title))
        .max(COMMAND_MIN_WRAP)
        .min(cols)
}

/// 统计文本的终端显示列数。
///
/// 参数:
/// - `text`: 纯文本
///
/// 返回:
/// - 显示列数
fn display_columns(text: &str) -> usize {
    text.chars()
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum()
}

/// 命令预览折行后至少保留的列数。
///
/// 终端极窄时前缀几乎吃掉整行，仍需留出足以看清片段的宽度。
const COMMAND_MIN_WRAP: usize = 24;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::render_width::with_render_width;

    /// ZWJ 家族 emoji 按字素簇计宽（真宽 6 列），不再逐字符折算成 8 列。
    #[test]
    fn wrap_counts_emoji_zwj_sequences_by_grapheme() {
        // "ab " + 👨‍👩‍👧 + " cd"：真实显示宽度 12，逐字符折算会算成 14
        let lines = wrap_display_lines("ab \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} cd", 12);
        assert_eq!(lines.len(), 1, "emoji 不应提前折行：{lines:?}");
    }

    /// 组合音标（e + U+0301）宽度为 0，不应把每个字符各算 1 列。
    #[test]
    fn wrap_counts_combining_marks_as_zero_width() {
        let decomposed = "\u{65}\u{301}".repeat(6);
        let lines = wrap_display_lines(&decomposed, 8);
        assert_eq!(lines.len(), 1, "组合音标不应撑宽：{lines:?}");
    }

    /// 纯 ASCII 行为不变。
    #[test]
    fn wrap_still_breaks_plain_ascii_on_width() {
        assert_eq!(wrap_display_lines("abcdefghij", 8), vec!["abcdefgh", "ij"]);
    }

    /// 命令折行宽度跟随终端实际列数，不再压在固定上限上。
    ///
    /// 早先这里额外取 72 列的下界，宽终端上命令会在右侧仍有大片空白时折行。
    #[test]
    fn command_wrap_width_follows_the_terminal() {
        // `• Ran $ ` 共八列，折行宽度即终端列数减八
        assert_eq!(
            with_render_width(120, || command_wrap_width_for_title("Ran")),
            112
        );
        assert_eq!(
            with_render_width(200, || command_wrap_width_for_title("Ran")),
            192
        );
    }

    /// 更长的标题占用更多行首列，折行宽度随之收窄。
    #[test]
    fn command_wrap_width_accounts_for_the_title_width() {
        assert_eq!(command_body_column("Ran"), 8);
        assert_eq!(command_body_column("Background"), 15);
        assert_eq!(
            with_render_width(120, || command_wrap_width_for_title("Background")),
            105
        );
    }

    /// 下限优先，但不能把宽度顶到超出终端列数。
    ///
    /// 终端比 `COMMAND_MIN_WRAP` 还窄时，若仍按下限折行，「标题列 + 下限宽度」
    /// 会一起超出屏幕，外层再硬折一次就把左侧装饰折成参差的碎片。
    #[test]
    fn command_wrap_width_never_exceeds_the_terminal() {
        // 极窄终端：夹回终端列数
        assert_eq!(
            with_render_width(10, || command_wrap_width_for_title("Ran")),
            10
        );
        // 正常宽度：扣除标题列后仍是可用列数
        let available = with_render_width(80, || 80 - command_body_column("Ran"));
        assert_eq!(
            with_render_width(80, || command_wrap_width_for_title("Ran")),
            available
        );
    }

    /// 宽终端上，长度未超出可用列数的命令不应被折行。
    #[test]
    fn wide_terminals_keep_commands_on_one_line() {
        let command = format!("cargo test --workspace {}", "-".repeat(60));
        let lines = with_render_width(120, || {
            wrap_display_lines(&command, command_wrap_width_for_title("Ran"))
        });

        assert_eq!(lines.len(), 1, "命令未超出可用宽度却被折行: {lines:?}");
    }

    #[test]
    fn wraps_long_line_by_display_width() {
        let lines = wrap_display_lines(&"字".repeat(30), 10);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| {
            let w: usize = l
                .chars()
                .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
                .sum();
            w <= 10
        }));
    }

    #[test]
    fn folds_middle_when_too_many_display_lines() {
        let lines: Vec<String> = (1..=20).map(|n| format!("line{n}")).collect();
        let visible = fold_display_lines(&lines, 2, 4, false);
        assert!(visible
            .iter()
            .any(|line| matches!(line, FoldedDisplayLine::Omitted { omitted: 14, .. })));
        for text in ["line1", "line2", "line20"] {
            assert!(visible.contains(&FoldedDisplayLine::Line(text.to_string())));
        }
        assert!(!visible.contains(&FoldedDisplayLine::Line("line10".to_string())));
    }

    /// 正文中的旧占位字符串必须按普通文本保留。
    #[test]
    fn literal_omission_token_remains_content() {
        let visible = fold_display_lines(&["__OMITTED__".to_string()], 2, 4, false);
        assert_eq!(visible, vec![FoldedDisplayLine::Line("__OMITTED__".into())]);
    }

    /// 全部省略时只留下折叠占位。
    #[test]
    fn hidden_preview_omits_every_line() {
        let previous = fold_preview_bounds();
        configure_fold_preview(&DisplayConfig {
            fold_preview: FoldPreviewMode::Hidden,
            ..DisplayConfig::default()
        });
        let lines: Vec<String> = (1..=6).map(|n| format!("line{n}")).collect();
        let visible = fold_preview_lines(&lines, false);
        assert_eq!(
            visible,
            vec![FoldedDisplayLine::Omitted {
                omitted: 6,
                skipped: lines.clone()
            }]
        );
        assert_eq!(fold_preview_lines(&lines, true).len(), 6);
        restore_preview(previous);
    }

    /// 只保留开头时不显示尾部行。
    #[test]
    fn head_preview_keeps_only_leading_lines() {
        let previous = fold_preview_bounds();
        configure_fold_preview(&DisplayConfig {
            fold_preview: FoldPreviewMode::Head,
            fold_head_lines: 2,
            ..DisplayConfig::default()
        });
        let lines: Vec<String> = (1..=8).map(|n| format!("line{n}")).collect();
        let visible = fold_preview_lines(&lines, false);
        assert!(visible.contains(&FoldedDisplayLine::Line("line1".into())));
        assert!(visible.contains(&FoldedDisplayLine::Line("line2".into())));
        assert!(!visible.contains(&FoldedDisplayLine::Line("line8".into())));
        assert!(visible
            .iter()
            .any(|line| matches!(line, FoldedDisplayLine::Omitted { omitted: 6, .. })));
        restore_preview(previous);
    }

    /// 把测试线程的折叠偏好恢复为用例开始时的值。
    fn restore_preview(bounds: FoldPreviewBounds) {
        let mode = if bounds.omit_all {
            FoldPreviewMode::Hidden
        } else if bounds.tail == 0 {
            FoldPreviewMode::Head
        } else {
            FoldPreviewMode::Ends
        };
        configure_fold_preview(&DisplayConfig {
            fold_preview: mode,
            fold_head_lines: bounds.head.max(1),
            fold_tail_lines: bounds.tail,
            ..DisplayConfig::default()
        });
    }
}

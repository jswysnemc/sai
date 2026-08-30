use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// 折叠预览：默认保留前 2 行与后 4 行。
pub(crate) const FOLD_HEAD_LINES: usize = 2;
pub(crate) const FOLD_TAIL_LINES: usize = 4;
/// 兼容旧名：对称折叠时取 head（优先使用 FOLD_HEAD/TAIL）。
#[allow(dead_code)]
pub(crate) const FOLD_PREVIEW_LINES: usize = FOLD_HEAD_LINES;

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
/// - `(可见行, 省略行数)`；省略处用 `__OMITTED__` 占位
pub(crate) fn fold_display_lines(
    lines: &[String],
    head: usize,
    tail: usize,
    expanded: bool,
) -> (Vec<String>, usize) {
    // 展开渲染上下文（备用屏回看）：全部折叠块按展开输出
    let expanded = expanded || crate::render::render_expand::expand_override();
    let keep = head.saturating_add(tail);
    if expanded || keep == 0 || lines.len() <= keep {
        return (lines.to_vec(), 0);
    }
    let omitted = lines.len() - keep;
    let mut visible = Vec::with_capacity(keep + 1);
    visible.extend_from_slice(&lines[..head.min(lines.len())]);
    visible.push("__OMITTED__".to_string());
    let tail_start = lines.len().saturating_sub(tail);
    visible.extend_from_slice(&lines[tail_start..]);
    (visible, omitted)
}

/// 折行折叠后的单个显示条目。
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
    let expanded = expanded || crate::render::render_expand::expand_override();
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
        assert_eq!(with_render_width(10, || command_wrap_width_for_title("Ran")), 10);
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
        let (visible, omitted) = fold_display_lines(&lines, 2, 4, false);
        assert_eq!(omitted, 14);
        assert!(visible.iter().any(|l| l == "__OMITTED__"));
        assert!(visible.contains(&"line1".to_string()));
        assert!(visible.contains(&"line2".to_string()));
        assert!(visible.contains(&"line20".to_string()));
        assert!(!visible.iter().any(|l| l == "line10"));
    }
}

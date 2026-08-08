use unicode_width::UnicodeWidthChar;

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
        for ch in raw.chars() {
            let ch_w = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
            if current_width > 0 && current_width.saturating_add(ch_w) > width {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            current.push(ch);
            current_width = current_width.saturating_add(ch_w);
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

/// 命令预览行首前缀占用的列数。
///
/// 首行为 `$ ` 加行首引导，续行缩进四列，取其中较大者留出余量。
const COMMAND_PREFIX_COLUMNS: usize = 6;

/// 命令预览折行后至少保留的列数。
///
/// 终端极窄时前缀几乎吃掉整行，仍需留出足以看清片段的宽度。
const COMMAND_MIN_WRAP: usize = 24;

/// 计算命令预览的折行宽度。
///
/// 早先这里额外压了一道 72 列上限，于是终端再宽命令也只用到 72 列，
/// 右侧大片空白闲置而命令被提前折行。宽度只应受终端实际列数约束。
///
/// 返回:
/// - 命令预览可用的折行列数
pub(crate) fn command_wrap_width() -> usize {
    terminal_wrap_width()
        .saturating_sub(COMMAND_PREFIX_COLUMNS)
        .max(COMMAND_MIN_WRAP)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::render_width::with_render_width;

    /// 命令折行宽度跟随终端实际列数，不再压在固定上限上。
    ///
    /// 早先这里额外取 72 列的下界，宽终端上命令会在右侧仍有大片空白时折行。
    #[test]
    fn command_wrap_width_follows_the_terminal() {
        assert_eq!(with_render_width(120, command_wrap_width), 114);
        assert_eq!(with_render_width(200, command_wrap_width), 194);
    }

    /// 终端极窄时仍保留可读的最小宽度。
    #[test]
    fn command_wrap_width_keeps_a_readable_minimum() {
        assert_eq!(with_render_width(10, command_wrap_width), COMMAND_MIN_WRAP);
    }

    /// 宽终端上，长度未超出可用列数的命令不应被折行。
    #[test]
    fn wide_terminals_keep_commands_on_one_line() {
        let command = format!("cargo test --workspace {}", "-".repeat(60));
        let lines = with_render_width(120, || {
            wrap_display_lines(&command, command_wrap_width())
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

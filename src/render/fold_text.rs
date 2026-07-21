use unicode_width::UnicodeWidthChar;

/// 折叠预览时首尾各保留的显示行数。
pub(crate) const FOLD_PREVIEW_LINES: usize = 5;

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
/// - `preview`: 首尾各保留行数
/// - `expanded`: 是否展开
///
/// 返回:
/// - `(可见行, 省略行数)`；省略处用 `__OMITTED__` 占位
pub(crate) fn fold_display_lines(
    lines: &[String],
    preview: usize,
    expanded: bool,
) -> (Vec<String>, usize) {
    if expanded || lines.len() <= preview.saturating_mul(2) {
        return (lines.to_vec(), 0);
    }
    let omitted = lines.len() - preview * 2;
    let mut visible = Vec::with_capacity(preview * 2 + 1);
    visible.extend_from_slice(&lines[..preview]);
    visible.push("__OMITTED__".to_string());
    visible.extend_from_slice(&lines[lines.len() - preview..]);
    (visible, omitted)
}

/// 查询当前终端列宽，失败时回退 96。
///
/// 返回:
/// - 可用列宽
pub(crate) fn terminal_wrap_width() -> usize {
    crossterm::terminal::size()
        .map(|(cols, _)| cols as usize)
        .unwrap_or(96)
        .max(8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_long_line_by_display_width() {
        let lines = wrap_display_lines(&"字".repeat(30), 10);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| {
            let w: usize = l.chars().map(|c| UnicodeWidthChar::width(c).unwrap_or(0)).sum();
            w <= 10
        }));
    }

    #[test]
    fn folds_middle_when_too_many_display_lines() {
        let lines: Vec<String> = (1..=20).map(|n| format!("line{n}")).collect();
        let (visible, omitted) = fold_display_lines(&lines, 5, false);
        assert_eq!(omitted, 10);
        assert!(visible.iter().any(|l| l == "__OMITTED__"));
        assert!(visible.contains(&"line1".to_string()));
        assert!(visible.contains(&"line20".to_string()));
        assert!(!visible.iter().any(|l| l == "line10"));
    }
}

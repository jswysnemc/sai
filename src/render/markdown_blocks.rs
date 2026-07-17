use crossterm::terminal;

const HORIZONTAL_RULE_WIDTH: usize = 100;

/// 渲染 Markdown 水平分隔线。
///
/// 返回:
/// - 当前终端 100% 宽度的分隔线文本
pub(crate) fn horizontal_rule() -> String {
    format!("\x1b[2m{}\x1b[0m", "─".repeat(horizontal_rule_width()))
}

/// 判断是否为水平分隔线。
///
/// 参数:
/// - `line`: 去除缩进后的行
///
/// 返回:
/// - 是否为水平分隔线
pub(crate) fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= 3 && trimmed.chars().all(|ch| ch == '-')
}

/// 计算水平分隔线宽度。
///
/// 返回:
/// - 当前终端列宽，无法读取时回退到 100
pub(crate) fn horizontal_rule_width() -> usize {
    terminal::size()
        .map(|(width, _)| usize::from(width))
        .unwrap_or(HORIZONTAL_RULE_WIDTH)
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::table::visible_width;

    #[test]
    fn horizontal_rule_uses_full_terminal_width() {
        assert_eq!(visible_width(&horizontal_rule()), horizontal_rule_width());
    }
}

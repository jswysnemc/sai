use crate::render::content_indent::CONTENT_LEFT_INDENT;
use crossterm::terminal;

const HORIZONTAL_RULE_WIDTH: usize = 100;
/// MD 水平线相对通栏 turn 线左右各内收的列数。
pub(crate) const MARKDOWN_HR_SIDE_INSET: usize = 1;

/// 渲染 Markdown 水平分隔线。
///
/// 在当前渲染列宽内左右各内收一点，不预埋引导缩进：transcript 的
/// `assistant_body` / CLI 的 `align_to_guide_column` 会再补引导区。
/// 引导缩进 + 缩短的横线，使 MD 线相对通栏 turn 线左右都留白。
///
/// 返回:
/// - 弱化、相对正文列左右内收的分隔线（尚未加引导缩进）
pub(crate) fn horizontal_rule() -> String {
    let full = horizontal_rule_width();
    // transcript 注入的是正文净宽；无注入时 full 为终端通栏，需先扣引导区
    let column = if crate::render::render_width::render_width_override().is_some() {
        full
    } else {
        full.saturating_sub(CONTENT_LEFT_INDENT).max(1)
    };
    let inset = MARKDOWN_HR_SIDE_INSET.min(column.saturating_sub(1) / 2);
    // 左右各收 inset：最终经引导缩进后相对通栏 turn 线对称留白
    let dashes = column.saturating_sub(inset.saturating_mul(2)).max(1);
    format!("\x1b[2m{}\x1b[0m", "─".repeat(dashes))
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

/// 计算水平分隔线可用的终端列宽（通栏基准，MD 线会再扣引导区与内收）。
///
/// 返回:
/// - 当前渲染宽度（渲染上下文注入值优先，失败时回退终端查询）
pub(crate) fn horizontal_rule_width() -> usize {
    if let Some(width) = crate::render::render_width::render_width_override() {
        return width;
    }
    terminal::size()
        .map(|(width, _)| usize::from(width))
        .unwrap_or(HORIZONTAL_RULE_WIDTH)
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test;
    use crate::render::content_indent::align_to_guide_column;
    use crate::render::table::visible_width;

    #[test]
    fn markdown_horizontal_rule_insets_inside_guide_column() {
        let rule = horizontal_rule();
        let plain = strip_ansi_for_test(&rule);
        let column = horizontal_rule_width()
            .saturating_sub(CONTENT_LEFT_INDENT)
            .max(1);
        let inset = MARKDOWN_HR_SIDE_INSET.min(column.saturating_sub(1) / 2);
        let dashes = column.saturating_sub(inset.saturating_mul(2)).max(1);
        assert!(
            plain.starts_with('─') && !plain.starts_with(' '),
            "raw MD hr is dash-only; guide indent comes later: {plain:?}"
        );
        assert_eq!(plain.chars().count(), dashes);
        assert_eq!(visible_width(&rule), dashes);
        let aligned = align_to_guide_column(&rule);
        let aligned_plain = strip_ansi_for_test(&aligned);
        assert!(
            aligned_plain.starts_with("  ─"),
            "align must place MD hr past the guide column: {aligned_plain:?}"
        );
        assert_eq!(
            visible_width(&aligned),
            CONTENT_LEFT_INDENT + dashes,
            "guide + inset dashes"
        );
        assert!(
            visible_width(&aligned) + inset <= horizontal_rule_width(),
            "MD hr must leave right inset vs full width"
        );
    }
}

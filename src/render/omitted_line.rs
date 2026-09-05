use crate::render::terminal_text as t;

/// 所有可展开内容共用的折叠符号。
pub(crate) const FOLD_MARKER: &str = "▸";
pub(crate) const EXPANDED_MARKER: &str = "▾";

/// 生成统一的折叠提示，供会话正文、输入框与底部面板共用。
///
/// 参数: `description` 为被隐藏内容的说明，`shortcut` 为可选展开快捷键
/// 返回: 无缩进的 ANSI 提示
pub(crate) fn render_fold_hint(description: &str, shortcut: Option<&str>) -> String {
    let hint = shortcut.map(|key| format!(" · {key}")).unwrap_or_default();
    format!("\x1b[2m\x1b[36m{FOLD_MARKER} {description}{hint}\x1b[0m")
}

/// 折叠省略行的统一渲染样式。
///
/// 全部折叠块（命令输出、diff、思考、粘贴回显）共用同一种省略行：
/// 折叠提示单独占一行，位于正文列，使用统一符号、行数与展开提示。
///
/// 参数:
/// - `omitted`: 被省略的显示行数
/// - `show_hint`: 是否显示 Ctrl+O 快捷键提示
///
/// 返回:
/// - 统一样式的省略行 ANSI 文本
pub(crate) fn render_omitted_line(omitted: usize, show_hint: bool) -> String {
    format!("  {}", render_omitted_line_plain(omitted, show_hint))
}

/// 无缩进变体：用于正文已经自带前缀的场景。
///
/// 参数:
/// - `omitted`: 被省略的显示行数
/// - `show_hint`: 是否显示 Ctrl+O 快捷键提示
///
/// 返回:
/// - 不带 gutter 前缀的省略行 ANSI 文本
pub(crate) fn render_omitted_line_plain(omitted: usize, show_hint: bool) -> String {
    let unit = if omitted == 1 {
        t("line hidden", "行已折叠")
    } else {
        t("lines hidden", "行已折叠")
    };
    render_fold_hint(&format!("{omitted} {unit}"), show_hint.then_some("Ctrl+O"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test;

    /// 统一省略行包含行数与展开提示，纯文本不含 ANSI。
    #[test]
    fn omitted_line_contains_count_and_hint() {
        let line = render_omitted_line(6, true);
        let plain = strip_ansi_for_test(&line);
        assert!(plain.contains("▸ 6 lines hidden"), "{plain}");
        assert!(plain.contains("Ctrl+O"), "{plain}");
        assert!(plain.starts_with("  ▸ "), "{plain}");
    }

    /// 关闭提示时不出现 Ctrl+O。
    #[test]
    fn omitted_line_without_hint_omits_shortcut() {
        let plain = strip_ansi_for_test(&render_omitted_line(6, false));
        assert!(plain.contains("▸ 6 lines hidden"));
        assert!(!plain.contains("Ctrl+O"));
    }
}

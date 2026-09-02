use crate::i18n::text as t;

/// 折叠省略行的统一渲染样式。
///
/// 全部折叠块（命令输出、diff、思考、粘贴回显）共用同一种省略行：
/// `  └ … +N 行 (Ctrl+O 展开)`，颜色为 dim 青色，与折叠正文弱化视觉一致。
///
/// 参数:
/// - `omitted`: 被省略的显示行数
/// - `show_hint`: 是否显示 Ctrl+O 快捷键提示
///
/// 返回:
/// - 统一样式的省略行 ANSI 文本
pub(crate) fn render_omitted_line(omitted: usize, show_hint: bool) -> String {
    let hint = if show_hint {
        format!(" (Ctrl+O {})", t("to expand", "展开"))
    } else {
        String::new()
    };
    format!(
        "\x1b[2m\x1b[36m  └ … +{omitted} {}{hint}\x1b[0m",
        t("lines", "行")
    )
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
    let hint = if show_hint {
        format!(" (Ctrl+O {})", t("to expand", "展开"))
    } else {
        String::new()
    };
    format!(
        "\x1b[2m\x1b[36m… +{omitted} {}{hint}\x1b[0m",
        t("lines", "行")
    )
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
        assert!(plain.contains("… +6"), "{plain}");
        assert!(plain.contains("Ctrl+O"), "{plain}");
        assert!(plain.starts_with("  └ "), "{plain}");
    }

    /// 关闭提示时不出现 Ctrl+O。
    #[test]
    fn omitted_line_without_hint_omits_shortcut() {
        let plain = strip_ansi_for_test(&render_omitted_line(6, false));
        assert!(plain.contains("… +6"));
        assert!(!plain.contains("Ctrl+O"));
    }
}

use crate::render::terminal_text as t;
use std::cell::Cell;

/// 所有可展开内容共用的折叠符号。
pub(crate) const FOLD_MARKER: &str = "▸";
pub(crate) const EXPANDED_MARKER: &str = "▾";
/// 内联视图中展开折叠块的快捷键。
pub(crate) const FOLD_SHORTCUT: &str = "Ctrl+O";

thread_local! {
    /// 当前是否在全屏会话视图中渲染：此时 Ctrl+O 是退出键，展开靠点击
    static FULLSCREEN_HINTS: Cell<bool> = const { Cell::new(false) };
}

/// 【折叠提示】【全屏】在全屏视图的渲染上下文中执行闭包，折叠提示改为点击展开。
///
/// 参数:
/// - `render`: 渲染闭包
///
/// 返回:
/// - 闭包返回值
pub(crate) fn with_fullscreen_hints<T>(render: impl FnOnce() -> T) -> T {
    FULLSCREEN_HINTS.with(|cell| {
        let previous = cell.replace(true);
        let result = render();
        cell.set(previous);
        result
    })
}

/// 当前上下文下展开折叠块的提示文字。
///
/// 返回:
/// - 内联视图为 Ctrl+O，全屏视图为点击展开
pub(crate) fn fold_shortcut() -> &'static str {
    if FULLSCREEN_HINTS.with(Cell::get) {
        t("click to expand", "点击展开")
    } else {
        FOLD_SHORTCUT
    }
}

/// 生成统一的折叠提示，供会话正文、输入框与底部面板共用。
///
/// 参数: `description` 为被隐藏内容的说明，`shortcut` 为可选展开快捷键
/// 返回: 无缩进的 ANSI 提示
pub(crate) fn render_fold_hint(description: &str, shortcut: Option<&str>) -> String {
    // 全屏视图中 Ctrl+O 用于退出，展开提示一律换成点击
    let shortcut = shortcut.map(|key| {
        if key == FOLD_SHORTCUT {
            fold_shortcut()
        } else {
            key
        }
    });
    let hint = shortcut.map(|key| format!(" · {key}")).unwrap_or_default();
    format!("\x1b[2m\x1b[36m{FOLD_MARKER} {description}{hint}\x1b[0m")
}

/// 把已缓存行里的 Ctrl+O 折叠提示改写为全屏视图的点击提示。
///
/// 渲染缓存按宽度与选项复用内联视图的行，缓存行里的提示仍是 Ctrl+O，
/// 这里只替换 [`render_fold_hint`] 生成的提示尾部，不触碰正文里的其它文字。
///
/// 参数:
/// - `line`: 已渲染的 ANSI 行
///
/// 返回:
/// - 改写后的行；不含折叠提示时原样返回
pub(crate) fn rewrite_fold_hint_for_fullscreen(line: &str) -> Option<String> {
    let inline = format!(" · {FOLD_SHORTCUT}\x1b[0m");
    if !line.contains(FOLD_MARKER) || !line.contains(&inline) {
        return None;
    }
    let fullscreen = format!(" · {}\x1b[0m", t("click to expand", "点击展开"));
    Some(line.replace(&inline, &fullscreen))
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
    render_fold_hint(
        &format!("{omitted} {unit}"),
        show_hint.then_some(FOLD_SHORTCUT),
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

    /// 全屏上下文中折叠提示改为点击展开，退出上下文后恢复 Ctrl+O。
    #[test]
    fn fullscreen_context_replaces_shortcut_with_click_hint() {
        let inside = with_fullscreen_hints(|| strip_ansi_for_test(&render_omitted_line(6, true)));
        assert!(!inside.contains("Ctrl+O"), "{inside}");
        assert!(
            inside.contains("click to expand") || inside.contains("点击展开"),
            "{inside}"
        );
        assert!(strip_ansi_for_test(&render_omitted_line(6, true)).contains("Ctrl+O"));
    }

    /// 缓存行只改写折叠提示尾部，正文里提到 Ctrl+O 的文字保持不变。
    #[test]
    fn cached_fold_hint_is_rewritten_in_place() {
        let hint = render_omitted_line(3, true);
        let rewritten = rewrite_fold_hint_for_fullscreen(&hint).expect("应识别折叠提示");
        assert!(!strip_ansi_for_test(&rewritten).contains("Ctrl+O"));
        assert!(rewrite_fold_hint_for_fullscreen("press Ctrl+O to open").is_none());
    }
}

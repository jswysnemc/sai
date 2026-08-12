/// 配置界面的边框与文本配色。
///
/// 与 TUI 会话界面共用同一套色板基调：品牌绿松用于标题与主按钮，
/// 亮青用于选中与键位提示，弱化灰蓝用于边框与说明。
/// 品牌色（与 Web --signal 同色）
pub(super) const BRAND: &str = "\x1b[38;2;58;114;100m";
/// 选中项高亮色
pub(super) const ACCENT: &str = "\x1b[38;2;190;246;255m";
/// 边框与提示的弱化色
pub(super) const MUTED: &str = "\x1b[38;2;77;116;125m";
/// 次级弱化色（分隔点、占位说明）
pub(super) const DIM: &str = "\x1b[38;2;58;82;90m";
/// 字段值的强调色：值比标签更值得被看到
pub(super) const VALUE: &str = "\x1b[38;2;152;216;200m";
/// 开启 / 成功状态
pub(super) const OK: &str = "\x1b[38;2;140;196;116m";
/// 关闭 / 中性状态沿用 MUTED；危险状态色
pub(super) const DANGER: &str = "\x1b[38;2;224;120;125m";
/// 选中行整行底色（深青灰，抬升一档）
pub(super) const SELECT_BG: &str = "\x1b[48;2;33;46;51m";
/// 主按钮实心底（品牌绿松）
pub(super) const BUTTON_BG: &str = "\x1b[48;2;58;114;100m";
/// 主按钮前景（近白）
pub(super) const BUTTON_FG: &str = "\x1b[38;2;226;240;236m";
/// 样式复位
pub(super) const RESET: &str = "\x1b[0m";
/// 加粗
pub(super) const BOLD: &str = "\x1b[1m";

/// 圆角边框字符：左上、右上、左下、右下、横线、竖线
pub(super) const CORNER_TOP_LEFT: char = '╭';
pub(super) const CORNER_TOP_RIGHT: char = '╮';
pub(super) const CORNER_BOTTOM_LEFT: char = '╰';
pub(super) const CORNER_BOTTOM_RIGHT: char = '╯';
pub(super) const LINE_HORIZONTAL: char = '─';
pub(super) const LINE_VERTICAL: char = '│';

/// 选中项左侧的指示条
pub(super) const SELECTION_BAR: char = '▏';

/// 【配置界面】【视觉】构造选中项的样式前缀。
///
/// 选中态 = 亮青竖条 + 整行深底 + 亮青文字；未选中恢复弱化前景。
/// 整行深底让选中行在长列表里可被余光捕获，竖条标明锚点。
///
/// 参数:
/// - `selected`: 是否为当前选中项
///
/// 返回:
/// - `(左侧指示条, 文本样式前缀)`
pub(super) fn selection_marks(selected: bool) -> (String, &'static str) {
    if selected {
        (
            format!("{SELECT_BG}{ACCENT}{SELECTION_BAR}{RESET}"),
            SELECTED_TEXT_STYLE,
        )
    } else {
        (" ".to_string(), MUTED)
    }
}

/// 选中行文本样式：深底 + 亮青。
const SELECTED_TEXT_STYLE: &str = "\x1b[48;2;33;46;51m\x1b[38;2;190;246;255m";

/// 【配置界面】【视觉】渲染分段式快捷键帮助条。
///
/// 键名亮青、说明弱化、分段间以中点分隔，替代整行灰字的提示。
///
/// 参数:
/// - `pairs`: `(键名, 动作说明)` 序列
///
/// 返回:
/// - 单行 ANSI 帮助文本
pub(super) fn help_line(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, description)| format!("{ACCENT}{key}{RESET} {MUTED}{description}{RESET}"))
        .collect::<Vec<_>>()
        .join(&format!("{DIM} · {RESET}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【配置界面】【视觉】验证选中与未选中的标记可区分且宽度一致。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn selection_marks_differ_without_changing_width() {
        let (selected_bar, selected_style) = selection_marks(true);
        let (plain_bar, plain_style) = selection_marks(false);

        assert_ne!(selected_style, plain_style);
        assert!(selected_bar.contains(SELECTION_BAR));
        assert!(!plain_bar.contains(SELECTION_BAR));
        // 两者可见宽度都是一列，切换选中时正文不会左右跳动
        assert_eq!(visible_width(&selected_bar), 1);
        assert_eq!(visible_width(&plain_bar), 1);
    }

    /// 选中样式带整行底色，未选中不带。
    #[test]
    fn selected_style_carries_row_background() {
        let (_, selected_style) = selection_marks(true);
        let (_, plain_style) = selection_marks(false);

        assert!(selected_style.contains("\x1b[48;2;"));
        assert!(!plain_style.contains("\x1b[48;2;"));
    }

    /// 帮助条按键名 / 说明分色，段间带分隔点。
    #[test]
    fn help_line_styles_keys_and_descriptions() {
        let line = help_line(&[("↑↓", "移动"), ("Enter", "打开")]);

        assert!(line.contains(&format!("{ACCENT}↑↓{RESET}")));
        assert!(line.contains(&format!("{MUTED}移动{RESET}")));
        assert!(line.contains('·'));
    }

    /// 计算去除 ANSI 后的显示宽度。
    ///
    /// 参数:
    /// - `text`: 带样式文本
    ///
    /// 返回:
    /// - 可见宽度
    fn visible_width(text: &str) -> usize {
        let mut width = 0usize;
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\x1b' && chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
                continue;
            }
            width += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        }
        width
    }
}

/// 配置界面的边框与文本配色。
///
/// 与 TUI 会话界面共用同一套色板：品牌绿松用于标题与选中态，
/// 弱化灰蓝用于边框与提示，避免配置界面自成一套观感。
/// 品牌色（与 Web --signal 同色）
pub(super) const BRAND: &str = "\x1b[38;2;58;114;100m";
/// 选中项高亮色
pub(super) const ACCENT: &str = "\x1b[38;2;190;246;255m";
/// 边框与提示的弱化色
pub(super) const MUTED: &str = "\x1b[38;2;77;116;125m";
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
/// 使用左侧指示条加高亮文本，而不是整行反色——反色在浅色终端下会形成
/// 大块色斑，且与会话界面的选中语汇不一致。
///
/// 参数:
/// - `selected`: 是否为当前选中项
///
/// 返回:
/// - `(左侧指示条, 文本样式前缀)`
pub(super) fn selection_marks(selected: bool) -> (String, &'static str) {
    if selected {
        (format!("{ACCENT}{SELECTION_BAR}{RESET}"), ACCENT)
    } else {
        (" ".to_string(), MUTED)
    }
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

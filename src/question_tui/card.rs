use super::text::{strip_ansi, truncate_width};
use unicode_width::UnicodeWidthStr;

const EDGE: &str = "\x1b[2;37m";
const RESET: &str = "\x1b[0m";

/// 【终端提问】【卡片尺寸】宽屏保持克制宽度，小终端退回无边框布局。
#[derive(Clone, Copy)]
pub(super) struct CardLayout {
    pub(super) width: usize,
    pub(super) framed: bool,
}

impl CardLayout {
    /// 【终端提问】【尺寸计算】参数为终端宽高，返回卡片布局。
    pub(super) fn new(cols: usize, rows: usize) -> Self {
        Self {
            width: cols.min(88),
            framed: cols >= 28 && rows >= 7,
        }
    }

    /// 【终端提问】【正文宽度】无参数，返回扣除边框与内边距的列数。
    pub(super) fn content_width(self) -> usize {
        self.width
            .saturating_sub(if self.framed { 4 } else { 2 })
            .max(1)
    }

    /// 【终端提问】【标题边框】参数为标题文本，返回对称细边框标题行。
    pub(super) fn heading(self, title: &str) -> String {
        let title = truncate_width(title, self.width.saturating_sub(6));
        let remaining = self.width.saturating_sub(visible(&title) + 5);
        format!(
            "{RESET}{EDGE}╭─{RESET} {title} {EDGE}{}╮{RESET}",
            "─".repeat(remaining)
        )
    }

    /// 【终端提问】【正文边框】参数为带样式正文，返回默认背景下的完整卡片行。
    pub(super) fn row(self, text: &str) -> String {
        let text = truncate_width(text, self.content_width());
        if !self.framed {
            return truncate_width(&format!("{RESET}  {text}{RESET}"), self.width);
        }
        let padding = self.content_width().saturating_sub(visible(&text));
        format!(
            "{RESET}{EDGE}│{RESET} {text}{RESET}{} {EDGE}│{RESET}",
            " ".repeat(padding)
        )
    }

    /// 【终端提问】【横向边框】参数为是否底边，返回同色细分隔线或圆角底边。
    pub(super) fn rule(self, bottom: bool) -> String {
        let (left, right) = if bottom {
            ('╰', '╯')
        } else {
            ('├', '┤')
        };
        format!(
            "{RESET}{EDGE}{left}{}{right}{RESET}",
            "─".repeat(self.width.saturating_sub(2))
        )
    }
}

/// 【终端提问】【可见宽度】参数为 ANSI 文本，返回终端显示列数。
fn visible(text: &str) -> usize {
    UnicodeWidthStr::width(strip_ansi(text).as_str())
}

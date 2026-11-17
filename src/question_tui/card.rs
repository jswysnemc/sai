use super::text::{strip_ansi, truncate_width};
use crate::render::panel_palette::{fg, PanelPalette};
use unicode_width::UnicodeWidthStr;

const RESET: &str = "\x1b[0m";

/// 【终端提问】【卡片尺寸】宽屏保持克制宽度，小终端退回无边框布局，边框色随终端明暗自适应。
#[derive(Clone, Copy)]
pub(super) struct CardLayout {
    pub(super) width: usize,
    pub(super) framed: bool,
    pub(super) palette: PanelPalette,
}

impl CardLayout {
    /// 【终端提问】【尺寸计算】按终端宽高与当前明暗主题创建卡片布局。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    /// - `rows`: 面板可用行数
    ///
    /// 返回:
    /// - 卡片布局实例
    pub(super) fn new(cols: usize, rows: usize) -> Self {
        Self::with_palette(cols, rows, PanelPalette::current())
    }

    /// 【终端提问】【指定主题】按给定调色板创建卡片布局，便于明暗背景单元测试。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    /// - `rows`: 面板可用行数
    /// - `palette`: 面板调色板
    ///
    /// 返回:
    /// - 卡片布局实例
    pub(super) fn with_palette(cols: usize, rows: usize, palette: PanelPalette) -> Self {
        Self {
            width: cols.min(88),
            framed: cols >= 28 && rows >= 7,
            palette,
        }
    }

    /// 【终端提问】【正文宽度】返回扣除边框与内边距后的内容列数。
    ///
    /// 参数:
    /// - `self`: 当前布局
    ///
    /// 返回:
    /// - 正文可用列宽
    pub(super) fn content_width(self) -> usize {
        self.width
            .saturating_sub(if self.framed { 4 } else { 2 })
            .max(1)
    }

    /// 【终端提问】【标题边框】生成对称圆角细边框标题行。
    ///
    /// 参数:
    /// - `title`: 标题文本
    ///
    /// 返回:
    /// - 带边框的标题行
    pub(super) fn heading(self, title: &str) -> String {
        let edge = fg(self.palette.subtle);
        let title = truncate_width(title, self.width.saturating_sub(6));
        let remaining = self.width.saturating_sub(visible(&title) + 5);
        format!(
            "{RESET}{edge}╭─{RESET} {title} {edge}{}╮{RESET}",
            "─".repeat(remaining)
        )
    }

    /// 【终端提问】【正文边框】生成默认背景下的完整卡片内容行。
    ///
    /// 参数:
    /// - `text`: 带样式正文
    ///
    /// 返回:
    /// - 补齐宽度后的卡片行
    pub(super) fn row(self, text: &str) -> String {
        let edge = fg(self.palette.subtle);
        let text = truncate_width(text, self.content_width());
        if !self.framed {
            return truncate_width(&format!("{RESET}  {text}{RESET}"), self.width);
        }
        let padding = self.content_width().saturating_sub(visible(&text));
        format!(
            "{RESET}{edge}│{RESET} {text}{RESET}{} {edge}│{RESET}",
            " ".repeat(padding)
        )
    }

    /// 【终端提问】【横向边框】生成同色系细分隔线或圆角底边。
    ///
    /// 参数:
    /// - `bottom`: 是否为底部收口边框
    ///
    /// 返回:
    /// - 边框行字符串
    pub(super) fn rule(self, bottom: bool) -> String {
        let edge = fg(self.palette.subtle);
        let (left, right) = if bottom {
            ('╰', '╯')
        } else {
            ('├', '┤')
        };
        format!(
            "{RESET}{edge}{left}{}{right}{RESET}",
            "─".repeat(self.width.saturating_sub(2))
        )
    }
}

/// 【终端提问】【可见宽度】计算去除 ANSI 控制码后的终端显示列数。
///
/// 参数:
/// - `text`: 含样式文本
///
/// 返回:
/// - 显示列数
fn visible(text: &str) -> usize {
    UnicodeWidthStr::width(strip_ansi(text).as_str())
}

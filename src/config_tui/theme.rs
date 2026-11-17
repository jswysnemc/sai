//! 配置界面的明暗终端自适应配色与行级视觉原语。

use crate::render::panel_palette::{PanelPalette, Rgb};

/// 样式复位序列。
pub(crate) const RESET: &str = "\x1b[0m";
/// 加粗序列。
pub(crate) const BOLD: &str = "\x1b[1m";

/// 圆角边框字符：左上、右上、左下、右下、横线、竖线。
pub(crate) const CORNER_TOP_LEFT: char = '╭';
pub(crate) const CORNER_TOP_RIGHT: char = '╮';
pub(crate) const CORNER_BOTTOM_LEFT: char = '╰';
pub(crate) const CORNER_BOTTOM_RIGHT: char = '╯';
pub(crate) const LINE_HORIZONTAL: char = '─';
pub(crate) const LINE_VERTICAL: char = '│';

/// 选中项左侧的异色竖条锚点。
pub(crate) const SELECTION_BAR: char = '▏';

/// 配置界面语义调色板，随终端明暗背景自动切换。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ThemePalette {
    /// 品牌绿松色（用于标题锚点与分组标识）
    pub brand: &'static str,
    /// 选中项与快捷键高亮色
    pub accent: &'static str,
    /// 说明文字与常规未选中项前景
    pub muted: &'static str,
    /// 边框、分隔符与占位提示色
    pub dim: &'static str,
    /// 表单字段值强调色
    pub value: &'static str,
    /// 开启与成功状态色
    pub ok: &'static str,
    /// 提醒与待保存状态色
    pub warn: &'static str,
    /// 删除与危险提示色
    pub danger: &'static str,
    /// 状态胶囊微底色
    pub pill_bg: &'static str,
    /// 选中行整行底色
    pub select_bg: &'static str,
    /// 鼠标悬停行连续柔和底色
    pub hover_bg: &'static str,
    /// 选中行组合样式（底色 + 高对比前景）
    pub selected_text: &'static str,
    /// 悬停行组合样式（柔和底色 + 清晰前景）
    pub hover_text: &'static str,
    /// 主按钮实心底色
    pub button_bg: &'static str,
    /// 主按钮文字前景色
    pub button_fg: &'static str,
}

/// 深色终端配色：保留品牌绿松与亮青质感。
const DARK_PALETTE: ThemePalette = ThemePalette {
    brand: "\x1b[38;2;94;196;168m",
    accent: "\x1b[38;2;170;232;246m",
    muted: "\x1b[38;2;160;180;186m",
    dim: "\x1b[38;2;118;138;146m",
    value: "\x1b[38;2;152;216;200m",
    ok: "\x1b[38;2;140;206;116m",
    warn: "\x1b[38;2;236;194;110m",
    danger: "\x1b[38;2;236;128;132m",
    pill_bg: "\x1b[48;2;32;54;60m",
    select_bg: "\x1b[48;2;36;62;70m",
    hover_bg: "\x1b[48;2;28;48;54m",
    selected_text: "\x1b[48;2;36;62;70m\x1b[38;2;190;246;255m",
    hover_text: "\x1b[48;2;28;48;54m\x1b[38;2;204;228;234m",
    button_bg: "\x1b[48;2;46;120;102m",
    button_fg: "\x1b[38;2;236;246;242m",
};

/// 浅色终端配色：使用清爽浅灰青底色与深色高对比前景，消除白底黑斑。
const LIGHT_PALETTE: ThemePalette = ThemePalette {
    brand: "\x1b[38;2;18;122;98m",
    accent: "\x1b[38;2;14;100;118m",
    muted: "\x1b[38;2;86;102;110m",
    dim: "\x1b[38;2;132;146;154m",
    value: "\x1b[38;2;20;112;92m",
    ok: "\x1b[38;2;32;124;62m",
    warn: "\x1b[38;2;168;102;18m",
    danger: "\x1b[38;2;176;48;60m",
    pill_bg: "\x1b[48;2;222;239;242m",
    select_bg: "\x1b[48;2;212;234;238m",
    hover_bg: "\x1b[48;2;232;244;246m",
    selected_text: "\x1b[48;2;212;234;238m\x1b[38;2;12;72;86m",
    hover_text: "\x1b[48;2;232;244;246m\x1b[38;2;28;56;64m",
    button_bg: "\x1b[48;2;24;126;104m",
    button_fg: "\x1b[38;2;246;252;250m",
};

impl ThemePalette {
    /// 【配置界面】【当前主题】通过共享 PanelPalette 感知终端明暗背景并返回对应主题色板。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 当前终端适用的静态主题调色板引用
    pub(crate) fn current() -> &'static Self {
        Self::from_panel(PanelPalette::current())
    }

    /// 【配置界面】【指定背景主题】按给定终端背景 RGB 计算对应的明暗主题色板。
    ///
    /// 参数:
    /// - `background`: 终端背景 RGB 三元组
    ///
    /// 返回:
    /// - 深色或浅色静态主题调色板引用
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn for_background(background: Rgb) -> &'static Self {
        Self::from_panel(PanelPalette::for_background(background))
    }

    /// 【配置界面】【面板映射】依据 PanelPalette 派生焦点底色亮度判定深浅色主题。
    ///
    /// 参数:
    /// - `panel`: 共享终端面板调色板
    ///
    /// 返回:
    /// - 匹配的静态主题调色板
    fn from_panel(panel: PanelPalette) -> &'static Self {
        let (r, g, b) = panel.focus_bg;
        let light = u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 > 128_000;
        if light {
            &LIGHT_PALETTE
        } else {
            &DARK_PALETTE
        }
    }

    /// 【配置界面】【行前缀构造】按当前色板生成列表项选中与悬停样式前缀。
    ///
    /// 参数:
    /// - `selected`: 是否为当前焦点选中项
    /// - `hovered`: 指针是否正悬停在该行
    ///
    /// 返回:
    /// - `(左侧单列指示标记, 行文本样式前缀)`
    pub(crate) fn row_marks(&self, selected: bool, hovered: bool) -> (String, &'static str) {
        if selected {
            // 1. 【配置界面】【主焦点锚点】选中行保留左侧异色竖条与整行选中底色
            (
                format!("{}{}{SELECTION_BAR}{RESET}", self.select_bg, self.accent),
                self.selected_text,
            )
        } else if hovered {
            // 2. 【配置界面】【悬停反馈】非选中悬停行输出带柔和悬停底色的单格空白，避免双竖条抢夺焦点
            (format!("{} {RESET}", self.hover_bg), self.hover_text)
        } else {
            (" ".to_string(), self.muted)
        }
    }
}

/// 【配置界面】【选中标记】构造不含鼠标悬停态的列表选中前缀。
///
/// 参数:
/// - `selected`: 是否为当前选中项
///
/// 返回:
/// - `(左侧指示条, 文本样式前缀)`
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn selection_marks(selected: bool) -> (String, &'static str) {
    row_marks(selected, false)
}

/// 【配置界面】【列表行标记】构造感知当前明暗主题的选中与悬停样式前缀。
///
/// 参数:
/// - `selected`: 是否为当前选中项
/// - `hovered`: 指针是否正悬停在该行
///
/// 返回:
/// - `(左侧指示条, 文本样式前缀)`
pub(crate) fn row_marks(selected: bool, hovered: bool) -> (String, &'static str) {
    ThemePalette::current().row_marks(selected, hovered)
}

/// 【配置界面】【连续底色】在行内每个样式复位序列后补回指定背景色，消除多色片段间的底色断层。
///
/// 参数:
/// - `text`: 含局部 ANSI 前景与复位序列的单行文本
/// - `bg_style`: 目标整行背景色序列
///
/// 返回:
/// - 背景连续贯穿且以复位序列收尾的文本
pub(crate) fn tint_bg(text: &str, bg_style: &str) -> String {
    if bg_style.is_empty() {
        return text.to_string();
    }
    let body = text.strip_suffix(RESET).unwrap_or(text);
    format!(
        "{bg_style}{}{RESET}",
        body.replace(RESET, &format!("{RESET}{bg_style}"))
    )
}

/// 【配置界面】【帮助分隔符】返回状态栏统一的快捷键分段分隔符。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 带弱化样式的中点分隔符文本
pub(crate) fn help_separator() -> String {
    let palette = ThemePalette::current();
    format!("{} · {RESET}", palette.dim)
}

/// 【配置界面】【帮助栏渲染】将快捷键与动作说明序列渲染为分段高亮帮助条。
///
/// 参数:
/// - `pairs`: `(键名, 动作说明)` 切片
///
/// 返回:
/// - 单行 ANSI 帮助文本
pub(crate) fn help_line(pairs: &[(&str, &str)]) -> String {
    let palette = ThemePalette::current();
    pairs
        .iter()
        .map(|(key, description)| {
            format!(
                "{}{key}{RESET} {}{description}{RESET}",
                palette.accent, palette.muted
            )
        })
        .collect::<Vec<_>>()
        .join(&help_separator())
}

/// 【配置界面】【状态胶囊】渲染带微底色的紧凑状态标签，适合顶栏摘要与保存指示。
///
/// 参数:
/// - `label`: 胶囊文字
/// - `fg_style`: 前景颜色转义串
/// - `bg_style`: 背景颜色转义串
///
/// 返回:
/// - 带样式复位序列的胶囊字符串
pub(crate) fn pill(label: &str, fg_style: &str, bg_style: &str) -> String {
    format!("{bg_style}{fg_style} {label} {RESET}")
}

/// 【配置界面】【保存标识】根据配置是否已修改生成统一状态的顶栏指示标签。
///
/// 参数:
/// - `dirty`: 是否存在未落盘修改
///
/// 返回:
/// - 已样式化的保存状态胶囊
pub(crate) fn save_state_pill(dirty: bool) -> String {
    let palette = ThemePalette::current();
    if dirty {
        pill(
            crate::i18n::text("unsaved", "待保存"),
            palette.warn,
            palette.pill_bg,
        )
    } else {
        pill(
            crate::i18n::text("saved", "已保存"),
            palette.ok,
            palette.pill_bg,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【配置界面】【选中标记测试】验证选中与悬停、普通行的标记宽度一致且仅选中态绘制异色竖条。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn selection_marks_differ_without_changing_width() {
        let (selected_bar, selected_style) = selection_marks(true);
        let (hover_bar, hover_style) = row_marks(false, true);
        let (plain_bar, plain_style) = selection_marks(false);

        assert_ne!(selected_style, plain_style);
        assert_ne!(hover_style, plain_style);
        assert!(selected_bar.contains(SELECTION_BAR));
        assert!(!hover_bar.contains(SELECTION_BAR));
        assert!(!plain_bar.contains(SELECTION_BAR));
        assert_eq!(visible_width(&selected_bar), 1);
        assert_eq!(visible_width(&hover_bar), 1);
        assert_eq!(visible_width(&plain_bar), 1);
    }

    /// 【配置界面】【行底色测试】验证选中态与悬停态携带背景色，未选中行不携带背景色。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn selected_style_carries_row_background() {
        let (_, selected_style) = selection_marks(true);
        let (_, hover_style) = row_marks(false, true);
        let (_, plain_style) = selection_marks(false);

        assert!(selected_style.contains("\x1b[48;2;"));
        assert!(hover_style.contains("\x1b[48;2;"));
        assert!(!plain_style.contains("\x1b[48;2;"));
    }

    /// 【配置界面】【明暗自适应测试】验证深浅色背景切换不同底色且连续底色函数补齐中间复位。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn palette_adapts_to_light_and_dark_backgrounds_and_tints_continuously() {
        let dark = ThemePalette::for_background((24, 28, 32));
        let light = ThemePalette::for_background((250, 250, 250));

        assert_ne!(dark.select_bg, light.select_bg);
        assert_ne!(dark.hover_bg, light.hover_bg);
        assert_ne!(dark.pill_bg, light.pill_bg);
        assert!(light.select_bg.contains("212;234;238"));

        let raw = format!("{}label{RESET}  {}value{RESET}", dark.muted, dark.value);
        let tinted = tint_bg(&raw, dark.hover_bg);
        let without_tail = tinted.strip_suffix(RESET).unwrap();
        assert!(!without_tail.contains(&format!("{RESET} ")));
        assert!(without_tail.contains(&format!("{RESET}{}  ", dark.hover_bg)));
    }

    /// 【配置界面】【帮助栏样式测试】验证帮助条按键名与说明分色且包含分隔符。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn help_line_styles_keys_and_descriptions() {
        let palette = ThemePalette::current();
        let line = help_line(&[("↑↓", "移动"), ("Enter", "打开")]);

        assert!(line.contains(&format!("{}↑↓{RESET}", palette.accent)));
        assert!(line.contains(&format!("{}移动{RESET}", palette.muted)));
        assert!(line.contains('·'));
    }

    /// 【配置界面】【测试辅助】计算去除 ANSI 转义序列后的可见列宽。
    ///
    /// 参数:
    /// - `text`: 带样式文本
    ///
    /// 返回:
    /// - 可见列宽
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

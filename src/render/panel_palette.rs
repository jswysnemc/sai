//! 侧栏、差异窗口与后台任务面板共用的终端明暗主题配色。

use crate::render::terminal_palette::terminal_palette;

pub(crate) type Rgb = (u8, u8, u8);

#[derive(Clone, Copy)]
pub(crate) struct PanelPalette {
    pub(crate) border: Rgb,
    pub(crate) text: Rgb,
    pub(crate) muted: Rgb,
    pub(crate) accent: Rgb,
    pub(crate) added: Rgb,
    pub(crate) removed: Rgb,
    pub(crate) added_bg: Rgb,
    pub(crate) removed_bg: Rgb,
    pub(crate) hover_bg: Rgb,
    pub(crate) focus_bg: Rgb,
}

impl PanelPalette {
    /// 【终端面板】【主题选择】读取缓存的终端背景，使用对应明暗主题。
    /// 参数: 无；返回: 面板调色板。
    pub(crate) fn current() -> Self {
        Self::for_background(terminal_palette().background)
    }

    /// 【终端面板】【主题配置】按背景亮度选择配色，状态色只表达增删和错误。
    /// 参数: background 为终端背景 RGB；返回: 面板调色板。
    pub(crate) fn for_background(background: Rgb) -> Self {
        let (r, g, b) = background;
        let light = u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 > 128_000;
        let mut palette = if light {
            Self {
                border: (210, 210, 210),
                text: (38, 40, 42),
                muted: (100, 104, 108),
                accent: (23, 109, 123),
                added: (45, 122, 74),
                removed: (169, 62, 73),
                added_bg: (226, 243, 231),
                removed_bg: (253, 229, 230),
                hover_bg: background,
                focus_bg: background,
            }
        } else {
            Self {
                border: (55, 55, 55),
                text: (222, 224, 226),
                muted: (137, 141, 145),
                accent: (103, 193, 207),
                added: (118, 185, 133),
                removed: (216, 125, 125),
                added_bg: (20, 46, 30),
                removed_bg: (63, 25, 30),
                hover_bg: background,
                focus_bg: background,
            }
        };
        // 1. 【终端面板】【主题融合】分隔线、悬停与键盘焦点底色从终端实际背景派生，保留原主题色温
        palette.border = blend(background, palette.text, 18);
        palette.hover_bg = blend(background, palette.text, 8);
        // 2. 【终端面板】【焦点层级】键盘焦点比鼠标悬停深一级，仍浅于分隔线
        palette.focus_bg = blend(background, palette.text, 13);
        // 3. 【终端面板】【浅色可读性】状态、弱化文字和强调色均满足正文对比度，边框仍保持轻量
        for color in [
            &mut palette.text,
            &mut palette.muted,
            &mut palette.accent,
            &mut palette.added,
            &mut palette.removed,
        ] {
            *color = crate::render::terminal_contrast::readable_color_on(
                *color,
                background,
                crate::render::terminal_contrast::MIN_TEXT_CONTRAST,
            );
        }
        palette
    }
}

/// 【终端面板】【颜色混合】按百分比混入前景，提供低对比度边线和轻量悬停底色
/// 参数：base 为背景，color 为混入色，percent 为混入百分比；返回：混合 RGB
fn blend(base: Rgb, color: Rgb, percent: u16) -> Rgb {
    let channel = |base: u8, color: u8| {
        ((u16::from(base) * (100 - percent) + u16::from(color) * percent) / 100) as u8
    };
    (
        channel(base.0, color.0),
        channel(base.1, color.1),
        channel(base.2, color.2),
    )
}

/// 【终端面板】【前景颜色】生成独立前景控制序列，不修改背景。
/// 参数: color 为 RGB；返回: ANSI 前景序列。
pub(crate) fn fg((r, g, b): Rgb) -> String {
    format!("\x1b[38;2;{r};{g};{b}m")
}

/// 【终端面板】【背景颜色】生成整行或差异行背景，不修改语法前景。
/// 参数: color 为 RGB；返回: ANSI 背景序列。
pub(crate) fn bg((r, g, b): Rgb) -> String {
    format!("\x1b[48;2;{r};{g};{b}m")
}

/// 【终端面板】【底色叠加】在每次样式复位后重新铺底，使带颜色片段的整行底色连续
/// 参数: text 为 ANSI 文本，color 为底色；返回: 以复位结尾的着底文本
pub(crate) fn tint(text: &str, color: Rgb) -> String {
    let style = bg(color);
    format!(
        "{style}{}\x1b[0m",
        text.replace("\x1b[0m", &format!("\x1b[0m{style}"))
    )
}

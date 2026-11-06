//! 侧栏和差异窗口共用的终端主题配色。

use crate::render::terminal_palette::terminal_palette;

pub(super) type Rgb = (u8, u8, u8);

#[derive(Clone, Copy)]
pub(super) struct SidebarPalette {
    pub(super) border: Rgb,
    pub(super) text: Rgb,
    pub(super) muted: Rgb,
    pub(super) accent: Rgb,
    pub(super) added: Rgb,
    pub(super) removed: Rgb,
    pub(super) added_bg: Rgb,
    pub(super) removed_bg: Rgb,
    pub(super) hover_bg: Rgb,
    pub(super) focus_bg: Rgb,
}

impl SidebarPalette {
    /// 【终端侧栏】【主题选择】读取缓存的终端背景，使用对应明暗主题。
    /// 参数: 无；返回: 面板调色板。
    pub(super) fn current() -> Self {
        Self::for_background(terminal_palette().background)
    }

    /// 【终端侧栏】【主题配置】按背景亮度选择配色，状态色只表达增删和错误。
    /// 参数: background 为终端背景 RGB；返回: 面板调色板。
    pub(super) fn for_background(background: Rgb) -> Self {
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
        // 1. 【终端侧栏】【主题融合】分隔线、悬停与键盘焦点底色从终端实际背景派生，保留原主题色温
        palette.border = blend(background, palette.text, 18);
        palette.hover_bg = blend(background, palette.text, 8);
        // 2. 【终端侧栏】【焦点层级】键盘焦点比鼠标悬停深一级，仍浅于分隔线
        palette.focus_bg = blend(background, palette.text, 13);
        palette
    }
}

/// 【终端侧栏】【颜色混合】按百分比混入前景，提供低对比度边线和轻量悬停底色
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

/// 【终端侧栏】【前景颜色】生成独立前景控制序列，不修改背景。
/// 参数: color 为 RGB；返回: ANSI 前景序列。
pub(super) fn fg((r, g, b): Rgb) -> String {
    format!("\x1b[38;2;{r};{g};{b}m")
}

/// 【终端审查】【背景颜色】生成差异行背景，不修改语法前景。
/// 参数: color 为 RGB；返回: ANSI 背景序列。
pub(super) fn bg((r, g, b): Rgb) -> String {
    format!("\x1b[48;2;{r};{g};{b}m")
}

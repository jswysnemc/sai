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
        if light {
            Self {
                border: (196, 208, 215),
                text: (38, 51, 58),
                muted: (97, 116, 127),
                accent: (23, 109, 123),
                added: (45, 122, 74),
                removed: (169, 62, 73),
                added_bg: (225, 237, 252),
                removed_bg: (253, 229, 230),
            }
        } else {
            Self {
                border: (55, 68, 76),
                text: (218, 226, 230),
                muted: (126, 145, 156),
                accent: (103, 193, 207),
                added: (118, 185, 133),
                removed: (216, 125, 125),
                added_bg: (20, 42, 66),
                removed_bg: (63, 25, 30),
            }
        }
    }
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

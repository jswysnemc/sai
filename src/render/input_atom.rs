use crate::render::panel_palette::{bg, fg, PanelPalette, Rgb};
use crate::render::terminal_contrast::{blend, contrast, readable_color_on, MIN_TEXT_CONTRAST};
use crate::render::terminal_palette::{terminal_default_colors, terminal_palette};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Mutex, OnceLock};

/// 输入框登记的原子块类型，用于输入与提交回显的统一样式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InputAtomKind {
    Text,
    Image,
    File,
    Skill,
}

impl InputAtomKind {
    /// 【输入胶囊】【终端样式】按当前终端调色板动态返回附件胶囊的前景与底色序列。
    ///
    /// 参数:
    /// - `self`: 原子块类别
    ///
    /// 返回:
    /// - 区分附件类型且适配深浅终端的前景与背景 SGR 序列
    pub(crate) fn style(self) -> &'static str {
        let terminal = terminal_palette();
        let (r, g, b) = terminal.background;
        let light = u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 > 128_000;
        // 1. 【输入胶囊】【深色兼容】无实测终端色的深色默认环境沿用经典 256 色胶囊，兼容非交互回退
        if !light && terminal_default_colors().is_none() {
            return self.classic_dark_style();
        }
        // 2. 【输入胶囊】【动态派生】浅色终端与实测主题均从 PanelPalette::current() 派生底色与字色
        self.style_for_palette(PanelPalette::current(), terminal.true_color)
    }

    /// 【输入胶囊】【面板派生】从指定面板调色板派生柔和胶囊底色与高对比度语义文字色。
    ///
    /// 参数:
    /// - `self`: 原子块类别
    /// - `palette`: 当前面板调色板
    /// - `true_color`: 是否输出 24 位真彩序列
    ///
    /// 返回:
    /// - 缓存后的静态 SGR 样式切片
    pub(crate) fn style_for_palette(self, palette: PanelPalette, true_color: bool) -> &'static str {
        // 1. 【输入胶囊】【色相映射】按附件语义选取面板色，并以悬停底色轻量混入形成柔和胶囊底
        let semantic = match self {
            Self::Text => palette.accent,
            Self::Image => blend(palette.removed, palette.accent, 0.35),
            Self::File => palette.added,
            Self::Skill => palette.warning,
        };
        let capsule_bg = blend(palette.hover_bg, semantic, 0.20);
        let capsule_fg = readable_color_on(semantic, capsule_bg, MIN_TEXT_CONTRAST);
        intern_capsule_style(self as u8, capsule_bg, capsule_fg, true_color)
    }

    /// 【输入胶囊】【经典深色】返回无探测深色终端回退使用的 256 色胶囊序列。
    ///
    /// 参数:
    /// - `self`: 原子块类别
    ///
    /// 返回:
    /// - 经典深色 256 色 SGR 序列
    const fn classic_dark_style(self) -> &'static str {
        match self {
            Self::Text => "\x1b[48;5;25m\x1b[38;5;159m",
            Self::Image => "\x1b[48;5;89m\x1b[38;5;225m",
            Self::File => "\x1b[48;5;23m\x1b[38;5;158m",
            Self::Skill => "\x1b[48;5;58m\x1b[38;5;229m",
        }
    }
}

/// 【输入胶囊】【样式驻留】缓存已派生的胶囊样式，为跨模块 `&'static str` 接口提供零重复分配。
///
/// 参数:
/// - `kind_id`: 原子块枚举序号
/// - `capsule_bg`: 胶囊背景 RGB
/// - `capsule_fg`: 胶囊文字 RGB
/// - `true_color`: 真彩能力标志
///
/// 返回:
/// - 进程期驻留的 SGR 字符串切片
fn intern_capsule_style(
    kind_id: u8,
    capsule_bg: Rgb,
    capsule_fg: Rgb,
    true_color: bool,
) -> &'static str {
    static CACHE: OnceLock<Mutex<HashMap<(u8, Rgb, Rgb, bool), &'static str>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (kind_id, capsule_bg, capsule_fg, true_color);
    let mut guard = cache.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(&cached) = guard.get(&key) {
        return cached;
    }
    let sequence = if true_color {
        format!("{}{}", bg(capsule_bg), fg(capsule_fg))
    } else {
        let bg_code = nearest_palette_256(capsule_bg, None);
        let resolved_bg =
            crate::render::terminal_text_style::palette_rgb(bg_code).unwrap_or(capsule_bg);
        let fg_code = nearest_palette_256(capsule_fg, Some(resolved_bg));
        format!("\x1b[48;5;{bg_code}m\x1b[38;5;{fg_code}m")
    };
    let leaked: &'static str = Box::leak(sequence.into_boxed_str());
    guard.insert(key, leaked);
    leaked
}

/// 【输入胶囊】【256 色映射】查找最接近目标 RGB（且可选满足背景对比度）的 xterm 256 色号。
///
/// 参数:
/// - `target`: 期望 RGB
/// - `against_bg`: 需要满足正文对比度时的背景 RGB
///
/// 返回:
/// - 16 到 255 之间的色号
fn nearest_palette_256(target: Rgb, against_bg: Option<Rgb>) -> u8 {
    (16u8..=255)
        .filter_map(|index| {
            let rgb = crate::render::terminal_text_style::palette_rgb(index)?;
            if let Some(bg_rgb) = against_bg {
                if contrast(rgb, bg_rgb) < MIN_TEXT_CONTRAST {
                    return None;
                }
            }
            let dist = (i32::from(rgb.0) - i32::from(target.0)).pow(2)
                + (i32::from(rgb.1) - i32::from(target.1)).pow(2)
                + (i32::from(rgb.2) - i32::from(target.2)).pow(2);
            Some((dist, index))
        })
        .min()
        .map(|(_, index)| index)
        .unwrap_or(236)
}

/// 提交回显中的真实原子块，范围以完整正文的 UTF-8 字节计量。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InputAtom {
    pub(crate) range: Range<usize>,
    pub(crate) label: String,
    pub(crate) kind: InputAtomKind,
    /// 图片原始数据 URL；其他类型不携带图片来源
    pub(crate) image_source: Option<String>,
}

/// 提交后的完整正文与原子块来源；普通方括号文本不携带原子块元数据。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct InputEcho {
    pub(crate) text: String,
    pub(crate) atoms: Vec<InputAtom>,
}

impl InputEcho {
    /// 【终端】【历史附件】恢复持久化图片来源，并给正文追加可展开的附件标签。
    /// 参数: `text` 为用户正文，`sources` 为图片 URL；返回: 保留正文和图片元数据的回显。
    pub(crate) fn with_images(text: String, sources: &[String]) -> Self {
        let mut echo = Self {
            text,
            atoms: Vec::new(),
        };
        for (index, source) in sources.iter().enumerate() {
            if !echo.text.is_empty() {
                echo.text.push('\n');
            }
            let label = format!("[image {}]", index + 1);
            let start = echo.text.len();
            echo.text.push_str(&label);
            echo.atoms.push(InputAtom {
                range: start..echo.text.len(),
                label,
                kind: InputAtomKind::Image,
                image_source: Some(source.clone()),
            });
        }
        echo
    }
}

/// 渲染已登记的原子块，保留其间的普通正文。
///
/// 参数: `text` 为完整正文，`atoms` 为按位置排序的原子块，`expanded` 控制长文本展开
/// 返回: 标签带附件样式的 ANSI 正文；无效范围按普通正文处理
pub(crate) fn render_input_atoms(text: &str, atoms: &[InputAtom], expanded: bool) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    for atom in atoms {
        if atom.range.start < cursor {
            continue;
        }
        let Some(before) = text.get(cursor..atom.range.start) else {
            continue;
        };
        let Some(body) = text.get(atom.range.clone()) else {
            continue;
        };
        // 1. 只给已登记的区间加标签样式，普通文本保持原样
        output.push_str(before);
        output.push_str(atom.kind.style());
        output.push_str(&atom.label);
        output.push_str("\x1b[0m");
        // 2. 全文视图保留标签并展示长文本原文
        if expanded && atom.kind == InputAtomKind::Text {
            output.push('\n');
            output.push_str(body);
        }
        cursor = atom.range.end;
    }
    output.push_str(&text[cursor..]);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// 【输入胶囊测试】【色彩提取】解析真彩胶囊样式中的背景与前景 RGB。
    ///
    /// 参数:
    /// - `style`: 胶囊样式字符串
    ///
    /// 返回:
    /// - `(背景 RGB, 前景 RGB)`
    fn parse_capsule_rgb(style: &str) -> (Rgb, Rgb) {
        let bg_raw = style
            .strip_prefix("\x1b[48;2;")
            .unwrap()
            .split('m')
            .next()
            .unwrap();
        let fg_raw = style
            .split("\x1b[38;2;")
            .nth(1)
            .unwrap()
            .split('m')
            .next()
            .unwrap();
        let to_rgb = |raw: &str| {
            let mut parts = raw.split(';').map(|item| item.parse::<u8>().unwrap());
            (
                parts.next().unwrap(),
                parts.next().unwrap(),
                parts.next().unwrap(),
            )
        };
        (to_rgb(bg_raw), to_rgb(fg_raw))
    }

    /// 【输入胶囊测试】【浅色终端自适应】验证四类胶囊在浅色面板下使用柔和浅底与高对比度深色字。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn capsule_styles_adapt_to_light_and_dark_panel_palettes() {
        let kinds = [
            InputAtomKind::Text,
            InputAtomKind::Image,
            InputAtomKind::File,
            InputAtomKind::Skill,
        ];
        let light = PanelPalette::for_background((248, 249, 250));
        let dark = PanelPalette::for_background((24, 28, 32));

        let mut light_styles = HashSet::new();
        for kind in kinds {
            let style = kind.style_for_palette(light, true);
            light_styles.insert(style);
            let (bg_rgb, fg_rgb) = parse_capsule_rgb(style);
            assert!(
                bg_rgb.0 > 160 && bg_rgb.1 > 160 && bg_rgb.2 > 160,
                "light capsule bg should stay soft and light: {kind:?} -> {bg_rgb:?}"
            );
            assert!(
                contrast(fg_rgb, bg_rgb) >= MIN_TEXT_CONTRAST - 0.05,
                "light capsule contrast too low: {kind:?} fg={fg_rgb:?} bg={bg_rgb:?}"
            );

            let dark_style = kind.style_for_palette(dark, true);
            let (dark_bg, dark_fg) = parse_capsule_rgb(dark_style);
            assert!(
                dark_bg.0 < 110 && dark_bg.1 < 110 && dark_bg.2 < 110,
                "dark capsule bg should stay muted dark: {kind:?} -> {dark_bg:?}"
            );
            assert!(
                contrast(dark_fg, dark_bg) >= MIN_TEXT_CONTRAST - 0.05,
                "dark capsule contrast too low: {kind:?} fg={dark_fg:?} bg={dark_bg:?}"
            );
        }
        assert_eq!(light_styles.len(), kinds.len());
    }
}

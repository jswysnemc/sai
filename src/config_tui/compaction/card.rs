//! 紧凑圆角压缩策略卡片、明暗终端自适应配色与窄矮窗口降级排版。

use crate::render::ansi_clip::{ansi_visible_width, clip_ansi_to_width};
use crate::render::panel_palette::{PanelPalette, Rgb};
use crate::render::transcript::AnsiLine;

pub(super) const RESET: &str = "\x1b[0m";

/// 压缩卡片明暗自适应调色板。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CardPalette {
    pub accent: &'static str,
    pub brand: &'static str,
    pub border: &'static str,
    pub muted: &'static str,
    pub selected: &'static str,
    pub hover: &'static str,
    pub used: &'static str,
    pub trigger: &'static str,
    pub danger: &'static str,
}

const DARK_CARD: CardPalette = CardPalette {
    accent: "\x1b[38;2;170;232;246m",
    brand: "\x1b[1m\x1b[38;2;94;196;168m",
    border: "\x1b[38;2;90;110;118m",
    muted: "\x1b[38;2;140;160;168m",
    selected: "\x1b[48;2;36;64;72m\x1b[1m",
    hover: "\x1b[48;2;28;48;54m",
    used: "\x1b[38;2;110;210;165m",
    trigger: "\x1b[38;2;245;166;110m",
    danger: "\x1b[38;2;236;128;132m",
};

const LIGHT_CARD: CardPalette = CardPalette {
    accent: "\x1b[38;2;14;100;118m",
    brand: "\x1b[1m\x1b[38;2;18;122;98m",
    border: "\x1b[38;2;132;146;154m",
    muted: "\x1b[38;2;86;102;110m",
    selected: "\x1b[48;2;212;234;238m\x1b[1m\x1b[38;2;12;72;86m",
    hover: "\x1b[48;2;232;244;246m",
    used: "\x1b[38;2;32;124;62m",
    trigger: "\x1b[38;2;176;98;20m",
    danger: "\x1b[38;2;176;48;60m",
};

impl CardPalette {
    /// 【上下文】【当前卡片配色】通过共享 PanelPalette 感知终端明暗背景并返回卡片色板。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 当前适用的卡片配色引用
    pub(super) fn current() -> &'static Self {
        Self::from_panel(PanelPalette::current())
    }

    /// 【上下文】【指定背景配色】按给定终端背景 RGB 返回深浅卡片色板。
    ///
    /// 参数:
    /// - `background`: 终端背景 RGB
    ///
    /// 返回:
    /// - 卡片配色引用
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn for_background(background: Rgb) -> &'static Self {
        Self::from_panel(PanelPalette::for_background(background))
    }

    fn from_panel(panel: PanelPalette) -> &'static Self {
        let (r, g, b) = panel.focus_bg;
        let light = u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114 > 128_000;
        if light {
            &LIGHT_CARD
        } else {
            &DARK_CARD
        }
    }
}

/// 【上下文】【矮屏帮助文案】返回高度裁剪模式下底边嵌入的精简保存/取消提示。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 双语简短快捷键说明
pub(super) fn short_height_help() -> &'static str {
    crate::i18n::text("Enter save · Esc cancel", "Enter 保存 · Esc 取消")
}

/// 【上下文】【数值缩写】按数量级缩写窗口和预留，原始数值仍用于计算。
///
/// 参数:
/// - `value`: token 数
///
/// 返回:
/// - 简短数量级标签
pub(super) fn compact(value: usize) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 && value.is_multiple_of(1_000) {
        format!("{}k", value / 1_000)
    } else {
        value.to_string()
    }
}

/// 【上下文】【边框排版】在边线上嵌入标题或快捷键，不超出给定列宽。
fn edge(text: &str, width: usize, bottom: bool) -> String {
    let border = CardPalette::current().border;
    let text = clip_ansi_to_width(text, width.saturating_sub(6), "");
    let fill = width.saturating_sub(ansi_visible_width(&text) + 5);
    let (left, right) = if bottom {
        ('╰', '╯')
    } else {
        ('╭', '╮')
    };
    format!(
        "{border}{left}─ {RESET}{text}{border} {}{right}{RESET}",
        "─".repeat(fill)
    )
}

/// 【上下文】【内容边框】为正文或快捷键补齐内侧空格并绘制统一侧边。
fn row(text: &str, inner: usize) -> String {
    let border = CardPalette::current().border;
    format!(
        "{border}│{RESET} {text}{RESET}{} {border}│{RESET}",
        " ".repeat(inner.saturating_sub(ansi_visible_width(text)))
    )
}

/// 【上下文】【快捷键换行】按完整操作分组折行，避免窄屏切断按键名称和说明。
///
/// 参数:
/// - `help`: 以间隔点分组的帮助文本
/// - `width`: 正文可用列宽
///
/// 返回:
/// - 按组折行后的帮助行列表
pub(super) fn help_rows(help: &str, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let mut current = String::new();
    for group in help.split(" · ") {
        let candidate = if current.is_empty() {
            group.to_string()
        } else {
            format!("{current} · {group}")
        };
        if ansi_visible_width(&candidate) <= width {
            current = candidate;
            continue;
        }
        if !current.is_empty() {
            rows.push(std::mem::take(&mut current));
        }
        if ansi_visible_width(group) > width {
            rows.extend(
                AnsiLine::wrap_block(group, width)
                    .into_iter()
                    .map(|line| line.as_str().to_string()),
            );
        } else {
            current = group.to_string();
        }
    }
    if !current.is_empty() {
        rows.push(current);
    }
    rows
}

/// 【上下文】【卡片组合】宽屏使用圆角边框，极窄屏退回无边框文本。
pub(super) fn frame(title: &str, body: &[String], help: &str, width: usize) -> Vec<String> {
    let palette = CardPalette::current();
    let framed = width >= 16;
    let inner = width.saturating_sub(if framed { 4 } else { 0 }).max(1);
    let mut lines = vec![if framed {
        edge(title, width, false)
    } else {
        clip_ansi_to_width(title, width, "")
    }];
    for source in body {
        for line in AnsiLine::wrap_block(source, inner) {
            let text = line.as_str();
            lines.push(if framed {
                row(text, inner)
            } else {
                clip_ansi_to_width(text, width, "")
            });
        }
    }
    if framed && ansi_visible_width(help) + 6 <= width {
        lines.push(edge(
            &format!("{}{help}{RESET}", palette.muted),
            width,
            true,
        ));
    } else {
        for text in help_rows(help, inner) {
            lines.push(if framed {
                row(&format!("{}{text}{RESET}", palette.muted), inner)
            } else {
                clip_ansi_to_width(&text, width, "")
            });
        }
        if framed {
            lines.push(format!(
                "{}╰{}╯{RESET}",
                palette.border,
                "─".repeat(width - 2)
            ));
        }
    }
    lines
}

/// 【上下文】【矮屏布局】高度不足时保留模式控件和精简退出说明，避免越界写屏。
pub(super) fn fit_height(mut lines: Vec<String>, rows: usize) -> Vec<String> {
    if lines.len() > rows {
        lines.truncate(rows);
        if let Some(last) = lines.last_mut() {
            let palette = CardPalette::current();
            let width = ansi_visible_width(last);
            let help = short_height_help();
            *last = if width >= 16 {
                edge(&format!("{}{help}{RESET}", palette.muted), width, true)
            } else {
                clip_ansi_to_width(help, width, "")
            };
        }
    }
    lines
}

//! 后台任务面板公共视觉组件：状态图标、徽章胶囊与主题分隔线。

use super::model::Stream;
use crate::render::ansi_clip::{ansi_visible_width, clip_ansi_to_width};
use crate::render::panel_palette::{fg, tint, PanelPalette, Rgb};

pub(super) const RESET: &str = "\x1b[0m";

/// 【后台任务】【状态图标】根据状态返回单一几何标记，禁用 Emoji。
///
/// 参数:
/// - `status`: 服务端任务状态字符串
///
/// 返回:
/// - 对应状态的单字符图标
pub(super) fn status_glyph(status: &str) -> &'static str {
    match status {
        "running" => "●",
        "failed" | "timed_out" => "×",
        "stopped" => "■",
        _ => "✓",
    }
}

/// 【后台任务】【通道名称】返回当前输出通道短标签。
///
/// 参数:
/// - `stream`: 输出通道枚举
///
/// 返回:
/// - 大写通道标识
pub(super) fn stream_name(stream: Stream) -> &'static str {
    match stream {
        Stream::Stdout => "STDOUT",
        Stream::Stderr => "STDERR",
        Stream::All => "ALL",
    }
}

/// 【后台任务】【徽章胶囊】在透明终端底色上用细微主题衬底绘制紧凑状态胶囊。
///
/// 参数:
/// - `label`: 胶囊文字
/// - `color`: 前景语义色
/// - `bg`: 胶囊浅底色，为空时不填背景
/// - `bold`: 是否加粗
///
/// 返回:
/// - 含完整复位序列的胶囊文本
pub(super) fn pill(label: &str, color: Rgb, bg: Option<Rgb>, bold: bool) -> String {
    let weight = if bold { "\x1b[1m" } else { "" };
    let text = format!("{weight}{}{label}", fg(color));
    match bg {
        Some(bg_color) => tint(&text, bg_color),
        None => format!("{text}{RESET}"),
    }
}

/// 【后台任务】【宽度裁剪】超宽时按显示列截断并附加省略号，保留转义序列完整。
///
/// 参数:
/// - `text`: 含样式文本
/// - `width`: 最大可见宽度
///
/// 返回:
/// - 适配宽度的文本
pub(super) fn fit(text: &str, width: usize) -> String {
    if ansi_visible_width(text) <= width {
        text.to_string()
    } else {
        clip_ansi_to_width(text, width, if width > 0 { "…" } else { "" })
    }
}

/// 【后台任务】【分段标题】绘制标题与右侧延伸细线，保持双栏和单栏标题结构统一。
///
/// 参数:
/// - `label`: 已着色标题文本
/// - `right`: 可选右侧辅助标记
/// - `width`: 可用总宽度
/// - `palette`: 当前面板色板
///
/// 返回:
/// - 铺满宽度的标题分隔行
pub(super) fn section_header(
    label: &str,
    right: &str,
    width: usize,
    palette: &PanelPalette,
) -> String {
    let label_w = ansi_visible_width(label);
    let right_w = ansi_visible_width(right);
    if right_w == 0 || label_w + right_w + 4 > width {
        let used = label_w + 2;
        return fit(
            &format!(
                " {label}{RESET} {}{}\x1b[0m",
                fg(palette.border),
                "─".repeat(width.saturating_sub(used))
            ),
            width,
        );
    }
    let gap = width.saturating_sub(label_w + right_w + 5);
    fit(
        &format!(
            " {label}{RESET} {}{}\x1b[0m {right}{RESET} ",
            fg(palette.border),
            "─".repeat(gap)
        ),
        width,
    )
}

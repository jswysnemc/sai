use super::{blend_color, RESET};
use crate::render::terminal_palette::{terminal_palette, TerminalPalette};

/// 复用现有实心圆，整个周期保持单列轮廓与基线
pub(crate) const ACTIVITY_GUIDE: char = crate::render::style::GUIDE_FILLED;
/// 每轮 1.6 秒，两个亮度脉冲之后短暂停歇
const GUIDE_CYCLE_FRAMES: usize = 50;
const GUIDE_MIN_CONTRAST: f32 = 0.6;

/// 【终端】【状态动效】渲染与终端前景同色的双脉冲圆点。
/// 参数：`frame` 为动画帧号；返回：带亮度变化的单列 ANSI 引导符。
pub(crate) fn render_activity_guide(frame: usize) -> String {
    render_activity_guide_with_color(frame, None)
}

/// 【终端】【状态动效】使用原有状态颜色渲染双脉冲圆点。
/// 参数：`frame` 为动画帧号，`color` 为可选 RGB 状态色；返回：单列 ANSI 引导符。
pub(crate) fn render_activity_guide_with_color(
    frame: usize,
    color: Option<(u8, u8, u8)>,
) -> String {
    render_guide_at(frame, color, terminal_palette())
}

/// 【终端】【状态动效】在深浅主题中保持轮廓和最低对比度，仅改变圆点亮度。
/// 参数：`frame` 为帧号，`color` 为状态色，`palette` 为终端颜色；返回：ANSI 圆点。
pub(super) fn render_guide_at(
    frame: usize,
    color: Option<(u8, u8, u8)>,
    palette: TerminalPalette,
) -> String {
    // 1. 【终端】【状态动效】主脉冲与稍弱的第二脉冲共用平滑曲线，字形和位置固定
    let position = (frame % GUIDE_CYCLE_FRAMES) as f32;
    let pulse =
        (pulse_at(position, 8.0, 8.0) + 0.7 * pulse_at(position, 22.0, 7.0)).clamp(0.0, 1.0);
    let mut output = String::from("\x1b[22m");
    if palette.true_color {
        // 2. 【终端】【状态动效】沿背景到状态色的方向变化，深浅主题均保留六成对比度
        let contrast = GUIDE_MIN_CONTRAST + (1.0 - GUIDE_MIN_CONTRAST) * pulse;
        let (red, green, blue) = blend_color(
            palette.background,
            color.unwrap_or(palette.foreground),
            contrast,
        );
        output.push_str(&format!("\x1b[38;2;{red};{green};{blue}m"));
    } else {
        // 3. 【终端】【状态动效】低色彩终端保留状态色，用弱化和正常亮度表达两个脉冲
        let ansi = color.map(ansi_status_color).unwrap_or(39);
        output.push_str(&format!("\x1b[{ansi}m"));
        if pulse < 0.35 {
            output.push_str("\x1b[2m");
        }
    }
    output.push(ACTIVITY_GUIDE);
    output.push_str(RESET);
    output
}

/// 【终端】【状态动效】计算两端斜率为零的单次亮度脉冲。
/// 参数：`position` 为帧位置，`center` 为峰值位置，`radius` 为半宽；返回：0 到 1 的强度。
fn pulse_at(position: f32, center: f32, radius: f32) -> f32 {
    let distance = (position - center).abs();
    if distance >= radius {
        return 0.0;
    }
    0.5 * (1.0 + (std::f32::consts::PI * distance / radius).cos())
}

/// 【终端】【状态动效】按色相将状态色映射到基础 ANSI 颜色。
/// 参数：`color` 为原始 RGB 状态色；返回：30 到 37 的 ANSI 前景色代码。
fn ansi_status_color(color: (u8, u8, u8)) -> usize {
    let minimum = color.0.min(color.1).min(color.2);
    let maximum = color.0.max(color.1).max(color.2);
    if maximum - minimum < 32 {
        return if maximum < 64 { 30 } else { 37 };
    }
    let midpoint = (u16::from(minimum) + u16::from(maximum)) / 2;
    30 + usize::from(u16::from(color.0) > midpoint)
        + 2 * usize::from(u16::from(color.1) > midpoint)
        + 4 * usize::from(u16::from(color.2) > midpoint)
}

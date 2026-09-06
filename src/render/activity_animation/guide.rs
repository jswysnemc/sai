use super::{blend_color, RESET};

/// 固定单列竖条，整个周期保持相同轮廓与基线
pub(crate) const ACTIVITY_GUIDE: char = '▮';
/// 两秒一轮的呼吸节拍
const GUIDE_CYCLE_FRAMES: usize = 63;
/// 呼吸低点保留六成亮度，避免运行状态消失
const GUIDE_MIN_BRIGHTNESS: f32 = 0.6;
const GUIDE_COLOR: (u8, u8, u8) = (239, 239, 239);

/// 【终端】【状态动效】渲染固定单列的中性灰阶呼吸竖条。
/// 参数：`frame` 为动画帧号；返回：带亮度变化的 ANSI 引导符。
pub(crate) fn render_activity_guide(frame: usize) -> String {
    render_activity_guide_with_color(frame, None)
}

/// 【终端】【状态动效】按原有状态色相渲染呼吸竖条。
/// 参数：`frame` 为动画帧号，`color` 为可选 RGB 状态色；返回：单列 ANSI 引导符。
pub(crate) fn render_activity_guide_with_color(
    frame: usize,
    color: Option<(u8, u8, u8)>,
) -> String {
    // 1. 【终端】【状态动效】亮度平滑往复，字形和位置始终固定
    let phase = (frame % GUIDE_CYCLE_FRAMES) as f32 / GUIDE_CYCLE_FRAMES as f32;
    let pulse = (1.0 + (std::f32::consts::TAU * phase).cos()) / 2.0;
    let brightness = GUIDE_MIN_BRIGHTNESS + (1.0 - GUIDE_MIN_BRIGHTNESS) * pulse;
    // 2. 【终端】【状态动效】等比例调整颜色通道，保留子任务与待办的状态色相
    let (red, green, blue) = blend_color((0, 0, 0), color.unwrap_or(GUIDE_COLOR), brightness);
    format!("\x1b[22m\x1b[38;2;{red};{green};{blue}m{ACTIVITY_GUIDE}{RESET}")
}

use super::{blend_color, ACTIVITY_FRAME_INTERVAL, RESET};
use crate::render::terminal_palette::{terminal_palette, TerminalPalette};
use std::time::Duration;

const PADDING: usize = 10;
const SWEEP_SECONDS: f32 = 2.0;
const BAND_HALF_WIDTH: f32 = 5.0;

/// 【终端】【状态动效】沿用 Codex 的两秒扫描与前景、背景混色渲染状态文字。
/// 参数：`text` 为状态文字，`frame` 为共享时钟帧号；返回：保持原文的 ANSI 流光文本。
pub(crate) fn render_activity_text(text: &str, frame: usize) -> String {
    let elapsed = Duration::from_secs_f64(frame as f64 * ACTIVITY_FRAME_INTERVAL.as_secs_f64());
    render_shimmer_at(text, elapsed, terminal_palette())
}

/// 【终端】【状态动效】按指定时间和调色板生成流光，便于逐帧核对参考实现。
/// 参数：`text` 为原文，`elapsed` 为时钟时长，`palette` 为终端颜色；返回：ANSI 文本。
pub(super) fn render_shimmer_at(text: &str, elapsed: Duration, palette: TerminalPalette) -> String {
    if text.is_empty() {
        return String::new();
    }
    // 1. 【终端】【状态动效】与 Codex 一样按字符定位，扫描两端各保留十个字符的缓冲
    let period = text.chars().count() + PADDING * 2;
    let position =
        ((elapsed.as_secs_f32() % SWEEP_SECONDS) / SWEEP_SECONDS * period as f32) as usize;
    let mut output = String::new();
    if palette.true_color {
        output.push_str("\x1b[22m\x1b[1m");
    }
    for (index, ch) in text.chars().enumerate() {
        let distance = (index + PADDING).abs_diff(position) as f32;
        let intensity = if distance <= BAND_HALF_WIDTH {
            0.5 * (1.0 + (std::f32::consts::PI * (distance / BAND_HALF_WIDTH)).cos())
        } else {
            0.0
        };
        // 2. 【终端】【状态动效】扫描带向背景色混合，其余字符保持默认前景色
        if palette.true_color {
            let (red, green, blue) = blend_color(
                palette.foreground,
                palette.background,
                intensity.clamp(0.0, 1.0) * 0.9,
            );
            output.push_str(&format!("\x1b[38;2;{red};{green};{blue}m{ch}"));
        } else {
            // 3. 【终端】【状态动效】低色彩终端使用 Codex 的三级样式，并清除继承样式
            output.push_str("\x1b[22m\x1b[39m");
            if intensity < 0.2 {
                output.push_str("\x1b[2m");
            } else if intensity >= 0.6 {
                output.push_str("\x1b[1m");
            }
            output.push(ch);
        }
    }
    output.push_str(RESET);
    output
}

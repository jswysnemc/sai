use std::time::Duration;

mod guide;
mod shimmer;

pub(crate) use guide::{render_activity_guide, render_activity_guide_with_color, ACTIVITY_GUIDE};
pub(crate) use shimmer::render_activity_text;

/// 状态动效统一刷新节拍
pub(crate) const ACTIVITY_FRAME_INTERVAL: Duration = Duration::from_millis(32);
const RESET: &str = "\x1b[0m";

/// 【终端】【状态动效】按比例混合两个 RGB 颜色。
/// 参数：`from` 为起始颜色，`to` 为目标颜色，`ratio` 为混合比例；返回：混合后的 RGB 颜色。
fn blend_color(from: (u8, u8, u8), to: (u8, u8, u8), ratio: f32) -> (u8, u8, u8) {
    let mix = |a: u8, b: u8| -> u8 {
        let value = a as f32 + (b as f32 - a as f32) * ratio.clamp(0.0, 1.0);
        value.round().clamp(0.0, 255.0) as u8
    };
    (mix(from.0, to.0), mix(from.1, to.1), mix(from.2, to.2))
}

/// 【终端】【状态动效】渲染状态文字后的弱化说明。
/// 参数：`text` 为耗时、token 或模型等辅助信息；返回：弱化白色 ANSI 文本。
pub(crate) fn render_activity_detail(text: &str) -> String {
    format!("\x1b[2m\x1b[37m{text}{RESET}")
}

/// 【终端】【状态动效】组合呼吸竖条、流光标题和弱化详情。
/// 参数：`label` 为状态文字，`detail` 为可选详情，`frame` 为动画帧号；返回：完整 ANSI 状态行。
pub(crate) fn render_activity_line(label: &str, detail: &str, frame: usize) -> String {
    let mut output = format!(
        "{} {}",
        render_activity_guide(frame),
        render_activity_text(label, frame)
    );
    if !detail.is_empty() {
        output.push(' ');
        output.push_str(&render_activity_detail(detail));
    }
    output
}

/// 【终端】【状态动效】按真实时长换算帧号，避免调用方刷新间隔改变动画速度。
/// 参数：`elapsed` 为动画开始至今的时长；返回：当前应当渲染的帧序号。
pub(crate) fn activity_frame_at(elapsed: Duration) -> usize {
    let interval = ACTIVITY_FRAME_INTERVAL.as_micros().max(1);
    (elapsed.as_micros() / interval) as usize
}

/// 【终端】【状态动效测试】去除终端文本中的 ANSI 控制序列。
///
/// 参数:
/// - `text`: 带终端样式的文本
///
/// 返回:
/// - 仅保留可见字符的文本
/// 供测试与调试日志剥离 ANSI；体积极小，非 test 构建也可调用。
pub(crate) fn strip_ansi_for_test(text: &str) -> String {
    let mut output = String::new();
    let mut index = 0usize;
    while index < text.len() {
        if text[index..].starts_with('\x1b') {
            index = crate::render::terminal_image::escape_sequence_end(text, index).max(index + 1);
            continue;
        }
        let ch = text[index..].chars().next().unwrap_or_default();
        output.push(ch);
        index += ch.len_utf8();
    }
    output
}

#[cfg(test)]
mod tests;

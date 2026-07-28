const SWEEP_TAIL_LENGTH: usize = 3;
const SWEEP_PAUSE_FRAMES: usize = 3;
const DIM_STYLE: &str = "\x1b[22m\x1b[2m\x1b[38;2;77;116;125m";
const TRAIL_STYLE: &str = "\x1b[22m\x1b[38;2;92;176;194m";
const ACTIVE_STYLE: &str = "\x1b[22m\x1b[1m\x1b[38;2;190;246;255m";
const RESET: &str = "\x1b[0m";

/// 【终端】【状态动效】渲染从左向右扫过状态文字的明暗动画。
///
/// 参数:
/// - `text`: Waiting、Thinking、Working 等状态文字
/// - `frame`: 当前动画帧序号
///
/// 返回:
/// - 每个字符按当前扫光位置着色的 ANSI 文本
pub(crate) fn render_activity_text(text: &str, frame: usize) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    if chars.is_empty() {
        return String::new();
    }
    let active_position = frame % activity_frame_count(text);
    let mut output = String::new();
    for (index, ch) in chars.iter().enumerate() {
        let style = match active_position.checked_sub(index) {
            Some(0) => ACTIVE_STYLE,
            Some(distance) if distance < SWEEP_TAIL_LENGTH => TRAIL_STYLE,
            _ => DIM_STYLE,
        };
        output.push_str(style);
        output.push(*ch);
    }
    output.push_str(RESET);
    output
}

/// 【终端】【状态动效】渲染状态文字后的弱化说明。
///
/// 参数:
/// - `text`: 耗时、token 或模型等辅助信息
///
/// 返回:
/// - 弱化青色 ANSI 文本
pub(crate) fn render_activity_detail(text: &str) -> String {
    format!("{DIM_STYLE}{text}{RESET}")
}

/// 【终端】【状态动效】返回一轮文字扫光包含的帧数。
///
/// 参数:
/// - `text`: 状态文字
///
/// 返回:
/// - 字符数量与末尾停顿帧之和
pub(crate) fn activity_frame_count(text: &str) -> usize {
    text.chars().count().saturating_add(SWEEP_PAUSE_FRAMES).max(1)
}

/// 【终端】【状态动效测试】去除终端文本中的 ANSI 控制序列。
///
/// 参数:
/// - `text`: 带终端样式的文本
///
/// 返回:
/// - 仅保留可见字符的文本
#[cfg(test)]
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
mod tests {
    use super::*;

    /// 【终端】【状态动效】验证高亮位置按字符从左向右移动。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn activity_highlight_moves_from_left_to_right() {
        let first = render_activity_text("Working", 0);
        let second = render_activity_text("Working", 1);

        assert_ne!(first, second);
        assert!(first.starts_with(ACTIVE_STYLE));
        assert!(second.starts_with(TRAIL_STYLE));
        assert!(second.contains(&format!("{ACTIVE_STYLE}o")));
    }

    /// 【终端】【状态动效】验证扫光末尾保留短暂停顿并稳定循环。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn activity_animation_loops_after_pause_frames() {
        let frames = activity_frame_count("Waiting");

        assert_eq!(render_activity_text("Waiting", 0), render_activity_text("Waiting", frames));
        assert!(frames > "Waiting".chars().count());
    }
}

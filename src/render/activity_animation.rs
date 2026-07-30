/// 扫光高亮向左拖出的渐隐长度（字符数）
const SWEEP_TAIL_LENGTH: f32 = 4.0;
/// 一轮扫光结束后的停顿帧数，避免循环显得突兀
const SWEEP_PAUSE_FRAMES: usize = 4;
/// 每个字符位细分的动画子帧数，帧数越多明暗过渡越平滑
const SUBFRAMES_PER_CELL: usize = 3;
/// 引导点一次完整呼吸占用的帧数
const PULSE_FRAMES: usize = 12;

/// 状态文字的暗态颜色
const DIM_COLOR: (u8, u8, u8) = (77, 116, 125);
/// 状态文字的亮态颜色
const BRIGHT_COLOR: (u8, u8, u8) = (190, 246, 255);
const RESET: &str = "\x1b[0m";

/// 视觉引导点字符
const GUIDE_DOT: char = '•';

/// 【终端】【状态动效】渲染状态行的视觉引导点。
///
/// 引导点与助手正文使用同一符号，因此状态行与正文共用一条视觉基线；
/// 亮度按正弦呼吸变化，用来表示"仍在进行"而不抢夺文字的注意力。
///
/// 参数:
/// - `frame`: 当前动画帧序号
///
/// 返回:
/// - 带亮度的引导点 ANSI 文本
pub(crate) fn render_activity_guide(frame: usize) -> String {
    // 正弦取值映射到 0.35..0.85，避免最暗时几乎看不见、最亮时与文字峰值撞色
    let phase = (frame % PULSE_FRAMES) as f32 / PULSE_FRAMES as f32;
    let wave = (phase * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    format!(
        "{}{GUIDE_DOT}{RESET}",
        color_escape(0.35 + wave * 0.5)
    )
}

/// 【终端】【状态动效】渲染从左向右扫过状态文字的明暗动画。
///
/// 高亮位置按子帧推进，每个字符的亮度由它与高亮中心的距离连续插值得出，
/// 因此相邻帧之间的色差很小，观感是平滑流动而不是逐格跳变。
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
    // 1. 高亮中心用子帧精度表示，可落在两个字符之间
    let total = activity_frame_count(text);
    let center = (frame % total) as f32 / SUBFRAMES_PER_CELL as f32;
    let mut output = String::new();
    for (index, ch) in chars.iter().enumerate() {
        // 2. 只向左拖尾：高亮已经扫过的字符渐隐，未扫到的保持暗态
        let distance = center - index as f32;
        let intensity = if (0.0..SWEEP_TAIL_LENGTH).contains(&distance) {
            1.0 - distance / SWEEP_TAIL_LENGTH
        } else {
            0.0
        };
        output.push_str(&color_escape(intensity));
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
    format!("\x1b[2m{}{text}{RESET}", color_escape(0.0))
}

/// 【终端】【状态动效】返回一轮文字扫光包含的帧数。
///
/// 参数:
/// - `text`: 状态文字
///
/// 返回:
/// - 字符位子帧总数与末尾停顿帧之和
pub(crate) fn activity_frame_count(text: &str) -> usize {
    text.chars()
        .count()
        .saturating_mul(SUBFRAMES_PER_CELL)
        .saturating_add(SWEEP_PAUSE_FRAMES)
        .max(1)
}

/// 【终端】【状态动效】按亮度插值生成前景色控制序列。
///
/// 参数:
/// - `intensity`: 0 为暗态、1 为亮态，超出范围时收敛到端点
///
/// 返回:
/// - 24 位真彩前景色序列
fn color_escape(intensity: f32) -> String {
    let ratio = intensity.clamp(0.0, 1.0);
    let channel = |dim: u8, bright: u8| {
        (f32::from(dim) + (f32::from(bright) - f32::from(dim)) * ratio).round() as u8
    };
    let red = channel(DIM_COLOR.0, BRIGHT_COLOR.0);
    let green = channel(DIM_COLOR.1, BRIGHT_COLOR.1);
    let blue = channel(DIM_COLOR.2, BRIGHT_COLOR.2);
    format!("\x1b[22m\x1b[38;2;{red};{green};{blue}m")
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

    /// 【终端】【状态动效】验证高亮位置按帧从左向右移动。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn activity_highlight_moves_from_left_to_right() {
        let early = brightest_index("Working", 0);
        let later = brightest_index("Working", SUBFRAMES_PER_CELL * 3);

        assert!(
            later > early,
            "高亮应向右推进: {early} -> {later}"
        );
    }

    /// 【终端】【状态动效】验证相邻帧之间的亮度变化足够细腻。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn adjacent_frames_change_smoothly() {
        // 子帧精度下同一字符在相邻帧的色差应远小于暗亮两端的跨度
        let span = f32::from(BRIGHT_COLOR.2) - f32::from(DIM_COLOR.2);
        // 取仍处在扫光拖尾范围内的字符，范围外恒为暗态、观测不到变化
        let first = channel_at("Working", 4, 1);
        let second = channel_at("Working", 5, 1);

        assert_ne!(first, second, "相邻帧必须有变化，否则动效停滞");
        assert!(
            (f32::from(first) - f32::from(second)).abs() < span / 2.0,
            "相邻帧色差过大，过渡不平滑: {first} -> {second}"
        );
    }

    /// 【终端】【状态动效】验证扫光末尾保留停顿并稳定循环。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn activity_animation_loops_after_pause_frames() {
        let frames = activity_frame_count("Waiting");

        assert_eq!(
            render_activity_text("Waiting", 0),
            render_activity_text("Waiting", frames)
        );
        assert!(frames > "Waiting".chars().count());
    }

    /// 【终端】【状态动效】验证引导点在呼吸周期内明暗往复。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn guide_dot_pulses_within_one_cycle() {
        let samples = (0..PULSE_FRAMES)
            .map(|frame| render_activity_guide(frame))
            .collect::<Vec<_>>();

        assert!(samples.iter().all(|dot| strip_ansi_for_test(dot) == "•"));
        assert!(
            samples.iter().collect::<std::collections::HashSet<_>>().len() > 1,
            "引导点必须有明暗变化"
        );
        assert_eq!(
            render_activity_guide(0),
            render_activity_guide(PULSE_FRAMES),
            "呼吸应按周期循环"
        );
    }

    /// 返回指定帧下亮度最高的字符下标。
    ///
    /// 参数:
    /// - `text`: 状态文字
    /// - `frame`: 帧序号
    ///
    /// 返回:
    /// - 最亮字符的下标
    fn brightest_index(text: &str, frame: usize) -> usize {
        (0..text.chars().count())
            .max_by_key(|index| channel_at(text, frame, *index))
            .unwrap_or_default()
    }

    /// 读取指定字符在指定帧的蓝色通道值。
    ///
    /// 蓝色通道在暗亮两端跨度最大，最适合作为亮度观测量。
    ///
    /// 参数:
    /// - `text`: 状态文字
    /// - `frame`: 帧序号
    /// - `index`: 字符下标
    ///
    /// 返回:
    /// - 蓝色通道取值
    fn channel_at(text: &str, frame: usize, index: usize) -> u8 {
        let rendered = render_activity_text(text, frame);
        // 每个字符前都写入完整的前景色序列，按顺序取第 index 段
        rendered
            .split("\x1b[38;2;")
            .nth(index + 1)
            .and_then(|segment| segment.split('m').next())
            .and_then(|color| color.split(';').nth(2).map(str::to_string))
            .and_then(|blue| blue.parse().ok())
            .unwrap_or_default()
    }
}

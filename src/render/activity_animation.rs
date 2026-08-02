use std::time::Duration;

/// 状态动效统一刷新节拍，与 Codex TUI 的帧率保持一致
pub(crate) const ACTIVITY_FRAME_INTERVAL: Duration = Duration::from_millis(32);
/// 亮带进入和离开文字前保留的字符位
const SHIMMER_PADDING: usize = 10;
/// 余弦亮带的半宽度（字符位）
const SHIMMER_BAND_HALF_WIDTH: f32 = 5.0;
/// 两秒一轮对应的帧数
const SHIMMER_CYCLE_FRAMES: usize = 63;

/// 亮带峰值处朝高亮色混合的最大比例
///
/// 与 Codex 一致保留一成底色，避免峰值处文字完全失去原有色相。
const SHIMMER_MAX_BLEND: f32 = 0.9;

/// 状态文字的基础颜色，对应亮带之外的常态字色
///
/// 这里刻意取比正文暗一档的中性灰，而不是正文常态字色。流光扫过时
/// 文字从中灰被点亮到接近纯白，明暗跨度足够大，扫光才看得出在动；
/// 若基础色本身就接近白色，整条带子的对比只剩几十级灰阶，观感像静止。
/// 状态行是临时提示，常态偏暗不影响阅读，换来的是清晰的动效。
const BASE_COLOR: (u8, u8, u8) = (96, 96, 96);
/// 亮带峰值处朝之混合的高亮颜色，对应终端默认背景
const HIGHLIGHT_COLOR: (u8, u8, u8) = (255, 255, 255);
const RESET: &str = "\x1b[0m";

/// 视觉引导点字符
const GUIDE_DOT: char = '•';

/// 【终端】【状态动效】渲染状态行的视觉引导点。
///
/// 引导点与助手正文使用同一符号，因此状态行与正文共用一条视觉基线；
/// 亮度复用 Codex 风格的余弦亮带，颜色只在中性灰与白色之间变化。
///
/// 参数:
/// - `frame`: 当前动画帧序号
///
/// 返回:
/// - 带亮度的引导点 ANSI 文本
pub(crate) fn render_activity_guide(frame: usize) -> String {
    let intensity = shimmer_intensity(1, frame, 0);
    format!("{}{GUIDE_DOT}{RESET}", color_escape(intensity))
}

/// 【终端】【状态动效】渲染从左向右扫过状态文字的白色余弦流光。
///
/// 实现参考 Codex TUI 流光：亮带前后保留固定留白，并使用余弦函数
/// 平滑衰减。全部颜色使用中性灰阶，不读取主题色。
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
    let mut output = String::new();
    for (index, ch) in chars.iter().enumerate() {
        // 1. 每个字符按它与余弦亮带中心的距离计算白色亮度
        let intensity = shimmer_intensity(chars.len(), frame, index);
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
/// - 弱化白色 ANSI 文本
pub(crate) fn render_activity_detail(text: &str) -> String {
    format!("\x1b[2m\x1b[37m{text}{RESET}")
}

/// 【终端】【状态动效】渲染统一的引导点、扫光标题与弱化详情。
///
/// 参数:
/// - `label`: Waiting、Thinking、Working 等状态文字
/// - `detail`: 耗时或 token 等辅助信息；空值时不追加间隔
/// - `frame`: 当前动画帧序号
///
/// 返回:
/// - 可直接显示的完整状态行
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

/// 【终端】【状态动效】返回一轮文字流光包含的帧数。
///
/// 参数:
/// - `text`: 状态文字
///
/// 返回:
/// - 空文字返回 1，其余文字固定为约两秒一轮
pub(crate) fn activity_frame_count(text: &str) -> usize {
    if text.is_empty() {
        1
    } else {
        SHIMMER_CYCLE_FRAMES
    }
}

/// 【终端】【状态动效】计算指定字符在当前帧的余弦亮带强度。
///
/// 参数:
/// - `char_count`: 状态文字字符数量
/// - `frame`: 当前动画帧序号
/// - `index`: 目标字符下标
///
/// 返回:
/// - 0 到 1 之间的亮度强度
fn shimmer_intensity(char_count: usize, frame: usize, index: usize) -> f32 {
    let period = char_count.saturating_add(SHIMMER_PADDING * 2).max(1);
    // 1. 亮带中心按帧在整个周期上匀速推进，与 Codex 的时间扫描等价
    let position =
        ((frame % SHIMMER_CYCLE_FRAMES) as f32 / SHIMMER_CYCLE_FRAMES as f32 * period as f32)
            as isize;
    // 2. 计算目标字符与亮带中心的距离，带外不做混合
    let char_position = index.saturating_add(SHIMMER_PADDING) as isize;
    let distance = (char_position - position).abs() as f32;
    if distance > SHIMMER_BAND_HALF_WIDTH {
        return 0.0;
    }
    // 3. 使用余弦函数平滑衰减
    let phase = std::f32::consts::PI * (distance / SHIMMER_BAND_HALF_WIDTH);
    0.5 * (1.0 + phase.cos())
}

/// 【终端】【状态动效】按亮带强度混合基础色与高亮色。
///
/// 强度为 0 时返回基础色本身，即文字保持常态可读；强度越高越靠近高亮色，
/// 峰值处也保留一成基础色，避免文字完全失去原有色相。
///
/// 参数:
/// - `intensity`: 0 为常态、1 为亮带中心，超出范围时收敛到端点
///
/// 返回:
/// - 24 位真彩前景色序列
fn color_escape(intensity: f32) -> String {
    let ratio = intensity.clamp(0.0, 1.0) * SHIMMER_MAX_BLEND;
    let channel = |base: u8, highlight: u8| {
        (f32::from(base) + (f32::from(highlight) - f32::from(base)) * ratio).round() as u8
    };
    let red = channel(BASE_COLOR.0, HIGHLIGHT_COLOR.0);
    let green = channel(BASE_COLOR.1, HIGHLIGHT_COLOR.1);
    let blue = channel(BASE_COLOR.2, HIGHLIGHT_COLOR.2);
    format!("\x1b[1m\x1b[38;2;{red};{green};{blue}m")
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
        let early = brightest_index("Working", frame_for_index("Working", 0));
        let later = brightest_index("Working", frame_for_index("Working", 3));

        assert!(later > early, "高亮应向右推进: {early} -> {later}");
    }

    /// 【终端】【状态动效】验证亮带推进时亮度变化是平滑的。
    ///
    /// 亮带中心按字符位取整推进，相邻帧可能停在同一位置；这里比较真正
    /// 发生了移动的两帧，确认变化幅度不会一步跨过整个明暗区间。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn adjacent_frames_change_smoothly() {
        // 1. 同一字符在相继位置上的色差应远小于常态到高亮的跨度
        let span = f32::from(HIGHLIGHT_COLOR.2) - f32::from(BASE_COLOR.2);
        // 2. 采样点取亮带斜坡而非峰顶：峰顶附近相继帧都收敛到同一颜色
        let center = frame_for_index("Working", 1);
        let slope = center + SHIMMER_CYCLE_FRAMES / 12;
        let first = channel_at("Working", slope, 1);
        // 3. 找到亮度真正发生变化的下一帧，跳过亮带停在原位的重复帧
        let Some((next_frame, second)) = (slope + 1..slope + SHIMMER_CYCLE_FRAMES)
            .map(|frame| (frame, channel_at("Working", frame, 1)))
            .find(|(_, value)| *value != first)
        else {
            panic!("整个周期内亮度没有变化，动效停滞");
        };

        assert!(
            next_frame - slope <= 4,
            "亮带停在同一位置过久，观感会卡顿: {} 帧未变化",
            next_frame - slope
        );
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
    fn activity_animation_loops_after_padding() {
        let frames = activity_frame_count("Waiting");

        assert_eq!(
            render_activity_text("Waiting", 0),
            render_activity_text("Waiting", frames)
        );
        assert!(frames > "Waiting".chars().count());
    }

    /// 【终端】【状态动效】验证引导点在流光周期内明暗往复。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn guide_dot_uses_shimmer_cycle() {
        let samples = (0..SHIMMER_CYCLE_FRAMES)
            .map(render_activity_guide)
            .collect::<Vec<_>>();

        assert!(samples.iter().all(|dot| strip_ansi_for_test(dot) == "•"));
        assert!(
            samples
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                > 1,
            "引导点必须有明暗变化"
        );
        assert_eq!(
            render_activity_guide(0),
            render_activity_guide(SHIMMER_CYCLE_FRAMES),
            "流光应按周期循环"
        );
    }

    /// 【终端】【状态动效】验证流光只使用中性灰阶且峰值为白色。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn activity_shimmer_uses_white_grayscale() {
        let color = color_at("Thinking", frame_for_index("Thinking", 2), 2);

        assert_eq!(color.0, color.1);
        assert_eq!(color.1, color.2);
        // 峰值保留一成基础色，不会混到纯白
        assert!(color.2 > BASE_COLOR.2);
        assert!(color.2 < HIGHLIGHT_COLOR.2);
    }

    /// 【终端】【状态动效】验证亮带之外的文字保持基础色。
    ///
    /// 基础色是有意取暗的中性灰，亮带之外的字符应当稳定停在这个暗端，
    /// 不能比它更暗；这样扫光掠过时才有"暗字被点亮"的清晰对比。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn text_outside_the_band_keeps_the_base_color() {
        for frame in 0..SHIMMER_CYCLE_FRAMES {
            for index in 0.."Working".chars().count() {
                let value = channel_at("Working", frame, index);
                assert!(
                    value >= BASE_COLOR.2,
                    "第 {frame} 帧第 {index} 个字符暗于常态前景: {value} < {}",
                    BASE_COLOR.2
                );
            }
        }
    }

    /// 【终端】【状态动效】验证亮带峰值明显区别于常态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn highlight_is_clearly_brighter_than_the_base() {
        let span = f32::from(HIGHLIGHT_COLOR.2) - f32::from(BASE_COLOR.2);
        let peak = (0..SHIMMER_CYCLE_FRAMES)
            .flat_map(|frame| {
                (0.."Working".chars().count()).map(move |index| channel_at("Working", frame, index))
            })
            .max()
            .unwrap_or_default();

        assert!(
            f32::from(peak) - f32::from(BASE_COLOR.2) > span * 0.5,
            "亮带峰值与常态差异过小，动效不明显: {peak}"
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

    /// 返回亮带中心经过指定字符时的帧序号。
    ///
    /// 参数:
    /// - `text`: 状态文字
    /// - `index`: 字符下标
    ///
    /// 返回:
    /// - 最接近目标字符中心的帧序号
    fn frame_for_index(text: &str, index: usize) -> usize {
        let period = text.chars().count().saturating_add(SHIMMER_PADDING * 2);
        // 亮带中心位于 index + SHIMMER_PADDING 时对应的帧号
        let position = (index + SHIMMER_PADDING) as f32;
        let frame = position * SHIMMER_CYCLE_FRAMES as f32 / period.max(1) as f32;
        (frame.round() as usize) % SHIMMER_CYCLE_FRAMES
    }

    /// 读取指定字符在指定帧的蓝色通道值。
    ///
    /// 流光使用中性灰阶，任一颜色通道都可作为亮度观测量。
    ///
    /// 参数:
    /// - `text`: 状态文字
    /// - `frame`: 帧序号
    /// - `index`: 字符下标
    ///
    /// 返回:
    /// - 蓝色通道取值
    fn channel_at(text: &str, frame: usize, index: usize) -> u8 {
        color_at(text, frame, index).2
    }

    /// 读取指定字符在指定帧的 RGB 颜色。
    ///
    /// 参数:
    /// - `text`: 状态文字
    /// - `frame`: 帧序号
    /// - `index`: 字符下标
    ///
    /// 返回:
    /// - 红、绿、蓝三个通道值
    fn color_at(text: &str, frame: usize, index: usize) -> (u8, u8, u8) {
        let rendered = render_activity_text(text, frame);
        // 每个字符前都写入完整的前景色序列，按顺序取第 index 段
        rendered
            .split("\x1b[38;2;")
            .nth(index + 1)
            .and_then(|segment| segment.split('m').next())
            .and_then(|color| {
                let mut channels = color.split(';').filter_map(|value| value.parse().ok());
                Some((channels.next()?, channels.next()?, channels.next()?))
            })
            .unwrap_or_default()
    }
}

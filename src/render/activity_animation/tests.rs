use super::*;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const CYCLE_FRAMES: usize = 63;
const BRIGHT_CHANNEL: u8 = 239;
const MIN_TEXT_CHANNEL: u8 = 168;
const STATUS_LABELS: [&str; 8] = [
    "Thinking",
    "Working",
    "Waiting for response",
    "Waiting to run",
    "Waiting to write",
    "Compacting",
    "Reconnecting... 2/3",
    "Waiting",
];

/// 【终端】【动效测试】每一帧至少八成可见文字保持完整亮度。
/// 参数：无；返回：无。
#[test]
fn dark_columns_never_exceed_twenty_percent() {
    let labels = STATUS_LABELS
        .into_iter()
        .chain([
            "a",
            "abcd",
            "等待",
            "处理中",
            "正在分析代码结构",
            "Working 工具",
            "a b c d e f g",
            "e\u{301}clair",
            "",
            "   ",
        ])
        .map(str::to_string)
        .chain((1..=80).map(|len| "W".repeat(len)));

    for label in labels {
        let widths = label
            .graphemes(true)
            .map(|glyph| {
                if glyph.chars().all(char::is_whitespace) {
                    0
                } else {
                    UnicodeWidthStr::width(glyph)
                }
            })
            .collect::<Vec<_>>();
        let visible_width: usize = widths.iter().sum();
        for frame in 0..CYCLE_FRAMES {
            let rendered = render_activity_text(&label, frame);
            let colors = colors(&rendered);
            assert_eq!(strip_ansi_for_test(&rendered), label);
            assert_eq!(colors.len(), widths.len(), "{label:?}, frame={frame}");
            let dark_width: usize = colors
                .iter()
                .zip(&widths)
                .filter(|(color, _)| color.0 < BRIGHT_CHANNEL)
                .map(|(_, width)| width)
                .sum();
            assert!(
                dark_width * 5 <= visible_width,
                "{label:?}, frame={frame}: 暗区占 {dark_width}/{visible_width} 列"
            );
            for color in colors {
                assert_eq!(color.0, color.1);
                assert_eq!(color.1, color.2);
                assert!(
                    (MIN_TEXT_CHANNEL..=BRIGHT_CHANNEL).contains(&color.0),
                    "{label:?}, frame={frame}: 文字亮度超出可读范围 {color:?}"
                );
            }
        }
    }
}

/// 【终端】【动效测试】暗带依次经过单词中的字符，并保留明显亮度变化。
/// 参数：无；返回：无。
#[test]
fn dark_band_moves_from_left_to_right() {
    for label in ["Thinking", "Working", "Compacting"] {
        let darkest_frames = (0..label.len())
            .map(|index| {
                let (frame, color) = (0..CYCLE_FRAMES)
                    .map(|frame| (frame, colors(&render_activity_text(label, frame))[index].0))
                    .min_by_key(|(_, channel)| *channel)
                    .unwrap();
                assert!(color < 195, "{label}, index={index}: 暗带未经过此字符");
                frame
            })
            .collect::<Vec<_>>();
        assert!(
            darkest_frames.windows(2).all(|pair| pair[0] < pair[1]),
            "{label}: 暗带应向右推进 {darkest_frames:?}"
        );
    }
}

/// 【终端】【动效测试】文字逐帧平滑过渡，循环边界也不会闪烁。
/// 参数：无；返回：无。
#[test]
fn adjacent_frames_and_loop_boundary_change_smoothly() {
    for label in STATUS_LABELS {
        for frame in 0..CYCLE_FRAMES {
            let first = colors(&render_activity_text(label, frame));
            let next = colors(&render_activity_text(label, frame + 1));
            assert!(
                first
                    .iter()
                    .zip(next)
                    .all(|(before, after)| before.0.abs_diff(after.0) <= 40),
                "{label}, frame={frame}: 相邻帧亮度跳变过大"
            );
        }
        assert_eq!(
            render_activity_text(label, 0),
            render_activity_text(label, CYCLE_FRAMES)
        );
    }
}

/// 【终端】【动效测试】引导符始终保持同一单列字形，通过亮度呼吸表达活动。
/// 参数：无；返回：无。
#[test]
fn guide_pulses_without_changing_its_shape_or_width() {
    let first = render_activity_guide(0);
    let middle = render_activity_guide(CYCLE_FRAMES / 2);
    assert_ne!(first, middle);
    assert!(colors(&first)[0].0 > colors(&middle)[0].0);
    for frame in 0..CYCLE_FRAMES {
        let plain = strip_ansi_for_test(&render_activity_guide(frame));
        assert_eq!(plain, "▮");
        assert_eq!(UnicodeWidthStr::width(plain.as_str()), 1);
    }
    assert_eq!(first, render_activity_guide(CYCLE_FRAMES));
}

/// 【终端】【动效测试】指定状态颜色的引导符同样呼吸，并保持原有色相。
/// 参数：无；返回：无。
#[test]
fn colored_guides_keep_their_hue_and_still_animate() {
    let color = (204, 167, 0);
    let first = render_activity_guide_with_color(0, Some(color));
    let middle = render_activity_guide_with_color(CYCLE_FRAMES / 2, Some(color));
    assert_eq!(colors(&first), [color]);
    assert_ne!(first, middle);
    for frame in 0..CYCLE_FRAMES {
        let rendered = render_activity_guide_with_color(frame, Some(color));
        assert_eq!(strip_ansi_for_test(&rendered), "▮");
        let (red, green, blue) = colors(&rendered)[0];
        assert!(red >= 120 && green >= 95);
        assert_eq!(blue, 0);
        assert!((f32::from(red) / 204.0 - f32::from(green) / 167.0).abs() < 0.01);
    }
    assert_eq!(
        first,
        render_activity_guide_with_color(CYCLE_FRAMES, Some(color))
    );
}

/// 【终端】【动效测试】按真实时间换算帧号，不受调用方刷新次数影响。
/// 参数：无；返回：无。
#[test]
fn frames_advance_with_real_time() {
    assert_eq!(activity_frame_at(Duration::ZERO), 0);
    assert_eq!(activity_frame_at(ACTIVITY_FRAME_INTERVAL), 1);
    assert_eq!(activity_frame_at(ACTIVITY_FRAME_INTERVAL * 10), 10);
    assert_eq!(activity_frame_at(ACTIVITY_FRAME_INTERVAL * 3 / 2), 1);
}

/// 【终端】【动效测试】短文字保持亮度，完整状态行保留标签和详情。
/// 参数：无；返回：无。
#[test]
fn short_labels_and_line_details_remain_readable() {
    for frame in 0..CYCLE_FRAMES {
        for label in ["a", "abcd", "等待"] {
            assert!(colors(&render_activity_text(label, frame))
                .iter()
                .all(|color| color.0 == BRIGHT_CHANNEL));
        }
        let line = render_activity_line("Working", "2s", frame);
        assert_eq!(strip_ansi_for_test(&line), "▮ Working 2s");
        assert_eq!(
            strip_ansi_for_test(&render_activity_line("Working", "", frame)),
            "▮ Working"
        );
    }
    assert!(render_activity_text("", 0).is_empty());
}

/// 【终端】【动效测试】读取渲染结果中按显示顺序出现的 RGB 前景色。
/// 参数：`rendered` 为 ANSI 文本；返回：各字形的 RGB 颜色。
fn colors(rendered: &str) -> Vec<(u8, u8, u8)> {
    rendered
        .split("\x1b[38;2;")
        .skip(1)
        .map(|segment| {
            let value = segment.split('m').next().unwrap();
            let mut channels = value.split(';').map(|part| part.parse().unwrap());
            (
                channels.next().unwrap(),
                channels.next().unwrap(),
                channels.next().unwrap(),
            )
        })
        .collect()
}

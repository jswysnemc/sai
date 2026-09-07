use super::guide::render_guide_at;
use super::shimmer::render_shimmer_at;
use super::*;
use crate::render::terminal_palette::TerminalPalette;
use unicode_width::UnicodeWidthStr;

const DARK: TerminalPalette = TerminalPalette {
    foreground: (230, 220, 210),
    background: (20, 24, 28),
    true_color: true,
};
const LIGHT: TerminalPalette = TerminalPalette {
    foreground: (30, 40, 50),
    background: (245, 240, 235),
    true_color: true,
};

/// 【终端】【流光测试】核对 Codex 在一秒时的逐字符颜色，覆盖混色方向和取整。
/// 参数：无；返回：无。
#[test]
fn shimmer_matches_codex_samples_in_dark_and_light_themes() {
    let dark = [
        (230, 220, 210),
        (211, 203, 194),
        (164, 159, 153),
        (106, 104, 102),
        (59, 60, 61),
        (41, 43, 46),
        (59, 60, 61),
        (106, 104, 102),
        (164, 159, 153),
        (211, 203, 194),
    ];
    let light = [
        (30, 40, 50),
        (48, 57, 65),
        (96, 102, 107),
        (156, 157, 158),
        (205, 202, 200),
        (223, 220, 216),
        (205, 202, 200),
        (156, 157, 158),
        (96, 102, 107),
        (48, 57, 65),
    ];
    for (palette, expected) in [(DARK, dark), (LIGHT, light)] {
        let rendered = render_shimmer_at("0123456789", Duration::from_secs(1), palette);
        assert_eq!(colors(&rendered), expected);
        assert!(rendered.starts_with("\x1b[22m\x1b[1m"));
        assert!(rendered.ends_with(RESET));
        assert_eq!(strip_ansi_for_test(&rendered), "0123456789");
    }
}

/// 【终端】【流光测试】扫描带从左向右移动，两端缓冲使循环衔接保持常态前景。
/// 参数：无；返回：无。
#[test]
fn shimmer_sweeps_left_to_right_and_repeats_every_two_seconds() {
    for label in ["Thinking", "Working", "Waiting for response", "Compacting"] {
        let darkest_times = (0..label.chars().count())
            .map(|index| {
                (0..2000)
                    .step_by(8)
                    .min_by_key(|millis| {
                        colors(&render_shimmer_at(
                            label,
                            Duration::from_millis(*millis),
                            DARK,
                        ))[index]
                            .0
                    })
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert!(
            darkest_times.windows(2).all(|pair| pair[0] < pair[1]),
            "{label}: {darkest_times:?}"
        );
        for millis in [0, 192, 832, 1200, 1984] {
            let elapsed = Duration::from_millis(millis);
            assert_eq!(
                render_shimmer_at(label, elapsed, DARK),
                render_shimmer_at(label, elapsed + Duration::from_secs(2), DARK),
            );
        }
        for millis in [0, 1984, 2000] {
            let frame_colors = colors(&render_shimmer_at(
                label,
                Duration::from_millis(millis),
                DARK,
            ));
            assert!(frame_colors.iter().all(|color| *color == DARK.foreground));
        }
    }
}

/// 【终端】【流光测试】低色彩终端采用弱化、正常、加粗三档，并逐字清理继承样式。
/// 参数：无；返回：无。
#[test]
fn shimmer_ansi_fallback_matches_codex_levels() {
    let palette = TerminalPalette {
        true_color: false,
        ..DARK
    };
    let rendered = render_shimmer_at("0123456789", Duration::from_secs(1), palette);
    let levels = [
        "\x1b[2m", "\x1b[2m", "", "\x1b[1m", "\x1b[1m", "\x1b[1m", "\x1b[1m", "\x1b[1m", "",
        "\x1b[2m",
    ];
    let expected = levels
        .iter()
        .enumerate()
        .map(|(index, level)| format!("\x1b[22m\x1b[39m{level}{index}"))
        .collect::<String>()
        + RESET;
    assert_eq!(rendered, expected);
    assert!(!rendered.contains("38;2"));
}

/// 【终端】【流光测试】短文字、汉字和组合字符在整个扫描周期保持内容与显示宽度。
/// 参数：无；返回：无。
#[test]
fn shimmer_preserves_short_and_unicode_labels() {
    for label in [
        "",
        "a",
        "等待",
        "正在分析代码结构",
        "Working 工具",
        "e\u{301}clair",
        "   ",
    ] {
        for palette in [
            DARK,
            LIGHT,
            TerminalPalette {
                true_color: false,
                ..DARK
            },
        ] {
            for frame in 0..126 {
                let elapsed = ACTIVITY_FRAME_INTERVAL * frame;
                let rendered = render_shimmer_at(label, elapsed, palette);
                let plain = strip_ansi_for_test(&rendered);
                assert_eq!(plain, label);
                assert_eq!(
                    UnicodeWidthStr::width(plain.as_str()),
                    UnicodeWidthStr::width(label)
                );
            }
        }
    }
    assert!(render_shimmer_at("", Duration::ZERO, DARK).is_empty());
}

/// 【终端】【引导符测试】圆点全周期保持统一单列字形，不通过粗体改变轮廓。
/// 参数：无；返回：无。
#[test]
fn guide_keeps_the_existing_circle_and_column_width() {
    assert_eq!(ACTIVITY_GUIDE, crate::render::style::GUIDE_FILLED);
    for palette in [
        DARK,
        LIGHT,
        TerminalPalette {
            true_color: false,
            ..DARK
        },
    ] {
        for frame in 0..50 {
            let rendered = render_guide_at(frame, None, palette);
            let plain = strip_ansi_for_test(&rendered);
            assert_eq!(plain, "●");
            assert_eq!(UnicodeWidthStr::width(plain.as_str()), 1);
            assert!(!rendered.contains("\x1b[1m"));
            assert_eq!(rendered, render_guide_at(frame + 50, None, palette));
        }
    }
}

/// 【终端】【引导符测试】每轮包含一强一弱两个平滑脉冲，停歇时仍保留六成对比度。
/// 参数：无；返回：无。
#[test]
fn guide_has_two_smooth_pulses_and_a_visible_rest() {
    let channels = (0..50)
        .map(|frame| colors(&render_guide_at(frame, None, DARK))[0].0)
        .collect::<Vec<_>>();
    let peaks = (1..49)
        .filter(|index| {
            channels[*index] > channels[index - 1] && channels[*index] > channels[index + 1]
        })
        .collect::<Vec<_>>();
    assert_eq!(peaks, [8, 22]);
    assert!(channels[8] > channels[22] && channels[22] > channels[0]);
    assert_eq!(channels[8], DARK.foreground.0);
    assert!(channels.iter().all(|channel| *channel >= 146));
    for frame in 0..50 {
        assert!(channels[frame].abs_diff(channels[(frame + 1) % 50]) <= 18);
    }
    assert!(channels[30..].iter().all(|channel| *channel == channels[0]));
    let dark_ink = TerminalPalette {
        foreground: (0, 0, 0),
        ..LIGHT
    };
    assert_ne!(
        render_guide_at(0, None, dark_ink),
        render_guide_at(8, None, dark_ink)
    );
}

/// 【终端】【引导符测试】有色任务在峰值保持原色，降级后仍保留黄色和青色的状态语义。
/// 参数：无；返回：无。
#[test]
fn colored_guides_keep_status_colors_in_both_color_modes() {
    for (color, ansi) in [((204, 167, 0), 33), ((86, 182, 194), 36)] {
        for palette in [DARK, LIGHT] {
            assert_eq!(colors(&render_guide_at(8, Some(color), palette)), [color]);
            assert_ne!(
                render_guide_at(0, Some(color), palette),
                render_guide_at(8, Some(color), palette)
            );
            let fallback = TerminalPalette {
                true_color: false,
                ..palette
            };
            assert_eq!(
                render_guide_at(0, Some(color), fallback),
                format!("\x1b[22m\x1b[{ansi}m\x1b[2m●{RESET}")
            );
            assert_eq!(
                render_guide_at(8, Some(color), fallback),
                format!("\x1b[22m\x1b[{ansi}m●{RESET}")
            );
        }
    }
}

/// 【终端】【动效测试】按真实时间换算帧号，各入口共用时钟起点。
/// 参数：无；返回：无。
#[test]
fn frames_advance_with_real_time_from_one_clock() {
    assert_eq!(activity_frame_at(Duration::ZERO), 0);
    assert_eq!(activity_frame_at(ACTIVITY_FRAME_INTERVAL), 1);
    assert_eq!(activity_frame_at(ACTIVITY_FRAME_INTERVAL * 10), 10);
    assert_eq!(activity_frame_at(ACTIVITY_FRAME_INTERVAL * 3 / 2), 1);
    assert_eq!(activity_started_at(), activity_started_at());
}

/// 【终端】【动效测试】组合行保留标题、详情和固定的引导间隔。
/// 参数：无；返回：无。
#[test]
fn activity_line_keeps_labels_and_details() {
    for frame in 0..126 {
        assert_eq!(
            strip_ansi_for_test(&render_activity_line("Working", "2s", frame)),
            "● Working 2s"
        );
        assert_eq!(
            strip_ansi_for_test(&render_activity_line("Working", "", frame)),
            "● Working"
        );
    }
    assert_eq!(blend_color((0, 0, 0), (1, 3, 5), 0.5), (0, 1, 2));
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

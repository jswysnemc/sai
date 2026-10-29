use super::face::{face_at, starts_with_face, BLINK_FACE, DEFAULT_FACE, GLANCE_FACES};
use super::*;
use crate::render::activity_animation::render_status_line;
use crate::render::activity_animation::strip_ansi_for_test;
use unicode_width::UnicodeWidthStr;

/// 【终端】【状态引导测试】全部表情在中西文宽度规则下都是五列，帧间切换不推动标题。
/// 参数：无；返回：无。
#[test]
fn companion_keeps_one_width_in_every_frame_and_locale() {
    for face in GLANCE_FACES.iter().chain([&DEFAULT_FACE, &BLINK_FACE]) {
        assert_eq!(UnicodeWidthStr::width(*face), 5, "{face}");
        assert_eq!(UnicodeWidthStr::width_cjk(*face), 5, "{face}");
    }
    for frame in 0..2000 {
        let rendered = render_status_companion(frame);
        let plain = strip_ansi_for_test(&rendered);
        assert_eq!(UnicodeWidthStr::width(plain.as_str()), 5);
        assert!(starts_with_face(&plain), "{plain}");
        assert!(!rendered.contains("38;2") && !rendered.contains("\x1b[1m"));
    }
}

/// 【终端】【状态引导测试】状态行由表情、一个空格和流光标题组成，不再带其他动效字形。
/// 参数：无；返回：无。
#[test]
fn status_line_is_face_then_shimmer_label() {
    for frame in [0, 9, 33, 64, 500] {
        let plain = strip_ansi_for_test(&render_status_line("Thinking", "2s", frame));
        assert_eq!(plain, format!("{} Thinking 2s", face_at(frame)));
    }
}

/// 【终端】【状态表情测试】眨眼短暂且间隔不规则，常态和各种神情都会出现。
/// 参数：无；返回：无。
#[test]
fn face_blinks_briefly_at_irregular_intervals() {
    let faces = (0..4100).map(face_at).collect::<Vec<_>>();
    let blink_starts = (1..4000)
        .filter(|index| faces[*index] == BLINK_FACE && faces[index - 1] != BLINK_FACE)
        .collect::<Vec<_>>();
    assert!(blink_starts.len() > 20, "{blink_starts:?}");
    let gaps = blink_starts
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect::<Vec<_>>();
    // 间隔下限为 40 帧，相邻两次眨眼至少相隔约 1.3 秒
    assert!(gaps.iter().all(|gap| *gap >= 40), "{gaps:?}");
    assert!(gaps.iter().any(|gap| *gap != gaps[0]), "{gaps:?}");
    for start in &blink_starts {
        assert!(faces[*start..*start + 3]
            .iter()
            .all(|face| *face == BLINK_FACE));
        assert_ne!(faces[start + 3], BLINK_FACE);
    }
    for face in GLANCE_FACES.iter().chain([&DEFAULT_FACE]) {
        assert!(faces.contains(face), "{face}");
    }
}

/// 【终端】【状态引导测试】只把完整表情开头的行识别为状态行，带括号的普通正文不受影响。
/// 参数：无；返回：无。
#[test]
fn status_companion_detection_requires_a_full_face() {
    for frame in [0, 9, 33, 64] {
        let line = render_status_line("Working", "3s", frame);
        assert!(is_status_companion_line(&line));
        assert!(is_status_companion_line(&format!("  {line}")));
        let plain = strip_ansi_for_test(&line);
        assert!(plain.ends_with(" Working 3s"), "{plain}");
    }
    assert!(strip_ansi_for_test(&render_status_line("Working", "", 0)).ends_with(" Working"));
    for text in ["", "(note) text", "smile ‿ here", "(◕) x", "text (◕‿◕)"] {
        assert!(!is_status_companion_line(text), "{text}");
    }
}

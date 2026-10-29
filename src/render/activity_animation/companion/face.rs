//! 状态行表情排期：不规则眨眼，间或切换神情，全部表情等宽。

/// 常态表情，也是神情排期中出现最多的一种
pub(super) const DEFAULT_FACE: &str = "(◕‿◕)";
/// 眨眼表情
pub(super) const BLINK_FACE: &str = "(-‿-)";
/// 偶尔出现的神情；字形均非东亚宽度不定字符，且在常用编程字体中有收录，切换时不改变列宽
pub(super) const GLANCE_FACES: [&str; 3] = ["(◔‿◔)", "(∙‿∙)", "(^‿^)"];
/// 全部表情共用的嘴形，供行首识别先做廉价过滤
pub(super) const MOUTH: char = '‿';

/// 两次眨眼之间的最短与浮动间隔，按 32ms 一帧约为 1.3 到 2.6 秒
const BLINK_GAP_MIN: usize = 40;
const BLINK_GAP_SPREAD: usize = 40;
/// 每次眨眼持续约 0.1 秒
const BLINK_FRAMES: usize = 3;
/// 每段神情持续约 1 到 3 秒
const MOOD_MIN: usize = 30;
const MOOD_SPREAD: usize = 64;
/// 排期分段数；周期约半分钟，重复不易察觉，单帧计算量固定
const SCHEDULE_SEGMENTS: u32 = 24;
const BLINK_SALT: u32 = 0x5bd1;
const MOOD_SALT: u32 = 0x27d4;

/// 【终端】【状态表情】按帧号选择表情，眨眼优先于神情切换。
/// 参数：`frame` 为共享时钟帧号；返回：固定五列的表情字符串。
pub(super) fn face_at(frame: usize) -> &'static str {
    // 1. 【终端】【状态表情】每段间隔的末尾几帧闭眼，间隔长度由哈希决定
    let (offset, span, _) = schedule_segment(frame, BLINK_SALT, BLINK_GAP_MIN, BLINK_GAP_SPREAD);
    if offset + BLINK_FRAMES >= span {
        return BLINK_FACE;
    }
    // 2. 【终端】【状态表情】神情段约一半保持常态，其余在偶发神情中轮换
    let (_, _, hash) = schedule_segment(frame, MOOD_SALT, MOOD_MIN, MOOD_SPREAD);
    let choice = (hash / 97) as usize % (GLANCE_FACES.len() * 2);
    GLANCE_FACES.get(choice).copied().unwrap_or(DEFAULT_FACE)
}

/// 【终端】【状态表情】判断纯文本是否以任一表情开头。
/// 参数：`plain` 为已去除样式与行首空白的文本；返回：是否为状态表情开头。
pub(super) fn starts_with_face(plain: &str) -> bool {
    plain.starts_with(DEFAULT_FACE)
        || plain.starts_with(BLINK_FACE)
        || GLANCE_FACES.iter().any(|face| plain.starts_with(face))
}

/// 【终端】【状态表情】在固定段数的循环排期中定位当前帧，避免逐帧回溯全部历史。
/// 参数：`frame` 为帧号，`salt` 区分排期，`min`/`spread` 为段长下限与浮动；返回：段内偏移、段长和段哈希。
fn schedule_segment(frame: usize, salt: u32, min: usize, spread: usize) -> (usize, usize, u32) {
    let segment = |index: u32| {
        let hash = mix(index ^ salt);
        (min + hash as usize % spread, hash)
    };
    let cycle: usize = (0..SCHEDULE_SEGMENTS).map(|index| segment(index).0).sum();
    let mut offset = frame % cycle;
    for index in 0..SCHEDULE_SEGMENTS {
        let (span, hash) = segment(index);
        if offset < span {
            return (offset, span, hash);
        }
        offset -= span;
    }
    // 偏移已对周期取模，循环必然在最后一段之前返回
    (0, min, 0)
}

/// 【终端】【状态表情】整数哈希，把段序号打散成不规则段长与神情。
/// 参数：`value` 为段序号与盐值的组合；返回：打散后的整数。
fn mix(value: u32) -> u32 {
    let value = ((value >> 16) ^ value).wrapping_mul(0x045d_9f3b);
    let value = ((value >> 16) ^ value).wrapping_mul(0x045d_9f3b);
    (value >> 16) ^ value
}

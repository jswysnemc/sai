/// 【终端】【微型进度条】根据当前视口位置与总行数生成紧凑的字符滑块。
///
/// 参数:
/// - `first`: 起始行号（从 1 起始）
/// - `last`: 结束行号
/// - `total`: 总行数
/// - `slots`: 滑块总列数
///
/// 返回:
/// - 固定列宽的纯文本滑块，如 `[━━──]`
pub(crate) fn render_scroll_track(first: usize, last: usize, total: usize, slots: usize) -> String {
    if total == 0 || slots == 0 {
        return format!("[{}]", "─".repeat(slots.max(1)));
    }

    // 1. 【终端】【比例映射】将行范围映射为离散槽位区间
    let slots_f = slots as f64;
    let total_f = total as f64;
    let start_ratio = (first.saturating_sub(1) as f64 / total_f).clamp(0.0, 1.0);
    let end_ratio = (last as f64 / total_f).clamp(0.0, 1.0);

    let start_slot = ((start_ratio * slots_f).floor() as usize).min(slots.saturating_sub(1));
    let end_slot = ((end_ratio * slots_f).ceil() as usize).clamp(start_slot + 1, slots);

    // 2. 【终端】【符号装配】活跃区间采用粗线，非活跃区间采用细线
    let mut bar = String::with_capacity(slots * 3 + 2);
    bar.push('[');
    for slot in 0..slots {
        if slot >= start_slot && slot < end_slot {
            bar.push('━');
        } else {
            bar.push('─');
        }
    }
    bar.push(']');
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_scroll_track_boundaries() {
        // 顶部
        assert_eq!(render_scroll_track(1, 10, 40, 4), "[━───]");
        // 中间
        assert_eq!(render_scroll_track(20, 30, 40, 4), "[─━━─]");
        // 底部
        assert_eq!(render_scroll_track(31, 40, 40, 4), "[───━]");
        // 全量覆盖
        assert_eq!(render_scroll_track(1, 40, 40, 4), "[━━━━]");
    }
}

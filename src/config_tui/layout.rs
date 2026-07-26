//! 配置界面纯布局计算：宽度钳制、滚动窗口和多栏分配。

/// 计算面板宽度，保证不超过终端可用宽度。
///
/// 参数:
/// - `desired`: 期望宽度
/// - `min_width`: 期望的最小宽度
/// - `available`: 终端可用宽度
///
/// 返回:
/// - 先按最小宽度抬升期望值，再钳制到可用宽度后的结果
pub(crate) fn panel_width(desired: u16, min_width: u16, available: u16) -> u16 {
    desired.max(min_width).min(available)
}

/// 计算列表滚动窗口起始下标，使选中项保持可见。
///
/// 参数:
/// - `selected`: 当前选中项下标
/// - `visible_rows`: 可见行数
///
/// 返回:
/// - 滚动窗口内第一项的下标
pub(crate) fn scroll_start(selected: usize, visible_rows: usize) -> usize {
    selected.saturating_sub(visible_rows.saturating_sub(1))
}

/// 计算供应商浏览器三栏宽度。
///
/// 参数:
/// - `inner_w`: 内容区总宽度，含两列分隔符
///
/// 返回:
/// - 左、中、右三栏宽度；终端过窄无法容纳三栏时返回空
pub(crate) fn three_column_widths(inner_w: u16) -> Option<(u16, u16, u16)> {
    // 1. 预留两列分隔符
    let usable = inner_w.checked_sub(2)?;
    // 2. 左栏 28%、中栏 22%，剩余给右栏，保证总和不超可用宽度
    let left = usable.saturating_mul(28) / 100;
    let mid = usable.saturating_mul(22) / 100;
    let right = usable.saturating_sub(left).saturating_sub(mid);
    // 3. 任一栏过窄则降级为单栏布局
    if left < 12 || mid < 8 || right < 12 {
        return None;
    }
    Some((left, mid, right))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证宽面板在宽终端下保留期望宽度。
    #[test]
    fn panel_width_keeps_desired_on_wide_terminal() {
        assert_eq!(panel_width(64, 56, 120), 64);
    }

    /// 验证期望值低于最小宽度时被抬升。
    #[test]
    fn panel_width_raises_to_min_width() {
        assert_eq!(panel_width(30, 56, 120), 56);
    }

    /// 验证窄终端下钳制到可用宽度，最小宽度不得撑破终端。
    #[test]
    fn panel_width_clamps_to_available_on_narrow_terminal() {
        assert_eq!(panel_width(96, 48, 40), 40);
        assert_eq!(panel_width(30, 56, 20), 20);
    }

    /// 验证选中项在窗口内时不滚动。
    #[test]
    fn scroll_start_stays_at_top_within_window() {
        assert_eq!(scroll_start(0, 5), 0);
        assert_eq!(scroll_start(4, 5), 0);
    }

    /// 验证选中项超出窗口后跟随滚动，选中项落在窗口末行。
    #[test]
    fn scroll_start_follows_selection() {
        assert_eq!(scroll_start(5, 5), 1);
        assert_eq!(scroll_start(9, 5), 5);
    }

    /// 验证可见行数为零时不发生下溢。
    #[test]
    fn scroll_start_handles_zero_rows() {
        assert_eq!(scroll_start(3, 0), 3);
    }

    /// 验证宽终端下三栏宽度之和不超过可用宽度。
    #[test]
    fn three_columns_fit_within_inner_width() {
        let (left, mid, right) = three_column_widths(120).expect("宽终端应支持三栏");
        assert_eq!(left + mid + right, 118);
        assert!(left >= 12 && mid >= 8 && right >= 12);
    }

    /// 验证窄终端下降级为单栏。
    #[test]
    fn three_columns_degrade_on_narrow_terminal() {
        assert_eq!(three_column_widths(30), None);
        assert_eq!(three_column_widths(1), None);
    }
}

//! 配置界面纯布局计算：宽度钳制、滚动窗口和多栏分配。

/// 近全屏内容框的矩形区域。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FrameRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

/// 计算近全屏内容框：只留一列边距，尽量吃满终端。
///
/// 参数:
/// - `cols`: 终端列数
/// - `rows`: 终端行数
///
/// 返回:
/// - 可用于绘制圆角面板的矩形
pub(crate) fn full_frame(cols: u16, rows: u16) -> FrameRect {
    let margin_x: u16 = if cols > 40 { 1 } else { 0 };
    let margin_y: u16 = 0;
    let width = cols.saturating_sub(margin_x.saturating_mul(2)).max(1);
    let height = rows.saturating_sub(margin_y.saturating_mul(2)).max(1);
    FrameRect {
        x: margin_x,
        y: margin_y,
        width,
        height,
    }
}

/// 在全屏框内划分「左侧列表 + 右侧说明」两栏。
///
/// 参数:
/// - `inner_w`: 去掉左右边框后的内容宽度
///
/// 返回:
/// - `(左栏宽, 右栏宽)`；过窄时右栏为 0（单栏）
pub(crate) fn master_detail_widths(inner_w: u16) -> (u16, u16) {
    if inner_w < 48 {
        return (inner_w, 0);
    }
    let gap = 2u16;
    let left = (inner_w.saturating_mul(38) / 100).clamp(18, 42);
    let right = inner_w.saturating_sub(left).saturating_sub(gap);
    if right < 16 {
        (inner_w, 0)
    } else {
        (left, right)
    }
}

/// 计算面板宽度，保证不超过终端可用宽度。
///
/// 参数:
/// - `desired`: 期望宽度
/// - `min_width`: 期望的最小宽度
/// - `available`: 终端可用宽度
///
/// 返回:
/// - 先按最小宽度抬升期望值，再钳制到可用宽度后的结果
#[cfg(test)]
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

    /// 验证全屏框几乎吃满终端。
    #[test]
    fn full_frame_uses_most_of_the_terminal() {
        let frame = full_frame(120, 40);
        assert_eq!(frame.width, 118);
        assert_eq!(frame.height, 40);
        assert!(frame.width * 100 / 120 >= 95);
    }

    /// 验证主从布局在宽终端分栏、窄终端合并。
    #[test]
    fn master_detail_splits_on_wide_terminals() {
        let (left, right) = master_detail_widths(100);
        assert!(left > 0 && right > 0);
        assert_eq!(left + right + 2, 100);
        let (only, none) = master_detail_widths(40);
        assert_eq!(only, 40);
        assert_eq!(none, 0);
    }
}

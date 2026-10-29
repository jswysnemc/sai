/// 提问面板最多占用的终端行数。
pub(crate) const MAX_PANEL_LINES: u16 = 16;

/// 【终端提问】【底部覆盖】覆盖层在当前屏幕中的固定区域。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayPlacement {
    /// 终端列数
    pub cols: u16,
    /// 终端行数
    pub rows: u16,
    /// 覆盖层首行
    pub anchor_y: u16,
    /// 覆盖层占用行数
    pub panel_lines: u16,
    /// 内联模式的正文尾部位置，窗口变高时恢复此锚点
    pub(crate) inline_anchor: Option<u16>,
}

impl OverlayPlacement {
    /// 【终端面板】【内联承接】在正文后接替输入区，空间不足时仅向上让出所需高度。
    /// 参数：终端宽高和输入区首行；返回：不超出屏幕的面板区域。
    pub fn inline(cols: u16, rows: u16, composer_top: u16) -> Self {
        let mut placement = Self::bottom(cols, rows);
        placement.anchor_y = composer_top.min(placement.anchor_y);
        placement.inline_anchor = Some(composer_top);
        placement
    }

    /// 【终端提问】【底部覆盖】参数为终端列行，返回贴底的覆盖区域。
    ///
    /// 返回:
    /// - 锚点为 `rows - panel_lines` 的固定区域
    pub fn bottom(cols: u16, rows: u16) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let panel_lines = rows.saturating_sub(1).clamp(1, MAX_PANEL_LINES);
        Self {
            cols,
            rows,
            anchor_y: rows.saturating_sub(panel_lines),
            panel_lines,
            inline_anchor: None,
        }
    }

    /// 【终端提问】【底部覆盖】参数为终端尺寸与输入槽，返回覆盖该槽的区域。
    ///
    /// 返回:
    /// - 与全屏底部输入槽对齐的覆盖区域
    pub fn slot(cols: u16, rows: u16, composer_top: u16, composer_height: u16) -> Self {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let available = rows.saturating_sub(composer_top).max(1);
        let panel_lines = composer_height.clamp(1, available.min(MAX_PANEL_LINES));
        Self {
            cols,
            rows,
            anchor_y: composer_top.min(rows.saturating_sub(panel_lines)),
            panel_lines,
            inline_anchor: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{OverlayPlacement, MAX_PANEL_LINES};

    /// 【终端面板】【内联锚点】短屏临时贴底仍保留正文位置，避免放大后出现空白；无参数或返回值。
    #[test]
    fn inline_placement_remembers_anchor_when_clamped() {
        let placement = OverlayPlacement::inline(80, 24, 12);
        assert_eq!(placement.anchor_y, 8);
        assert_eq!(placement.inline_anchor, Some(12));
    }

    /// 【终端提问】【底部覆盖】24 行终端贴底占用上限行，历史区留在上方。
    #[test]
    fn bottom_pins_panel_to_last_rows() {
        let placement = OverlayPlacement::bottom(80, 24);
        assert_eq!(placement.panel_lines, MAX_PANEL_LINES);
        assert_eq!(placement.anchor_y, 8);
        assert_eq!(placement.anchor_y + placement.panel_lines, 24);
    }

    /// 【终端提问】【底部覆盖】全屏输入槽决定覆盖起点，不向上滚出行。
    #[test]
    fn slot_covers_composer_region() {
        let placement = OverlayPlacement::slot(80, 24, 8, 16);
        assert_eq!(placement.anchor_y, 8);
        assert_eq!(placement.panel_lines, 16);
    }

    /// 【终端提问】【底部覆盖】矮窗口仍保留至少一行，且不超过终端高度。
    #[test]
    fn tiny_terminal_keeps_one_row_panel() {
        let placement = OverlayPlacement::bottom(40, 2);
        assert_eq!(placement.panel_lines, 1);
        assert_eq!(placement.anchor_y, 1);
        let slot = OverlayPlacement::slot(40, 10, 7, 30);
        assert_eq!(slot.anchor_y, 7);
        assert_eq!(slot.panel_lines, 3);
    }
}

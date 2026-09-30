//! 全屏会话视图的屏幕分区：顶部浮动标题、正文、概览轨道、滚动条与底部输入框。

/// 正文区至少保留的行数，输入框再高也不能把正文挤没。
const MIN_BODY_ROWS: u16 = 3;
/// 显示概览轨道所需的最小终端列数。
const RAIL_MIN_COLS: u16 = 60;

/// 一帧的屏幕分区，所有坐标从 0 开始。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FullscreenLayout {
    /// 终端列数
    pub(super) cols: u16,
    /// 终端行数
    pub(super) rows: u16,
    /// 正文首行（浮动标题占第 0 行）
    pub(super) body_top: u16,
    /// 正文行数
    pub(super) body_height: u16,
    /// 输入框首行
    pub(super) composer_top: u16,
    /// 输入框行数
    pub(super) composer_height: u16,
    /// 正文渲染宽度
    pub(super) content_width: u16,
    /// 概览轨道所在列；窄终端不显示
    pub(super) rail_col: Option<u16>,
    /// 滚动条所在列
    pub(super) scrollbar_col: u16,
}

impl FullscreenLayout {
    /// 按终端尺寸与输入框期望高度划分屏幕。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    /// - `rows`: 终端行数
    /// - `composer_wanted`: 输入框期望行数；无输入框时为 0
    ///
    /// 返回:
    /// - 屏幕分区
    pub(super) fn compute(cols: u16, rows: u16, composer_wanted: u16) -> Self {
        let cols = cols.max(8);
        let rows = rows.max(2);
        // 1. 标题固定一行，输入框贴底，正文保底 MIN_BODY_ROWS 行
        let header = 1u16;
        let composer_cap = rows.saturating_sub(header + MIN_BODY_ROWS.min(rows - header));
        let composer_height = composer_wanted.min(composer_cap);
        let composer_top = rows - composer_height;
        let body_height = composer_top.saturating_sub(header).max(1);
        // 2. 右侧依次为间隔列、概览轨道、滚动条；窄终端只保留滚动条
        let (content_width, rail_col) = if cols >= RAIL_MIN_COLS {
            (cols - 4, Some(cols - 3))
        } else {
            (cols - 1, None)
        };
        Self {
            cols,
            rows,
            body_top: header,
            body_height,
            composer_top,
            composer_height,
            content_width,
            rail_col,
            scrollbar_col: cols - 1,
        }
    }

    /// 判断屏幕行是否位于正文区。
    ///
    /// 参数:
    /// - `row`: 屏幕行
    ///
    /// 返回:
    /// - 位于正文区时为 true
    pub(super) fn in_body(&self, row: u16) -> bool {
        row >= self.body_top && row < self.body_top + self.body_height
    }

    /// 判断屏幕列是否位于概览轨道（含两列标记宽度）。
    ///
    /// 参数:
    /// - `col`: 屏幕列
    ///
    /// 返回:
    /// - 位于轨道时为 true
    pub(super) fn in_rail(&self, col: u16) -> bool {
        self.rail_col
            .is_some_and(|rail| col >= rail && col < rail + 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 宽终端：标题一行、正文居中、输入框贴底，右侧留出轨道与滚动条。
    #[test]
    fn wide_terminal_splits_header_body_composer_and_gutter() {
        let layout = FullscreenLayout::compute(120, 40, 6);
        assert_eq!(layout.body_top, 1);
        assert_eq!(layout.composer_top, 34);
        assert_eq!(layout.body_height, 33);
        assert_eq!(layout.content_width, 116);
        assert_eq!(layout.rail_col, Some(117));
        assert_eq!(layout.scrollbar_col, 119);
        assert!(layout.in_rail(118) && !layout.in_rail(119));
    }

    /// 输入框再高也要给正文留出保底行数。
    #[test]
    fn composer_never_swallows_the_body() {
        let layout = FullscreenLayout::compute(80, 10, 30);
        assert_eq!(layout.body_height, MIN_BODY_ROWS);
        assert_eq!(layout.composer_top + layout.composer_height, 10);
    }

    /// 窄终端省略概览轨道，正文只让出滚动条一列。
    #[test]
    fn narrow_terminal_drops_the_rail() {
        let layout = FullscreenLayout::compute(40, 20, 4);
        assert_eq!(layout.rail_col, None);
        assert_eq!(layout.content_width, 39);
    }
}

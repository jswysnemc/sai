//! 后台任务面板在终端中的矩形区域，绘制位置与鼠标命中共用同一组坐标。

/// 面板外框所在的终端矩形，坐标从 0 开始
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::cli) struct PanelRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl PanelRect {
    /// 【后台任务】【面板区域】占满给定行段，左右各留一列显出底层内容，窄终端不留边。
    /// 参数: cols 为终端列数，top 为首行，height 为可用行数；返回: 面板矩形。
    pub fn inset(cols: u16, top: u16, height: u16) -> Self {
        let margin = if cols >= 40 { 1 } else { 0 };
        Self {
            x: margin,
            y: top,
            width: cols.saturating_sub(margin * 2),
            height,
        }
    }

    /// 【后台任务】【区域命中】判断终端坐标是否落在面板外框范围内，面板外的滚轮不滚动列表。
    /// 参数: column/row 为终端坐标；返回: 是否在面板内。
    pub fn contains(self, column: u16, row: u16) -> bool {
        column >= self.x
            && column < self.x.saturating_add(self.width)
            && row >= self.y
            && row < self.y.saturating_add(self.height)
    }
}

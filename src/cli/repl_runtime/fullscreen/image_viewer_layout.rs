//! 图片浮窗边界与实际图片范围，供绘制和鼠标命中共用。

use super::layout::FullscreenLayout;

#[derive(Clone, Copy, Debug)]
pub(super) struct ImageArea {
    pub(super) left: usize,
    pub(super) top: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

impl ImageArea {
    /// 【图片浮窗】【图片命中】检查鼠标是否落在实际绘制的图片矩形内。
    /// 参数: column/row 为屏幕坐标；返回: 是否命中图片。
    pub(super) fn contains(self, column: u16, row: u16) -> bool {
        (self.left..self.left + self.width).contains(&usize::from(column))
            && (self.top..self.top + self.height).contains(&usize::from(row))
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ViewerLayout {
    pub(super) left: usize,
    pub(super) top: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

impl ViewerLayout {
    /// 【图片浮窗】【自适应布局】使用正文区，保留原来的顶部导航与底部输入区。
    /// 参数: layout 为主界面分区；返回: 浮窗边界，尺寸不足时使用紧凑布局。
    pub(super) fn compute(layout: &FullscreenLayout) -> Self {
        let columns = usize::from(layout.cols);
        let rows = usize::from(layout.body_height);
        let width = columns
            .saturating_sub(if columns >= 16 { 4 } else { 0 })
            .min(124);
        let height = rows.saturating_sub(if rows >= 8 { 2 } else { 0 });
        Self {
            left: (columns - width) / 2,
            top: usize::from(layout.body_top) + (rows - height) / 2,
            width,
            height,
        }
    }

    /// 【图片浮窗】【图片定位】按实际渲染尺寸居中，裁剪范围与可用正文一致。
    /// 参数: width/height 为图片列行数；返回: 屏幕上的图片矩形。
    pub(super) fn image_area(self, width: usize, height: usize) -> ImageArea {
        let width = width.min(self.width.saturating_sub(4).max(1));
        let height = height.min(self.height.saturating_sub(2));
        ImageArea {
            left: self.left + 1 + self.width.saturating_sub(width + 2) / 2,
            top: self.top + 1 + self.height.saturating_sub(height + 2) / 2,
            width,
            height,
        }
    }

    /// 【图片浮窗】【关闭命中】右上角三列按钮与显示范围完全一致。
    /// 参数: column/row 为鼠标坐标；返回: 是否命中关闭按钮。
    pub(super) fn close_at(self, column: u16, row: u16) -> bool {
        usize::from(row) == self.top
            && usize::from(column) >= self.left + self.width.saturating_sub(4)
            && usize::from(column) < self.left + self.width.saturating_sub(1)
    }
}

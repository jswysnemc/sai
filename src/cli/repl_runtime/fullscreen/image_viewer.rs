//! 图片浮窗只保存查看状态，不修改聊天位置、段落展开或输入草稿。

use std::sync::Arc;

#[derive(Default)]
pub(super) struct ImageViewer {
    pub(super) source: Option<Arc<str>>,
    pub(super) close_hovered: bool,
    pub(super) close_pressed: bool,
}

impl ImageViewer {
    /// 【图片浮窗】【打开】记录缩略图对应的真实来源。
    /// 参数: source 为文件路径或数据 URL；返回: 无。
    pub(super) fn open(&mut self, source: Arc<str>) {
        self.source = Some(source);
        self.close_hovered = false;
        self.close_pressed = false;
    }

    /// 【图片浮窗】【关闭】释放图片引用与按钮状态。
    /// 参数: 无；返回: 无。
    pub(super) fn close(&mut self) {
        *self = Self::default();
    }
}

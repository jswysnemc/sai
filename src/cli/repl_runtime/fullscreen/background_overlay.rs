//! 后台任务面板叠加到全屏会话画面：占用正文行与左侧会话列，标题、输入框和右侧侧栏保持原样。

use super::{image_window, layout::FullscreenLayout};
use crate::cli::repl_background::{EmbeddedTaskPanel, PanelRect};
use crate::render::ansi_overlay::overlay_columns;

/// 【后台任务】【浮层区域】覆盖全部正文行与左侧会话列，左右各留一列显出底层内容。
/// 参数: layout 为当前分区；返回: 面板矩形。
pub(super) fn panel_rect(layout: &FullscreenLayout) -> PanelRect {
    PanelRect::inset(layout.main_width, layout.body_top, layout.body_height)
}

/// 【后台任务】【叠加绘制】把面板逐行写入已组装的画面，面板两侧保留底层正文与侧栏。
/// 参数: panel 为任务面板，layout 为当前分区，rows 为整屏行；返回: 无。
pub(super) fn overlay(
    panel: &mut EmbeddedTaskPanel,
    layout: &FullscreenLayout,
    rows: &mut [String],
) {
    let rect = panel_rect(layout);
    let lines = panel.render(rect);
    let left = usize::from(rect.x);
    let width = usize::from(rect.width);
    // 1. 【后台任务】【滚动条隐藏】会话滚动条落在面板外时一并清除，避免与面板日志滑块混淆
    let scrollbar =
        (!rect.contains(layout.scrollbar_col, rect.y)).then_some(usize::from(layout.scrollbar_col));
    for (index, line) in lines.iter().enumerate() {
        let Some(row) = rows.get_mut(usize::from(rect.y) + index) else {
            break;
        };
        // 2. 【后台任务】【图片遮挡】Kitty 图片按单元格绘制，只覆盖字符遮不住，先移除该行的放置序列
        let base = image_window::strip_placements(row);
        let mut composed = overlay_columns(&base, left, width, line);
        if let Some(column) = scrollbar {
            composed = overlay_columns(&composed, column, 1, " ");
        }
        *row = composed;
    }
}

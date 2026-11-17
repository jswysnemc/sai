//! 标题栏下方的紧凑消息列表，绘制和点击使用同一份窗口几何信息，并通过列局部覆盖保留右侧画面。

use super::layout::FullscreenLayout;
use super::state::FullscreenState;
use crate::cli::repl_text::visible_width;
use crate::i18n::text as t;
use crate::render::ansi_overlay::overlay_columns;
use crate::render::panel_palette::{bg, fg, PanelPalette};

pub(super) struct PickerLayout {
    pub(super) top: usize,
    pub(super) width: usize,
    pub(super) visible: usize,
    item_top: usize,
    chrome: bool,
}

impl PickerLayout {
    /// 【消息导航】【面板尺寸】限制宽度和条目数，极矮终端优先保留可选消息。
    /// 参数: layout 为全屏分区，count 为消息总数；返回: 不侵入输入框的面板布局。
    pub(super) fn compute(layout: &FullscreenLayout, count: usize) -> Self {
        let height = usize::from(layout.body_height);
        let chrome = height >= 3;
        let top = usize::from(layout.body_top);
        let visible = height
            .saturating_sub(if chrome { 2 } else { 0 })
            .min(8)
            .min(count);
        Self {
            top,
            width: usize::from(layout.content_width).min(88),
            visible,
            item_top: top + usize::from(chrome),
            chrome,
        }
    }

    /// 【消息导航】【消息命中】仅可见条目响应点击，标题和操作提示不作为消息。
    /// 参数: column/row 为屏幕坐标，start 为列表起点；返回: 命中消息序号。
    pub(super) fn item_at(&self, column: u16, row: u16, start: usize) -> Option<usize> {
        let row = usize::from(row);
        (usize::from(column) < self.width
            && row >= self.item_top
            && row < self.item_top + self.visible)
            .then(|| start + row - self.item_top)
    }

    /// 【消息导航】【面板边界】区分面板内部与外部点击。
    /// 参数: column/row 为屏幕坐标；返回: 是否位于面板内。
    pub(super) fn contains(&self, column: u16, row: u16) -> bool {
        let height = self.visible + if self.chrome { 2 } else { 0 };
        usize::from(column) < self.width
            && usize::from(row) >= self.top
            && usize::from(row) < self.top + height
    }
}

/// 【消息导航】【列表绘制】按列区间局部覆盖消息列表，保留面板右侧正文、概览轨道与主滚动条。
/// 参数: state 为全屏状态，layout 为屏幕分区，rows 为现有绘制行；返回: 无。
pub(super) fn overlay(state: &FullscreenState, layout: &FullscreenLayout, rows: &mut [String]) {
    if !state.message_picker.active {
        return;
    }
    let palette = PanelPalette::current();
    let anchors = &state.document.anchors;
    let picker = &state.message_picker;
    let geometry = PickerLayout::compute(layout, anchors.len());
    let mut panel = Vec::new();
    if geometry.chrome {
        panel.push(panel_row(
            &format!("{} ({})", t("User messages", "用户消息"), anchors.len()),
            geometry.width,
            false,
            false,
            &palette,
        ));
    }
    for (index, anchor) in anchors
        .iter()
        .enumerate()
        .skip(picker.start)
        .take(geometry.visible)
    {
        let marker = if index == picker.selected { "→" } else { " " };
        let summary = crate::render::ansi_clip::strip_ansi(&anchor.summary)
            .chars()
            .map(|ch| if ch.is_control() { ' ' } else { ch })
            .collect::<String>();
        panel.push(panel_row(
            &format!("{marker} {}  {summary}", index + 1),
            geometry.width,
            index == picker.selected,
            false,
            &palette,
        ));
    }
    if geometry.chrome {
        let range = format!(
            "{}-{} / {}",
            picker.start + 1,
            (picker.start + geometry.visible).min(anchors.len()),
            anchors.len()
        );
        let hint = if geometry.width >= 62 {
            t(
                "Up/Down select · Enter jump · Esc close",
                "上下选择 · Enter 跳转 · Esc 关闭",
            )
        } else {
            t("Enter jump · Esc close", "Enter 跳转 · Esc 关闭")
        };
        panel.push(panel_row(
            &format!("{range} · {hint}"),
            geometry.width,
            false,
            true,
            &palette,
        ));
    }
    // 1. 【消息导航】【列区间覆盖】只替换 0..geometry.width 列，保留宽屏右侧正文、概览轨道与主滚动条
    for (offset, line) in panel.into_iter().enumerate() {
        if let Some(row) = rows.get_mut(geometry.top + offset) {
            *row = overlay_columns(row, 0, geometry.width, &line);
        }
    }
}

/// 【消息导航】【定宽行】按 PanelPalette 明暗色板生成单行卡片，长消息按显示列数截断。
/// 参数: `text` 为文本，`width` 为宽度，`selected` 为选中状态，`dim` 为弱化状态，`palette` 为色板；返回: ANSI 行。
fn panel_row(
    text: &str,
    width: usize,
    selected: bool,
    dim: bool,
    palette: &PanelPalette,
) -> String {
    let text = crate::render::clip_to_width(
        &format!(" {text}"),
        width.saturating_sub(1),
        if width >= 4 { "..." } else { "" },
    );
    let padding = width.saturating_sub(visible_width(&text));
    let (row_bg, row_fg) = if selected {
        (bg(palette.focus_bg), format!("\x1b[1m{}", fg(palette.accent)))
    } else if dim {
        (bg(palette.hover_bg), fg(palette.muted))
    } else {
        (bg(palette.hover_bg), fg(palette.text))
    };
    format!("{row_bg}{row_fg}{text}{}\x1b[0m", " ".repeat(padding))
}

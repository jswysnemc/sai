//! 标题栏下方的紧凑消息列表，绘制和点击使用同一份窗口几何信息。

use super::layout::FullscreenLayout;
use super::state::FullscreenState;
use crate::cli::repl_text::visible_width;
use crate::i18n::text as t;

const BACKGROUND: &str = "\x1b[48;5;235m";
const SELECTED: &str = "\x1b[48;5;238m\x1b[38;2;94;196;168m";
const TEXT: &str = "\x1b[38;5;252m";
const DIM: &str = "\x1b[38;5;244m";

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

/// 【消息导航】【列表绘制】叠加当前窗口内的消息与操作提示，选中行使用统一深灰底色。
/// 参数: state 为全屏状态，layout 为屏幕分区，rows 为现有绘制行；返回: 无。
pub(super) fn overlay(state: &FullscreenState, layout: &FullscreenLayout, rows: &mut [String]) {
    if !state.message_picker.active {
        return;
    }
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
        ));
    }
    for (index, anchor) in anchors
        .iter()
        .enumerate()
        .skip(picker.start)
        .take(geometry.visible)
    {
        let marker = if index == picker.selected { ">" } else { " " };
        let summary = crate::render::ansi_clip::strip_ansi(&anchor.summary)
            .chars()
            .map(|ch| if ch.is_control() { ' ' } else { ch })
            .collect::<String>();
        panel.push(panel_row(
            &format!("{marker} {}  {summary}", index + 1),
            geometry.width,
            index == picker.selected,
            false,
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
        ));
    }
    // 1. 【消息导航】【局部覆盖】不改变会话文档和输入框，只覆盖列表占用的屏幕行
    for (offset, line) in panel.into_iter().enumerate() {
        if let Some(row) = rows.get_mut(geometry.top + offset) {
            *row = format!(
                "{line}{}",
                " ".repeat(usize::from(layout.main_width).saturating_sub(geometry.width))
            );
        }
    }
}

/// 【消息导航】【定宽行】生成带完整背景的单行，长消息按显示列数截断。
/// 参数: text 为文本，width 为宽度，selected 为选中状态，dim 为弱化状态；返回: ANSI 行。
fn panel_row(text: &str, width: usize, selected: bool, dim: bool) -> String {
    let text = crate::render::clip_to_width(
        &format!(" {text}"),
        width.saturating_sub(1),
        if width >= 4 { "..." } else { "" },
    );
    let padding = width.saturating_sub(visible_width(&text));
    let color = if selected {
        SELECTED
    } else if dim {
        DIM
    } else {
        TEXT
    };
    format!("{BACKGROUND}{color}{text}{}\x1b[0m", " ".repeat(padding))
}

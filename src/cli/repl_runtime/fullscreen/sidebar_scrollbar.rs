//! 右侧概览与审查区共用的滚动条几何和绘制，不占用正文列

use super::git_preview_view::PreviewLayout;
use super::layout::FullscreenLayout;
use super::sidebar::Sidebar;
use super::sidebar_action::SidebarSection;
use super::sidebar_palette::{fg, SidebarPalette};
use super::sidebar_regions::regions;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScrollTarget {
    Section(SidebarSection),
    ReviewFiles,
    ReviewDiff,
}

#[derive(Clone, Copy)]
pub(super) struct Scrollbar {
    pub(super) target: ScrollTarget,
    pub(super) top: usize,
    pub(super) visible: usize,
    pub(super) total: usize,
    pub(super) offset: usize,
}

#[derive(Clone, Copy)]
pub(super) struct ScrollbarDrag {
    pub(super) target: ScrollTarget,
    pub(super) grab: usize,
}

#[derive(Default)]
pub(super) struct ScrollbarInteraction {
    pub(super) hovered: Option<ScrollTarget>,
    pub(super) dragging: Option<ScrollbarDrag>,
}

impl ScrollbarInteraction {
    /// 【终端侧栏】【状态同步】分组折叠、内容收缩或面板切换时移除已消失轨道的交互
    /// 参数：bars 为本帧有效轨道；返回：无
    pub(super) fn sync(&mut self, bars: &[Scrollbar]) {
        self.hovered = self
            .hovered
            .filter(|target| bars.iter().any(|bar| bar.target == *target));
        self.dragging = self
            .dragging
            .filter(|drag| bars.iter().any(|bar| bar.target == drag.target));
    }
}

impl Scrollbar {
    /// 【终端侧栏】【滑块比例】按可见范围与内容长度计算轨道中的滑块
    /// 参数：当前滚动条几何；返回：相对轨道顶部的滑块范围
    pub(super) fn thumb(self) -> Range<usize> {
        let size = (self.visible.saturating_mul(self.visible) / self.total.max(1))
            .clamp(1, self.visible.max(1));
        let travel = self.visible.saturating_sub(size);
        let maximum = self.total.saturating_sub(self.visible);
        let top = self.offset.min(maximum).saturating_mul(travel) / maximum.max(1);
        top..top + size
    }

    /// 【终端侧栏】【拖动映射】保留按下位置与滑块顶部的距离，允许指针拖出轨道
    /// 参数：row 为屏幕行，grab 为滑块内按下位置；返回：有界内容偏移
    pub(super) fn offset_at(self, row: usize, grab: usize) -> usize {
        let travel = self.visible.saturating_sub(self.thumb().len());
        if travel == 0 {
            return self.offset.min(self.total.saturating_sub(self.visible));
        }
        let position = row.saturating_sub(self.top + grab).min(travel);
        position.saturating_mul(self.total.saturating_sub(self.visible)) / travel.max(1)
    }
}

/// 【终端侧栏】【滚动几何】只为实际溢出的分组或审查内容生成轨道，固定标题与页脚不参与
/// 参数：sidebar 为文档与偏移，layout 为屏幕分区；返回：各自独立的滚动条
pub(super) fn scrollbars(sidebar: &Sidebar, layout: &FullscreenLayout) -> Vec<Scrollbar> {
    if layout.sidebar.is_none() {
        return Vec::new();
    }
    let bars = if layout.review {
        let geometry = PreviewLayout::compute(layout, sidebar.git.summary.files.len());
        vec![
            Scrollbar {
                target: ScrollTarget::ReviewFiles,
                top: geometry.files_top,
                visible: geometry.files_visible,
                total: sidebar.git.summary.files.len(),
                offset: sidebar.preview.file_scroll,
            },
            Scrollbar {
                target: ScrollTarget::ReviewDiff,
                top: geometry.diff_top,
                visible: geometry.visible,
                total: sidebar
                    .preview
                    .content
                    .as_ref()
                    .map_or(0, |content| content.lines.len()),
                offset: sidebar.preview.scroll,
            },
        ]
    } else {
        regions(&sidebar.document, usize::from(layout.rows))
            .into_iter()
            .map(|region| Scrollbar {
                target: ScrollTarget::Section(region.section),
                top: region.top + 1,
                visible: region.visible,
                total: region.content.len(),
                offset: sidebar.scrolls.get(region.section),
            })
            .collect()
    };
    bars.into_iter()
        .filter(|bar| bar.visible > 0 && bar.total > bar.visible)
        .collect()
}

/// 【终端侧栏】【滚动条绘制】使用右侧预留空白列显示细轨道和滑块，悬停与拖动时提亮
/// 参数：bars 为轨道，interaction 为交互，rows 为绘制行，column 为预留列，row_offset 为首行位置
/// 返回：无，原地修改指定列，保留正文与末行位置
pub(super) fn decorate(
    bars: &[Scrollbar],
    interaction: &ScrollbarInteraction,
    rows: &mut [String],
    column: usize,
    row_offset: usize,
) {
    let palette = SidebarPalette::current();
    for (index, line) in rows.iter_mut().enumerate() {
        let screen_row = row_offset + index;
        let Some(bar) = bars
            .iter()
            .find(|bar| (bar.top..bar.top + bar.visible).contains(&screen_row))
        else {
            continue;
        };
        let thumb = bar.thumb().contains(&(screen_row - bar.top));
        let active = interaction.hovered == Some(bar.target)
            || interaction
                .dragging
                .is_some_and(|drag| drag.target == bar.target);
        let color = if thumb && active {
            palette.accent
        } else if thumb {
            palette.muted
        } else {
            palette.border
        };
        let prefix = crate::render::ansi_clip::clip_ansi_to_width(line, column, "");
        let padding = column.saturating_sub(crate::cli::repl_text::visible_width(&prefix));
        *line = format!(
            "{prefix}{}\x1b[0m{}{}\x1b[0m",
            " ".repeat(padding),
            fg(color),
            if thumb { "┃" } else { "│" }
        );
    }
}

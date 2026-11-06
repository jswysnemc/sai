//! 右侧滚动条优先处理自身交互，正文选择和分界线拖动仍按原有入口处理

use super::input::Effect;
use super::sidebar_scrollbar::{scrollbars, ScrollTarget, Scrollbar, ScrollbarDrag};
use crate::cli::repl_runtime::ReplRuntime;
use crossterm::event::{Event, MouseButton, MouseEventKind};

impl ReplRuntime {
    /// 【终端侧栏】【滚动条输入】处理预留列上的悬停、轨道翻页与滑块拖动，关闭或失焦时清理
    /// 参数：event 为终端事件；返回：绘制效果，其他控件继续处理未命中事件
    pub(super) fn sidebar_scrollbar_event(&mut self, event: &Event) -> Option<Effect> {
        let session = self.fullscreen.as_mut()?;
        let layout = session.state.layout?;
        let interaction = &mut self.sidebar_control.scrollbar;
        let Event::Mouse(mouse) = event else {
            if matches!(event, Event::Resize(..) | Event::FocusLost) {
                let changed =
                    interaction.hovered.take().is_some() | interaction.dragging.take().is_some();
                let changed = changed
                    | std::mem::take(&mut self.sidebar_control.dragging)
                    | std::mem::take(&mut self.sidebar_control.divider_hovered);
                return changed.then_some(Effect::Repaint);
            }
            if matches!(event, Event::Key(_)) {
                interaction.dragging = None;
                interaction.hovered = None;
            }
            return None;
        };
        let bars = scrollbars(&session.sidebar, &layout);
        if let Some(drag) = interaction.dragging {
            let bar = bars.iter().find(|bar| bar.target == drag.target).copied();
            if let Some(bar) = bar {
                if matches!(
                    mouse.kind,
                    MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)
                ) {
                    set_offset(
                        &mut session.sidebar,
                        bar,
                        bar.offset_at(usize::from(mouse.row), drag.grab),
                    );
                    if mouse.kind == MouseEventKind::Up(MouseButton::Left) {
                        interaction.dragging = None;
                        interaction.hovered = (mouse.column == layout.cols - 1
                            && (bar.top..bar.top + bar.visible).contains(&usize::from(mouse.row)))
                        .then_some(bar.target);
                    }
                    return Some(Effect::Repaint);
                }
            } else {
                interaction.dragging = None;
                interaction.hovered = None;
            }
        }
        // 1. 【终端侧栏】【交互归属】已开始的聊天或差异拖选不被右侧滚动条接管
        if session.state.press.is_some() || session.sidebar.preview.selection.is_pressed() {
            interaction.hovered = None;
            return None;
        }
        let target = (mouse.column == layout.cols.saturating_sub(1))
            .then(|| {
                bars.iter()
                    .find(|bar| (bar.top..bar.top + bar.visible).contains(&usize::from(mouse.row)))
                    .copied()
            })
            .flatten();
        if mouse.kind == MouseEventKind::Moved {
            let hovered = target.map(|bar| bar.target);
            let changed = interaction.hovered != hovered;
            interaction.hovered = hovered;
            if target.is_some() {
                let cleared = session.sidebar.hovered.take().is_some()
                    | session.sidebar.preview.hovered.take().is_some()
                    | session.state.pointer_row.take().is_some()
                    | session.state.pointer_col.take().is_some()
                    | session.state.hover.take().is_some()
                    | std::mem::take(&mut session.state.header_hover);
                return Some(if changed || cleared {
                    Effect::Repaint
                } else {
                    Effect::Nothing
                });
            }
            return None;
        }
        let Some(bar) = target else {
            return (layout.sidebar.is_some()
                && mouse.column == layout.cols.saturating_sub(1)
                && mouse.kind == MouseEventKind::Down(MouseButton::Left))
            .then_some(Effect::Nothing);
        };
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            // 2. 【终端侧栏】【轨道定位】滑块内按下开始拖动，轨道空白按一屏翻页
            let relative = usize::from(mouse.row) - bar.top;
            let thumb = bar.thumb();
            if thumb.contains(&relative) {
                interaction.dragging = Some(ScrollbarDrag {
                    target: bar.target,
                    grab: relative - thumb.start,
                });
            } else {
                let next = if relative < thumb.start {
                    bar.offset.saturating_sub(bar.visible)
                } else {
                    bar.offset.saturating_add(bar.visible)
                };
                set_offset(&mut session.sidebar, bar, next);
            }
            interaction.hovered = Some(bar.target);
            session.sidebar.hovered = None;
            session.sidebar.preview.hovered = None;
            session.state.pointer_row = None;
            session.state.pointer_col = None;
            session.state.hover = None;
            session.state.header_hover = false;
            return Some(Effect::Repaint);
        }
        None
    }
}

/// 【终端侧栏】【局部偏移】只修改滚动条对应内容，不改变其他分组、聊天位置或输入草稿
/// 参数：sidebar 为侧栏状态，bar 为轨道几何，offset 为目标偏移；返回：无
fn set_offset(sidebar: &mut super::sidebar::Sidebar, bar: Scrollbar, offset: usize) {
    let offset = offset.min(bar.total.saturating_sub(bar.visible));
    match bar.target {
        ScrollTarget::Section(section) => sidebar.scrolls.set(section, offset),
        ScrollTarget::ReviewFiles => sidebar.preview.file_scroll = offset,
        ScrollTarget::ReviewDiff => sidebar.preview.scroll = offset,
    }
}

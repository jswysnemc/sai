//! 全屏会话视图的交互状态：滚动位置、跟随底部、段落展开与用户消息跳转。

use super::layout::FullscreenLayout;
use crate::render::transcript::{FullscreenDocument, ParagraphKey, ParagraphSpan};
use std::collections::HashSet;

/// 鼠标滚轮每格滚动的行数。
pub(super) const WHEEL_STEP: usize = 3;
/// 跳转到用户消息时，消息上方保留的上下文行数。
const JUMP_CONTEXT_ROWS: usize = 1;

pub(super) enum ActivityNavigation {
    Page(bool),
    Top,
    Latest,
}

/// 全屏视图状态；只描述“看哪里”，不持有终端资源。
#[derive(Default)]
pub(super) struct FullscreenState {
    pub(super) activity: super::activity_viewport::ActivityViewports,
    /// 正文首行在文档中的行号
    pub(super) scroll: usize,
    /// 是否跟随最新输出（停在底部）
    pub(super) follow: bool,
    /// 离开底部后出现了新输出
    pub(super) unseen: bool,
    /// 用户展开的段落键
    pub(super) expanded: HashSet<ParagraphKey>,
    /// 最近一次渲染的正文
    pub(super) document: FullscreenDocument,
    /// 最近一次绘制的屏幕分区
    pub(super) layout: Option<FullscreenLayout>,
    /// 上一帧逐行内容，用于只重绘变化行
    pub(super) previous: Option<Vec<String>>,
    /// 鼠标悬停的概览标记对应的用户消息
    pub(super) hover: Option<usize>,
    /// 鼠标在正文区的位置（相对正文顶部的行）；滚动后据此重新判断悬停段落
    pub(super) pointer_row: Option<usize>,
    /// 鼠标在正文区的列；用于「回到底部」按钮悬停
    pub(super) pointer_col: Option<u16>,
    /// 是否正在拖动滚动条
    pub(super) dragging: bool,
    /// 正文按下位置；松开时没有拖动则按点击处理
    pub(super) press: Option<super::selection::TextPoint>,
    /// 当前拖动选区
    pub(super) selection: Option<super::selection::Selection>,
    /// 最近一次复制的字符数，显示在标题上直到下一次操作
    pub(super) copied: Option<usize>,
}

impl FullscreenState {
    /// 创建停在底部、跟随输出的初始状态。
    ///
    /// 返回:
    /// - 初始状态
    pub(super) fn new() -> Self {
        Self {
            follow: true,
            ..Self::default()
        }
    }

    /// 当前正文区可滚动的最大首行。
    ///
    /// 参数:
    /// - `body_height`: 正文区行数
    ///
    /// 返回:
    /// - 最大滚动位置
    pub(super) fn max_scroll(&self, body_height: usize) -> usize {
        self.document.lines.len().saturating_sub(body_height)
    }

    /// 换入新渲染的正文，并按跟随状态修正滚动位置。
    ///
    /// 参数:
    /// - `document`: 新正文
    /// - `body_height`: 正文区行数
    ///
    /// 返回:
    /// - 无
    pub(super) fn apply_document(&mut self, document: FullscreenDocument, body_height: usize) {
        let grew = document.lines.len() > self.document.lines.len();
        // 1. 清掉已不存在的展开键（/clear 或会话切换后）
        let mut live_keys = document
            .paragraphs
            .iter()
            .map(|span| span.key)
            .collect::<HashSet<_>>();
        live_keys.extend(document.retained_keys.iter().copied());
        self.expanded.retain(|key| live_keys.contains(key));
        self.document = document;
        // 2. 跟随时贴底；否则保持位置，只在内容增长时提示有新输出
        let max = self.max_scroll(body_height);
        if self.follow {
            self.scroll = max;
            self.unseen = false;
        } else {
            if grew {
                self.unseen = true;
            }
            self.scroll = self.scroll.min(max);
            if self.scroll == max && max == 0 {
                self.follow = true;
                self.unseen = false;
            }
        }
    }

    /// 相对滚动，滚到底部时恢复跟随。
    ///
    /// 参数:
    /// - `delta`: 正数向下，负数向上
    /// - `body_height`: 正文区行数
    ///
    /// 返回:
    /// - 位置是否变化
    pub(super) fn scroll_by(&mut self, delta: isize, body_height: usize) -> bool {
        let target = if delta < 0 {
            self.scroll.saturating_sub(delta.unsigned_abs())
        } else {
            self.scroll.saturating_add(delta as usize)
        };
        self.scroll_to(target, body_height)
    }

    /// 【终端】【链内输入】命中子视口时优先滚动链内列表；达到边界无法滚动时返回空值，允许外层主会话平滑接管滚动。
    ///
    /// 参数:
    /// - `row`: 鼠标命中的文档物理行
    /// - `delta`: 滚轮位移
    ///
    /// 返回:
    /// - 若链内产生位移返回 Some(true)；未命中或已达边界返回 None 以触发冒泡回退
    pub(super) fn scroll_activity_at(&mut self, row: usize, delta: isize) -> Option<bool> {
        let key = self
            .document
            .activities
            .iter()
            .find(|region| region.scrollable && row >= region.start && row < region.end)?
            .key;
        let changed = self.scroll_activity(key, delta);
        if changed {
            Some(true)
        } else {
            // 链内已触顶或触底，返回 None 允许外部主会话继续滚动
            None
        }
    }

    /// 【终端】【链内键盘】优先浏览已操作的可见链，否则使用最近的可见链。
    /// 参数: `navigation` 为翻页或首尾操作，`body_height` 为正文高度；返回: 是否命中并改变位置。
    pub(super) fn navigate_activity(
        &mut self,
        navigation: ActivityNavigation,
        body_height: usize,
    ) -> Option<bool> {
        let visible = |region: &&crate::render::transcript::ActivityRegion| {
            region.scrollable
                && region.end > self.scroll
                && region.start < self.scroll + body_height
        };
        let key = self
            .document
            .activities
            .iter()
            .filter(visible)
            .find(|region| Some(region.key) == self.activity.selected)
            .or_else(|| self.document.activities.iter().filter(visible).next_back())?
            .key;
        let delta = match navigation {
            ActivityNavigation::Top => isize::MIN,
            ActivityNavigation::Latest => isize::MAX,
            ActivityNavigation::Page(forward) => {
                let page = self.activity.page(key) as isize;
                if forward {
                    page
                } else {
                    -page
                }
            }
        };
        Some(self.scroll_activity(key, delta))
    }

    /// 【终端】【链内跟随】提示行单击恢复跟随，不折叠节点，也不移动外层会话。
    /// 参数: `row` 为文档行；返回: 是否命中并改变位置。
    pub(super) fn resume_activity_at(&mut self, row: usize) -> Option<bool> {
        let key = self
            .document
            .activities
            .iter()
            .find(|region| region.footer == Some(row))?
            .key;
        Some(self.scroll_activity(key, isize::MAX))
    }

    /// 【终端】【链内状态】应用独立滚动并清理已失效选区。
    /// 参数: `key` 为链键，`delta` 为偏移；返回: 位置是否改变。
    fn scroll_activity(&mut self, key: ParagraphKey, delta: isize) -> bool {
        let changed = self.activity.scroll(key, delta);
        if changed {
            self.document = self.activity.project();
            self.press = None;
            self.selection = None;
            self.copied = None;
        }
        changed
    }

    /// 滚动到指定首行，滚到底部时恢复跟随。
    ///
    /// 参数:
    /// - `target`: 目标首行
    /// - `body_height`: 正文区行数
    ///
    /// 返回:
    /// - 位置是否变化
    pub(super) fn scroll_to(&mut self, target: usize, body_height: usize) -> bool {
        let max = self.max_scroll(body_height);
        let next = target.min(max);
        let changed = next != self.scroll;
        self.scroll = next;
        self.follow = next == max;
        if self.follow {
            self.unseen = false;
        }
        changed
    }

    /// 鼠标当前悬停的可展开段落；拖选进行中不显示悬停。
    ///
    /// 返回:
    /// - 被悬停的段落
    pub(super) fn hovered_paragraph(&self) -> Option<&ParagraphSpan> {
        if self.press.is_some() || self.selection.is_some() || self.dragging {
            return None;
        }
        let row = self.pointer_row?;
        self.document.paragraph_control_at(self.scroll + row)
    }

    /// 当前浮动标题对应的用户消息：正文首行之前最近的一条。
    ///
    /// 返回:
    /// - 用户消息下标
    pub(super) fn current_anchor(&self) -> Option<usize> {
        self.document
            .anchor_index_at(self.scroll + JUMP_CONTEXT_ROWS)
    }

    /// 把指定用户消息滚到正文顶部。
    ///
    /// 参数:
    /// - `index`: 用户消息下标
    /// - `body_height`: 正文区行数
    ///
    /// 返回:
    /// - 位置是否变化
    pub(super) fn jump_to_anchor(&mut self, index: usize, body_height: usize) -> bool {
        let Some(anchor) = self.document.anchors.get(index) else {
            return false;
        };
        let target = anchor.row.saturating_sub(JUMP_CONTEXT_ROWS);
        self.scroll_to(target, body_height)
    }

    /// 跳到上一条或下一条用户消息。
    ///
    /// 参数:
    /// - `forward`: true 为下一条
    /// - `body_height`: 正文区行数
    ///
    /// 返回:
    /// - 位置是否变化
    pub(super) fn jump_relative(&mut self, forward: bool, body_height: usize) -> bool {
        let anchors = &self.document.anchors;
        if anchors.is_empty() {
            return false;
        }
        let top = self.scroll + JUMP_CONTEXT_ROWS;
        let index = if forward {
            anchors.iter().position(|anchor| anchor.row > top)
        } else {
            anchors.iter().rposition(|anchor| anchor.row < top)
        };
        match index {
            Some(index) => self.jump_to_anchor(index, body_height),
            // 已在最后一条之后：再向下直接到底
            None if forward => self.scroll_to(usize::MAX, body_height),
            None => self.scroll_to(0, body_height),
        }
    }

    /// 切换段落展开状态，执行链使用分组标题锚定，其他内容使用段落标题。
    ///
    /// 折叠时只响应提示行；展开后整段均可收起，拖选由输入层优先处理。
    ///
    /// 参数:
    /// - `row`: 被点击的文档行
    ///
    /// 返回:
    /// - 被切换的段落键与点击时标题行在屏幕上的偏移；未命中为 None
    pub(super) fn toggle_at(&mut self, row: usize) -> Option<(ParagraphKey, isize)> {
        let span = self
            .document
            .paragraph_at(row)
            .filter(|span| span.expanded || span.control == row)?
            .clone();
        if !self.expanded.remove(&span.key) {
            self.expanded.insert(span.key);
        }
        self.activity.focus(span.key);
        // 切换后停止跟随，否则展开的内容会被新输出推走
        self.follow = false;
        Some((
            span.key,
            self.toggle_anchor_row(span.start) as isize - self.scroll as isize,
        ))
    }

    /// 【终端】【折叠锚定】节点展开收起只影响链内窗口，不推动外层会话。
    /// 参数: `row` 为段落标题行；返回: 应保持屏幕位置的分组或段落标题行。
    fn toggle_anchor_row(&self, row: usize) -> usize {
        self.document
            .activities
            .iter()
            .find(|region| row >= region.start && row < region.end)
            .map(|region| region.start)
            .unwrap_or(row)
    }

    /// 重排后让被切换段落的标题行停在原来的屏幕位置。
    ///
    /// 在长段正文中间点击收起时，标题行可能已滚出正文顶部；此时把收起后的段落
    /// 放到正文顶部，而不是让它停在视野之外。
    ///
    /// 参数:
    /// - `key`: 被切换的段落键
    /// - `screen_offset`: 切换前标题行相对正文顶部的偏移
    /// - `body_height`: 正文区行数
    ///
    /// 返回:
    /// - 无
    pub(super) fn restore_toggle_anchor(
        &mut self,
        key: ParagraphKey,
        screen_offset: isize,
        body_height: usize,
    ) {
        let Some(span) = self.document.paragraphs.iter().find(|span| span.key == key) else {
            return;
        };
        let target =
            (self.toggle_anchor_row(span.start) as isize - screen_offset.max(0)).max(0) as usize;
        let max = self.max_scroll(body_height);
        self.scroll = target.min(max);
        self.follow = self.scroll == max && !span.expanded;
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;

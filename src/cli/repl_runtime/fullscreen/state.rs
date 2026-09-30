//! 全屏会话视图的交互状态：滚动位置、跟随底部、段落展开与用户消息跳转。

use super::layout::FullscreenLayout;
use crate::render::transcript::FullscreenDocument;
use std::collections::HashSet;

/// 鼠标滚轮每格滚动的行数。
pub(super) const WHEEL_STEP: usize = 3;
/// 跳转到用户消息时，消息上方保留的上下文行数。
const JUMP_CONTEXT_ROWS: usize = 1;

/// 全屏视图状态；只描述“看哪里”，不持有终端资源。
#[derive(Default)]
pub(super) struct FullscreenState {
    /// 正文首行在文档中的行号
    pub(super) scroll: usize,
    /// 是否跟随最新输出（停在底部）
    pub(super) follow: bool,
    /// 离开底部后出现了新输出
    pub(super) unseen: bool,
    /// 用户展开的段落键
    pub(super) expanded: HashSet<usize>,
    /// 最近一次渲染的正文
    pub(super) document: FullscreenDocument,
    /// 最近一次绘制的屏幕分区
    pub(super) layout: Option<FullscreenLayout>,
    /// 上一帧逐行内容，用于只重绘变化行
    pub(super) previous: Option<Vec<String>>,
    /// 鼠标悬停的概览标记对应的用户消息
    pub(super) hover: Option<usize>,
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
        let live_keys = document
            .paragraphs
            .iter()
            .map(|span| span.key)
            .collect::<HashSet<_>>();
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

    /// 切换段落展开状态，并记录段落标题相对正文顶部的位置用于重排后锚定。
    ///
    /// 展开与收起都只需点击段落范围内的任意一行。
    ///
    /// 参数:
    /// - `row`: 被点击的文档行
    ///
    /// 返回:
    /// - 被切换的段落键与点击时标题行在屏幕上的偏移；未命中为 None
    pub(super) fn toggle_at(&mut self, row: usize) -> Option<(usize, isize)> {
        let span = self.document.paragraph_at(row)?.clone();
        if !self.expanded.remove(&span.key) {
            self.expanded.insert(span.key);
        }
        // 切换后停止跟随，否则展开的内容会被新输出推走
        self.follow = false;
        Some((span.key, span.start as isize - self.scroll as isize))
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
        key: usize,
        screen_offset: isize,
        body_height: usize,
    ) {
        let Some(span) = self.document.paragraphs.iter().find(|span| span.key == key) else {
            return;
        };
        let target = (span.start as isize - screen_offset.max(0)).max(0) as usize;
        let max = self.max_scroll(body_height);
        self.scroll = target.min(max);
        self.follow = self.scroll == max && !span.expanded;
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;

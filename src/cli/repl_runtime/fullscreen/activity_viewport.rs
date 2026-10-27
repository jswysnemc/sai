use super::activity_scroll_anchor::ActivityScrollAnchor;
use super::activity_viewport_hints;
use super::activity_window::{ActivityWindow, DocumentProjection};
use crate::render::transcript::{ActivityRegion, AnsiLine, FullscreenDocument, ParagraphKey};
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
struct ScrollPosition {
    offset: usize,
    follow: bool,
    height_limit: Option<usize>,
}

/// 【终端】【点击锚点】保存节点在链内的位置，以及标题下方可以展开的空间。
struct ActivityFocus {
    key: ParagraphKey,
    body_offset: usize,
    height_limit: Option<usize>,
}

impl Default for ScrollPosition {
    /// 【终端】【链内滚动】初始化独立跟随状态。
    /// 参数: 无；返回: 跟随最新步骤的滚动位置。
    fn default() -> Self {
        Self {
            offset: 0,
            follow: true,
            height_limit: None,
        }
    }
}

/// 保留完整源文档与独立位置，交互仅重新裁切，不重新渲染历史。
#[derive(Default)]
pub(super) struct ActivityViewports {
    source: FullscreenDocument,
    positions: HashMap<ParagraphKey, ScrollPosition>,
    body_rows: usize,
    width: usize,
    terminal_height: usize,
    focus: Option<ActivityFocus>,
    pub(super) selected: Option<ParagraphKey>,
}

impl ActivityViewports {
    /// 【终端】【跟随状态】参数为执行链键；返回是否已跟随末尾，用于区分状态和按钮。
    pub(super) fn is_following(&self, key: ParagraphKey) -> bool {
        self.positions
            .get(&key)
            .is_none_or(|position| position.follow)
    }
    /// 【终端】【链内视口】换入源文档，运行与完成共用自然高度，长详情受八成高度预算限制。
    /// 参数: `source` 为完整文档，`height` 为正文高度，`width` 为净宽，`terminal_height` 为终端总行数；返回: 可见文档。
    pub(super) fn update(
        &mut self,
        source: FullscreenDocument,
        height: usize,
        width: usize,
        terminal_height: usize,
    ) -> FullscreenDocument {
        let mut keys = source.retained_keys.iter().copied().collect::<HashSet<_>>();
        keys.extend(source.paragraphs.iter().map(|span| span.key));
        keys.extend(source.activities.iter().map(|region| region.key));
        self.positions.retain(|key, _| keys.contains(key));
        self.selected = self.selected.filter(|key| keys.contains(key));
        if source.activities.is_empty() {
            self.source = FullscreenDocument::default();
            self.focus = None;
            return source;
        }
        // 1. 【终端】【尺寸变化】只有真实终端缩放才释放点击时的高度限制，状态栏变化不重置阅读位置
        if self.width != width || self.terminal_height != terminal_height {
            for position in self.positions.values_mut() {
                position.height_limit = None;
            }
        }
        if self.width != width {
            for region in &source.activities {
                let Some(position) = self
                    .positions
                    .get_mut(&region.key)
                    .filter(|position| !position.follow)
                else {
                    continue;
                };
                let Some(previous) = self
                    .source
                    .activities
                    .iter()
                    .find(|previous| previous.key == region.key)
                else {
                    continue;
                };
                if let Some(offset) =
                    ActivityScrollAnchor::capture(&self.source, previous, position.offset)
                        .and_then(|anchor| anchor.locate(&source, region))
                {
                    position.offset = offset;
                }
            }
        }
        self.body_rows = if height < 4 {
            0
        } else {
            // 【终端】【高度预算】以终端总高度计算八成，给分组标题和页脚留出空间
            (terminal_height.saturating_mul(8) / 10).min(height.saturating_sub(2))
        };
        self.width = width;
        self.terminal_height = terminal_height;
        self.source = source;
        if let Some(focus) = self.focus.take() {
            if let Some(span) = self
                .source
                .paragraphs
                .iter()
                .find(|span| span.key == focus.key)
            {
                if let Some(region) = self
                    .source
                    .activities
                    .iter()
                    .find(|region| span.start >= region.start && span.start < region.end)
                {
                    self.selected = Some(region.key);
                    let height = self
                        .body_rows
                        .min(focus.height_limit.unwrap_or(self.body_rows))
                        .min(region.end.saturating_sub(region.start + 1));
                    let pinned = usize::from(
                        ActivityWindow::at(&self.source, region, 0, height)
                            .pinned_control
                            .is_some(),
                    );
                    let position = self.positions.entry(region.key).or_default();
                    position.height_limit = focus.height_limit;
                    if region.key != focus.key {
                        position.offset = span
                            .control
                            .saturating_sub(region.start + 1)
                            .saturating_sub(focus.body_offset.saturating_sub(pinned));
                        position.follow = focus.key.is_active()
                            && region.active
                            && span.expanded
                            && span.end == region.end;
                    }
                }
            }
        }
        self.project()
    }

    /// 【终端】【链内定位】展开或折叠后保留操作节点的位置。
    /// 参数: `key` 为节点键；返回: 无。
    #[cfg(test)]
    pub(super) fn focus(&mut self, key: ParagraphKey) {
        self.focus_at(key, 0, None);
    }

    /// 【终端】【点击定位】记录原节点行位置，只在标题下方剩余空间中展开详情。
    /// 参数: `key` 为节点键，`body_offset` 为节点相对链正文的行数，`height_limit` 为可用详情高度；返回: 无。
    pub(super) fn focus_at(
        &mut self,
        key: ParagraphKey,
        body_offset: usize,
        height_limit: Option<usize>,
    ) {
        self.focus = Some(ActivityFocus {
            key,
            body_offset,
            height_limit,
        });
    }

    /// 【终端】【链内翻页】计算留有上下文重叠的页高。
    /// 参数: `key` 为链键；返回: 一页的滚动行数。
    pub(super) fn page(&self, key: ParagraphKey) -> usize {
        self.source
            .activities
            .iter()
            .find(|region| region.key == key)
            .map(|region| {
                let height = self.height(region);
                let pinned = usize::from(
                    ActivityWindow::at(&self.source, region, 0, height)
                        .pinned_control
                        .is_some(),
                );
                height.saturating_sub(2 + pinned).max(1)
            })
            .unwrap_or(1)
    }

    /// 【终端】【链内滚动】只改变命中链的位置，链内边界不影响整页滚动。
    /// 参数: `key` 为链键，`delta` 为行偏移；返回: 位置是否改变。
    pub(super) fn scroll(&mut self, key: ParagraphKey, delta: isize) -> bool {
        let Some(region) = self
            .source
            .activities
            .iter()
            .find(|region| region.key == key)
        else {
            return false;
        };
        let height = self.height(region);
        let position = self.positions.entry(key).or_default();
        let window = ActivityWindow::at(
            &self.source,
            region,
            position.offset.saturating_add_signed(delta),
            height,
        );
        let next = window.offset;
        let changed = next != position.offset;
        position.offset = next;
        position.follow = next == window.max;
        self.selected = Some(key);
        changed
    }

    /// 【终端】【链内高度】短内容不填充空白，展开时受原标题下方可用空间限制。
    /// 参数: `region` 为链范围；返回: 正文行数。
    fn height(&self, region: &ActivityRegion) -> usize {
        self.body_rows
            .min(
                self.positions
                    .get(&region.key)
                    .and_then(|position| position.height_limit)
                    .unwrap_or(self.body_rows),
            )
            .min(region.end.saturating_sub(region.start + 1))
    }

    /// 【终端】【链内裁切】保留分组标题和被滚过的节点标题，映射点击与消息锚点。
    /// 参数: 无；返回: 当前各执行链位置对应的文档。
    pub(super) fn project(&mut self) -> FullscreenDocument {
        if self.body_rows == 0 {
            let mut document = self.source.clone();
            for region in &mut document.activities {
                region.scrollable = false;
            }
            return document;
        }
        let mut projection = DocumentProjection::new(&self.source);
        let mut cursor = 0;
        // 1. 【终端】【链内裁切】审批始终完整显示，普通链只在内容溢出时启用独立滚动
        for region in &self.source.activities {
            projection.copy(cursor..region.start);
            let mut visible = region.clone();
            visible.start = projection.len();
            if !region.scrollable {
                projection.copy(region.start..region.end);
            } else {
                projection.copy(region.start..region.start + 1);
                let height = self.height(region);
                let position = self.positions.entry(region.key).or_default();
                let window = ActivityWindow::at(
                    &self.source,
                    region,
                    if position.follow {
                        usize::MAX
                    } else {
                        position.offset
                    },
                    height,
                );
                position.offset = window.offset;
                position.follow = window.offset == window.max;
                if let Some(control) = window.pinned_control {
                    projection.copy(control..control + 1);
                }
                if let Some(context) = window.context {
                    projection.copy(context..context + 1);
                }
                projection.copy(window.rows.clone());
                let pinned = usize::from(window.pinned_control.is_some());
                let rendered = window.rows.len() + usize::from(window.context.is_some()) + pinned;
                for _ in rendered..height {
                    projection.push(AnsiLine::new(String::new()));
                }
                visible.scrollable = window.max > window.min;
                if visible.scrollable {
                    let focused = self.selected == Some(region.key);
                    visible.footer = Some(projection.len());
                    projection.push(activity_viewport_hints::footer_with_focus(
                        window.offset + 1 - pinned,
                        window.offset + window.rows.len() - pinned,
                        region.end - region.start - 1 - pinned,
                        position.follow,
                        self.width,
                        focused,
                    ));
                }
            }
            visible.end = projection.len();
            projection.region(visible);
            cursor = region.end;
        }
        projection.copy(cursor..self.source.lines.len());
        projection.finish()
    }
}

#[cfg(test)]
#[path = "activity_viewport_tests.rs"]
mod tests;

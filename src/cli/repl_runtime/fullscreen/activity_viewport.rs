use super::activity_scroll_anchor::ActivityScrollAnchor;
use super::activity_viewport_hints;
use super::activity_window::{ActivityWindow, DocumentProjection};
use crate::render::transcript::{ActivityRegion, AnsiLine, FullscreenDocument, ParagraphKey};
use std::collections::{HashMap, HashSet};

const MAX_BODY_ROWS: usize = 10;

#[derive(Clone)]
struct ScrollPosition {
    offset: usize,
    follow: bool,
}

impl Default for ScrollPosition {
    /// 【终端】【链内滚动】初始化独立跟随状态。
    /// 参数: 无；返回: 跟随最新步骤的滚动位置。
    fn default() -> Self {
        Self {
            offset: 0,
            follow: true,
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
    focus: Option<ParagraphKey>,
    pub(super) selected: Option<ParagraphKey>,
}

impl ActivityViewports {
    /// 【终端】【链内视口】换入源文档，运行区固定高度，已完成的短链不保留大片留白。
    /// 参数: `source` 为完整文档，`height` 为正文高度，`width` 为净宽；返回: 可见文档。
    pub(super) fn update(
        &mut self,
        source: FullscreenDocument,
        height: usize,
        width: usize,
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
            ((height - 2) / 2).min(MAX_BODY_ROWS)
        };
        self.width = width;
        self.source = source;
        if let Some(key) = self.focus.take() {
            if let Some(span) = self.source.paragraphs.iter().find(|span| span.key == key) {
                if let Some(region) = self.source.activities.iter().find(|region| {
                    region.key != key && span.start > region.start && span.start < region.end
                }) {
                    self.selected = Some(region.key);
                    self.positions.insert(
                        region.key,
                        ScrollPosition {
                            offset: span.start - region.start - 1,
                            follow: key.is_active()
                                && region.active
                                && span.expanded
                                && span.end == region.end,
                        },
                    );
                }
            }
        }
        self.project()
    }

    /// 【终端】【链内定位】展开或折叠后保留操作节点的位置。
    /// 参数: `key` 为节点键；返回: 无。
    pub(super) fn focus(&mut self, key: ParagraphKey) {
        self.focus = Some(key);
    }

    /// 【终端】【链内翻页】计算留有上下文重叠的页高。
    /// 参数: `key` 为链键；返回: 一页的滚动行数。
    pub(super) fn page(&self, key: ParagraphKey) -> usize {
        self.source
            .activities
            .iter()
            .find(|region| region.key == key)
            .map(|region| self.height(region).saturating_sub(2).max(1))
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
        let max = ActivityWindow::at(&self.source, region, usize::MAX, self.height(region)).max;
        let position = self.positions.entry(key).or_default();
        let next = position.offset.saturating_add_signed(delta).min(max);
        let changed = next != position.offset;
        position.offset = next;
        position.follow = next == max;
        self.selected = Some(key);
        changed
    }

    /// 【终端】【链内高度】只有运行中的短链填满窗口，历史短链使用自然高度。
    /// 参数: `region` 为链范围；返回: 正文行数。
    fn height(&self, region: &ActivityRegion) -> usize {
        if region.active {
            self.body_rows
        } else {
            self.body_rows
                .min(region.end.saturating_sub(region.start + 1))
        }
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
        // 1. 【终端】【链内裁切】审批始终完整显示，普通链按运行状态决定是否保留固定高度
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
                if let Some(context) = window.context {
                    projection.copy(context..context + 1);
                }
                projection.copy(window.rows.clone());
                let rendered = window.rows.len() + usize::from(window.context.is_some());
                for _ in rendered..height {
                    projection.push(AnsiLine::new(String::new()));
                }
                visible.scrollable = region.active || window.max > 0;
                if visible.scrollable {
                    let focused = self.selected == Some(region.key);
                    visible.footer = Some(projection.len());
                    projection.push(activity_viewport_hints::footer_with_focus(
                        window.offset + 1,
                        window.offset + window.rows.len(),
                        region.end - region.start - 1,
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

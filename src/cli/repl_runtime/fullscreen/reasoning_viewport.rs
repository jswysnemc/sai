use super::activity_window::DocumentProjection;
use crate::render::transcript::{FullscreenDocument, ParagraphKey};
use std::collections::HashMap;

const BODY_ROWS: usize = 5;

struct Position {
    offset: usize,
    follow: bool,
    live: bool,
    manually_scrolled: bool,
}

/// 【终端】【思考窗口】保存完整正文与独立阅读位置，不把截断结果写回会话。
#[derive(Default)]
pub(super) struct ReasoningViewports {
    source: FullscreenDocument,
    positions: HashMap<usize, Position>,
    width: usize,
}

impl ReasoningViewports {
    /// 【终端】【思考定位】按轨道比例定位；参数为段落键和比例，返回是否改变位置
    pub(super) fn seek(&mut self, key: ParagraphKey, fraction: f64) -> bool {
        let Some(region) = self
            .source
            .reasonings
            .iter()
            .find(|region| region.key.cell == key.cell)
        else {
            return false;
        };
        let max = region.end.saturating_sub(region.start + 1 + BODY_ROWS);
        let Some(position) = self.positions.get_mut(&key.cell) else {
            return false;
        };
        let next = (max as f64 * fraction.clamp(0.0, 1.0)).round() as usize;
        let changed = position.offset != next;
        position.offset = next;
        position.follow = region.live && next == max;
        position.manually_scrolled = true;
        changed
    }
    /// 【终端】【思考暂停】判断是否存在已暂停跟随的实时思考块。
    /// 参数: 无；返回: 存在暂停项时为 true。
    pub(super) fn any_paused(&self) -> bool {
        self.positions
            .values()
            .any(|position| position.live && !position.follow)
    }

    /// 【终端】【主动查看】用户展开实时思考后暂停内部跟随，保持阅读位置。
    /// 参数: cell 为主动展开的思考单元下标；返回: 无。
    pub(super) fn pause_for_inspection(&mut self, cell: usize) {
        if let Some(position) = self.positions.get_mut(&cell) {
            if position.live {
                position.follow = false;
            }
        }
    }

    /// 【终端】【思考恢复】回到底部时恢复所有实时思考块的跟随状态。
    /// 参数: 无；返回: 无。
    pub(super) fn resume_live_follow(&mut self) {
        for position in self.positions.values_mut() {
            if position.live {
                position.follow = true;
            }
        }
    }

    /// 【终端】【思考边界】判断段落是否需要独立滚动，全文可见时交由主会话滚动。
    /// 参数: `key` 为思考段落键；返回: 正文是否超过五行窗口。
    pub(super) fn is_scrollable(&self, key: ParagraphKey) -> bool {
        self.source.reasonings.iter().any(|region| {
            region.key.cell == key.cell && region.end.saturating_sub(region.start + 1) > BODY_ROWS
        })
    }

    /// 【终端】【思考窗口】更新完整源文档；流式默认跟随，历史默认从开头阅读。
    /// 参数: `source` 为源文档，`width` 为调用方已完成排版的显示列数；返回: 每段正文至多五行的文档。
    pub(super) fn update(
        &mut self,
        source: FullscreenDocument,
        width: usize,
    ) -> FullscreenDocument {
        self.width = width;
        self.positions.retain(|cell, _| {
            source
                .reasonings
                .iter()
                .any(|region| region.key.cell == *cell)
                || source.retained_keys.iter().any(|key| key.cell == *cell)
                || source.paragraphs.iter().any(|span| span.key.cell == *cell)
        });
        self.source = source;
        self.project()
    }

    /// 【终端】【思考滚动】仅改变正文位置，边界返回未改变，让外层视口接管。
    /// 参数: `key` 为段落键，`delta` 为行偏移；返回: 是否产生实际位移。
    pub(super) fn scroll(&mut self, key: ParagraphKey, delta: isize) -> bool {
        let Some(region) = self
            .source
            .reasonings
            .iter()
            .find(|region| region.key.cell == key.cell)
        else {
            return false;
        };
        let max = region.end.saturating_sub(region.start + 1 + BODY_ROWS);
        let Some(position) = self.positions.get_mut(&key.cell) else {
            return false;
        };
        let next = position.offset.saturating_add_signed(delta).min(max);
        let changed = next != position.offset;
        position.offset = next;
        position.follow = region.live && next == max;
        position.manually_scrolled |= changed;
        changed
    }

    /// 【终端】【思考裁切】标题固定，正文按五行窗口投影，点击与执行链范围同步映射。
    /// 参数: 无；返回: 带双色引导线的可见文档。
    pub(super) fn project(&mut self) -> FullscreenDocument {
        let mut projection = DocumentProjection::new(&self.source);
        let mut cursor = 0;
        let mut scrollbars = Vec::new();
        let mut paused_titles = Vec::new();
        for region in &self.source.reasonings {
            let total = region.end.saturating_sub(region.start + 1);
            projection.copy(cursor..region.start);
            let title_row = projection.len();
            projection.copy(region.start..region.start + 1);
            let position = self.positions.entry(region.key.cell).or_insert(Position {
                offset: 0,
                follow: region.live,
                live: region.live,
                manually_scrolled: false,
            });
            // 1. 【终端】【思考结算】只重置自动阅读位置，保留用户滚动结果并停止历史跟随
            if position.live && !region.live {
                if !position.manually_scrolled {
                    position.offset = 0;
                }
                position.follow = false;
            }
            position.live = region.live;
            let max = total.saturating_sub(BODY_ROWS);
            position.offset = if position.follow {
                max
            } else {
                position.offset.min(max)
            };
            if position.live && !position.follow && max > 0 {
                paused_titles.push(title_row);
            }
            let first = region.start + 1 + position.offset;
            let body_start = projection.len();
            projection.copy(first..(first + BODY_ROWS).min(region.end));
            scrollbars.push((body_start..projection.len(), position.offset, total));
            cursor = region.end;
        }
        projection.copy(cursor..self.source.lines.len());
        projection.remap_activities();
        let mut document = projection.finish();
        // 2. 【终端】【思考暂停提示】实时流式思考被用户上滚暂停跟随时在标题追加轻量状态
        if !paused_titles.is_empty() {
            let limit = if self.width > 0 { self.width } else { 80 };
            let label = format!(" · {}", crate::i18n::text("paused", "已暂停跟随"));
            let label_width = unicode_width::UnicodeWidthStr::width(label.as_str());
            let muted =
                super::sidebar_palette::fg(super::sidebar_palette::SidebarPalette::current().muted);
            for title_row in paused_titles {
                if let Some(line) = document.lines.get_mut(title_row) {
                    let current = line.as_str();
                    if crate::render::ansi_clip::ansi_visible_width(current) + label_width <= limit
                    {
                        *line = crate::render::transcript::AnsiLine::new(format!(
                            "{current}{muted}{label}\x1b[0m"
                        ));
                    }
                }
            }
        }
        // 3. 【终端】【思考滚动条】只为投影正文着色，完整源文档保持原有样式
        for (rows, offset, total) in scrollbars {
            super::block_scrollbar::decorate(&mut document.lines[rows], offset, total);
        }
        document
    }
}

#[cfg(test)]
#[path = "reasoning_viewport_tests.rs"]
mod tests;

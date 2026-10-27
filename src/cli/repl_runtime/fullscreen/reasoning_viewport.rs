use super::activity_window::DocumentProjection;
use crate::render::transcript::{AnsiLine, FullscreenDocument, ParagraphKey};
use std::collections::HashMap;

const BODY_ROWS: usize = 5;

struct Position {
    offset: usize,
    follow: bool,
}

/// 【终端】【思考窗口】保存完整正文与独立阅读位置，不把截断结果写回会话。
#[derive(Default)]
pub(super) struct ReasoningViewports {
    source: FullscreenDocument,
    positions: HashMap<usize, Position>,
    width: usize,
}

impl ReasoningViewports {
    /// 【终端】【思考窗口】更新完整源文档；流式默认跟随，历史默认从开头阅读。
    /// 参数: `source` 为源文档，`width` 为显示列数；返回: 每段正文至多五行的文档。
    pub(super) fn update(
        &mut self,
        source: FullscreenDocument,
        width: usize,
    ) -> FullscreenDocument {
        self.positions.retain(|cell, _| {
            source
                .reasonings
                .iter()
                .any(|region| region.key.cell == *cell)
                || source.retained_keys.iter().any(|key| key.cell == *cell)
                || source.paragraphs.iter().any(|span| span.key.cell == *cell)
        });
        self.source = source;
        self.width = width;
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
        position.follow = next == max;
        changed
    }

    /// 【终端】【思考裁切】标题固定，正文按五行窗口投影，点击与执行链范围同步映射。
    /// 参数: 无；返回: 带位置提示的可见文档。
    pub(super) fn project(&mut self) -> FullscreenDocument {
        let mut projection = DocumentProjection::new(&self.source);
        let mut cursor = 0;
        let mut hints = Vec::new();
        for region in &self.source.reasonings {
            let total = region.end.saturating_sub(region.start + 1);
            projection.copy(cursor..region.start + 1);
            let title_row = projection.len().saturating_sub(1);
            let position = self.positions.entry(region.key.cell).or_insert(Position {
                offset: 0,
                follow: region.live,
            });
            let max = total.saturating_sub(BODY_ROWS);
            position.offset = if position.follow {
                max
            } else {
                position.offset.min(max)
            };
            let first = region.start + 1 + position.offset;
            projection.copy(first..(first + BODY_ROWS).min(region.end));
            if max > 0 {
                hints.push((
                    title_row,
                    format!(
                        " · {}-{}/{total} ↕",
                        position.offset + 1,
                        (position.offset + BODY_ROWS).min(total)
                    ),
                ));
            }
            cursor = region.end;
        }
        projection.copy(cursor..self.source.lines.len());
        projection.remap_activities();
        let mut document = projection.finish();
        for (row, hint) in hints {
            let hint_width = unicode_width::UnicodeWidthStr::width(hint.as_str());
            if self.width > hint_width + 12 {
                let title = crate::render::ansi_clip::clip_ansi_to_width(
                    document.lines[row].as_str(),
                    self.width - hint_width,
                    "…",
                );
                document.lines[row] = AnsiLine::new(format!("{title}\x1b[2m{hint}\x1b[0m"));
            }
        }
        document
    }
}

#[cfg(test)]
#[path = "reasoning_viewport_tests.rs"]
mod tests;

use super::activity_window::node_spans;
use crate::render::transcript::{ActivityRegion, AnsiLine, FullscreenDocument, ParagraphKey};

/// 以节点正文中的字符位置定位，忽略换行新增的引导线和空白，不依赖视觉行号。
pub(super) struct ActivityScrollAnchor {
    key: ParagraphKey,
    content_offset: Option<usize>,
}

impl ActivityScrollAnchor {
    /// 【终端】【阅读锚点】捕获暂停浏览时的首个可见内容位置。
    /// 参数: `source` 为完整源文档，`region` 为链范围，`offset` 为视觉行偏移；返回: 节点阅读锚点。
    pub(super) fn capture(
        source: &FullscreenDocument,
        region: &ActivityRegion,
        offset: usize,
    ) -> Option<Self> {
        let row = region.start + 1 + offset;
        let nodes = node_spans(source, region);
        let span = nodes.iter().find(|span| row < span.end)?;
        let content_offset = (row > span.start).then(|| {
            source.lines[span.start + 1..row]
                .iter()
                .map(content_chars)
                .sum()
        });
        Some(Self {
            key: span.key,
            content_offset,
        })
    }

    /// 【终端】【阅读恢复】在重新换行后的同一节点中恢复内容位置。
    /// 参数: `source` 为新源文档，`region` 为新链范围；返回: 对应视觉行偏移，节点消失时为空。
    pub(super) fn locate(
        &self,
        source: &FullscreenDocument,
        region: &ActivityRegion,
    ) -> Option<usize> {
        let nodes = node_spans(source, region);
        let span = nodes.iter().find(|span| span.key == self.key)?;
        let row = match self.content_offset {
            None => span.start,
            Some(target) => {
                let mut consumed = 0;
                (span.start + 1..span.end)
                    .find(|row| {
                        consumed += content_chars(&source.lines[*row]);
                        consumed > target
                    })
                    .unwrap_or_else(|| span.end.saturating_sub(1).max(span.start))
            }
        };
        Some(row.saturating_sub(region.start + 1))
    }
}

/// 【终端】【内容计数】去掉每个视觉行重复的外层引导线和空白，保留真实正文字符顺序。
/// 参数: `line` 为带颜色的视觉行；返回: 稳定的正文字符数。
fn content_chars(line: &AnsiLine) -> usize {
    let plain = crate::render::activity_animation::strip_ansi_for_test(line.as_str());
    let content = plain.trim_start().strip_prefix('│').unwrap_or(&plain);
    content
        .chars()
        .filter(|character| !character.is_whitespace())
        .count()
}

#[cfg(test)]
#[path = "activity_scroll_anchor_tests.rs"]
mod tests;

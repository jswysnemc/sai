use crate::render::transcript::{ActivityRegion, AnsiLine, FullscreenDocument, ParagraphSpan};
use std::ops::Range;

pub(super) struct ActivityWindow {
    pub(super) rows: Range<usize>,
    pub(super) context: Option<usize>,
    pub(super) offset: usize,
    pub(super) max: usize,
}

impl ActivityWindow {
    /// 【终端】【链内窗口】详情滚动越过标题时保留同一节点的标题，不另设详情面板。
    /// 参数: `source` 为完整文档，`region` 为链范围，`offset` 为目标位置，`height` 为窗口高度；返回: 行范围和上下文。
    pub(super) fn at(
        source: &FullscreenDocument,
        region: &ActivityRegion,
        offset: usize,
        height: usize,
    ) -> Self {
        let body = region.start + 1;
        let nodes = node_spans(source, region);
        let tail_context = height > 1
            && nodes.iter().any(|span| {
                span.start >= body
                    && span.end == region.end
                    && span.expanded
                    && span.control == span.start
                    && span.end - span.start > height
            });
        let capacity = height.saturating_sub(usize::from(tail_context));
        let max = region.end.saturating_sub(body + capacity);
        let offset = offset.min(max);
        let first = body + offset;
        let context = (height > 1)
            .then(|| {
                nodes
                    .iter()
                    .find(|span| {
                        span.start >= body
                            && span.start < first
                            && span.end > first
                            && span.expanded
                            && span.control == span.start
                    })
                    .map(|span| span.start)
            })
            .flatten();
        let last = (first + height.saturating_sub(usize::from(context.is_some()))).min(region.end);
        Self {
            rows: first..last,
            context,
            offset,
            max,
        }
    }
}

/// 【终端】【链内节点】源标题按行排序，二分定位当前链的节点，避免重复扫描完整历史。
/// 参数: `source` 为有序源文档，`region` 为链范围；返回: 仅包含当前链节点的切片。
pub(super) fn node_spans<'a>(
    source: &'a FullscreenDocument,
    region: &ActivityRegion,
) -> &'a [ParagraphSpan] {
    let first = source
        .paragraphs
        .partition_point(|span| span.start < region.start + 1);
    let last = source
        .paragraphs
        .partition_point(|span| span.start < region.end);
    &source.paragraphs[first..last]
}

/// 保存少量连续行映射，避免每个节点遍历全部历史分组。
pub(super) struct DocumentProjection<'a> {
    source: &'a FullscreenDocument,
    document: FullscreenDocument,
    chunks: Vec<(Range<usize>, usize)>,
}

impl<'a> DocumentProjection<'a> {
    /// 【终端】【视口投影】初始化源文档的可见行投影。
    /// 参数: `source` 为完整源文档；返回: 空投影。
    pub(super) fn new(source: &'a FullscreenDocument) -> Self {
        Self {
            source,
            document: FullscreenDocument::default(),
            chunks: Vec::new(),
        }
    }

    /// 【终端】【视口拼接】追加源行并登记可见坐标，保留原有语法颜色。
    /// 参数: `range` 为源行范围；返回: 无。
    pub(super) fn copy(&mut self, range: Range<usize>) {
        if !range.is_empty() {
            self.chunks.push((range.clone(), self.document.lines.len()));
            self.document.lines.extend(
                super::image_window::window_lines(
                    &self.source.lines,
                    range.start,
                    range.len(),
                    crate::render::terminal_image::kitty_cell_pixel_height(),
                )
                .into_iter()
                .map(AnsiLine::new),
            );
        }
    }

    /// 【终端】【视口拼接】追加没有源坐标的留白或提示。
    /// 参数: `line` 为附加行；返回: 无。
    pub(super) fn push(&mut self, line: AnsiLine) {
        self.document.lines.push(line);
    }

    /// 【终端】【视口坐标】读取已追加行数。
    /// 参数: 无；返回: 下一个可见行坐标。
    pub(super) fn len(&self) -> usize {
        self.document.lines.len()
    }

    /// 【终端】【视口范围】登记已裁切分组的鼠标交互范围。
    /// 参数: `region` 为可见分组；返回: 无。
    pub(super) fn region(&mut self, region: ActivityRegion) {
        self.document.activities.push(region);
    }

    /// 【终端】【坐标映射】合并保留标题与正文的点击范围，并保留不可见节点状态。
    /// 参数: 无；返回: 完整可见文档。
    pub(super) fn finish(mut self) -> FullscreenDocument {
        self.document.retained_keys = self.source.retained_keys.clone();
        self.document
            .retained_keys
            .extend(self.source.paragraphs.iter().map(|span| span.key));
        // 1. 【终端】【坐标映射】源区间按行排序，二分定位候选区间而非全表扫描
        for span in &self.source.paragraphs {
            let first = self
                .chunks
                .partition_point(|(range, _)| range.end <= span.start);
            let mut merged: Option<ParagraphSpan> = None;
            for (range, target) in self.chunks[first..]
                .iter()
                .take_while(|(range, _)| range.start < span.end)
            {
                let mut visible = span.clone();
                visible.start = target + span.start.max(range.start) - range.start;
                visible.end = target + span.end.min(range.end) - range.start;
                visible.control = if range.contains(&span.control) {
                    target + span.control - range.start
                } else {
                    usize::MAX
                };
                if let Some(previous) = merged
                    .as_mut()
                    .filter(|previous| previous.end == visible.start)
                {
                    previous.end = visible.end;
                    previous.control = previous.control.min(visible.control);
                } else {
                    if let Some(previous) = merged.take() {
                        self.document.paragraphs.push(previous);
                    }
                    merged = Some(visible);
                }
            }
            if let Some(visible) = merged {
                self.document.paragraphs.push(visible);
            }
        }
        // 2. 【终端】【坐标映射】普通消息始终保留，概览锚点转换到裁切后的实际位置
        for anchor in &self.source.anchors {
            let first = self
                .chunks
                .partition_point(|(range, _)| range.end <= anchor.row);
            if let Some((range, target)) = self
                .chunks
                .get(first)
                .filter(|(range, _)| range.contains(&anchor.row))
            {
                let mut visible = anchor.clone();
                visible.row = target + anchor.row - range.start;
                self.document.anchors.push(visible);
            }
        }
        self.document
    }
}

#[cfg(test)]
#[path = "activity_window_tests.rs"]
mod tests;

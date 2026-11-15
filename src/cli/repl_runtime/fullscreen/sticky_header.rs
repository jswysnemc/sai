//! 工具与思考标题在主视口中的固定显示，以及绘制和交互共用的坐标映射。

use super::state::FullscreenState;
use crate::render::transcript::{ParagraphPart, ParagraphSpan};

impl FullscreenState {
    /// 【终端】【固定标题】定位滚过顶部、正文仍可见的最内层工具或思考标题。
    /// 参数：无；返回：需要固定的段落，单行视口优先显示正文。
    pub(super) fn sticky_paragraph(&self) -> Option<&ParagraphSpan> {
        if self.layout.is_some_and(|layout| layout.body_height <= 1) {
            return None;
        }
        self.document
            .paragraphs
            .iter()
            .filter(|span| {
                span.expanded
                    && span.control == span.start
                    && span.start < self.scroll
                    && self.scroll < span.end
                    && matches!(span.key.part, ParagraphPart::Whole | ParagraphPart::Active)
            })
            .max_by_key(|span| span.start)
    }

    /// 【终端】【标题坐标】固定标题映射到原控制行，其余屏幕行保持文档位置。
    /// 参数：body_row 为相对正文顶部的屏幕行；返回：该行实际显示内容的文档行。
    pub(super) fn body_document_row(&self, body_row: usize) -> usize {
        // 1. 【终端】【固定标题】顶部覆盖行复用原标题，点击与悬停保持同一目标
        if body_row == 0 {
            if let Some(span) = self.sticky_paragraph() {
                return span.control;
            }
        }
        // 2. 【终端】【正文坐标】其余内容按主会话滚动位置显示
        self.scroll + body_row
    }
}

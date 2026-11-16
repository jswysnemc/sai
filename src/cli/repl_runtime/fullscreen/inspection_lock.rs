//! 工具详情查看保护：在展开或悬停阅读期间锁定视口与焦点，避免新输出抢占画面。

use super::state::FullscreenState;
use crate::render::transcript::{FullscreenDocument, ParagraphKey, ParagraphPart};
use std::collections::HashSet;

/// 【终端】【查看锚点】在正文重排前捕获用户正在查看的步骤位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InspectTarget {
    Step(usize),
    Reasoning(usize),
    Activity(ParagraphKey),
}

/// 【终端】【悬停锚点】记录指针命中的元素及其当前文档行，用于重排后保持屏幕行不动。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InspectAnchor {
    pub(super) target: InspectTarget,
    pub(super) row: usize,
}

impl FullscreenState {
    /// 【终端】【悬停识别】识别指针当前正在查看的工具步骤、思考块或执行链区域。
    ///
    /// 参数: 无
    ///
    /// 返回:
    /// - 命中可查看区域时返回查看锚点，拖选或底部按钮悬停时返回 None
    pub(super) fn hovered_inspect_anchor(&self) -> Option<InspectAnchor> {
        if self.press.is_some() || self.selection.is_some() || self.dragging {
            return None;
        }
        let body_row = self.pointer_row?;
        if let Some(layout) = self.layout {
            let content_width = usize::from(layout.content_width);
            let body_height = usize::from(layout.body_height);
            if self
                .pointer_col
                .is_some_and(|col| usize::from(col) >= content_width)
            {
                return None;
            }
            if super::bottom_button::is_hovered(self, content_width, body_height) {
                return None;
            }
        }
        let doc_row = self.body_document_row(body_row);
        if self.hovered_activity_footer().is_some() {
            return None;
        }
        if let Some(span) = self.document.paragraph_at(doc_row) {
            if !matches!(
                span.key.part,
                ParagraphPart::ActivityGroup | ParagraphPart::OlderActivity
            ) {
                return Some(InspectAnchor {
                    target: InspectTarget::Step(span.key.cell),
                    row: span.start,
                });
            }
        }
        if let Some(region) = self
            .document
            .reasonings
            .iter()
            .find(|region| doc_row >= region.start && doc_row < region.end)
        {
            return Some(InspectAnchor {
                target: InspectTarget::Reasoning(region.key.cell),
                row: region.start,
            });
        }
        if let Some(region) = self
            .document
            .activities
            .iter()
            .find(|region| doc_row >= region.start && doc_row < region.end)
        {
            return Some(InspectAnchor {
                target: InspectTarget::Activity(region.key),
                row: region.start,
            });
        }
        None
    }

    /// 【终端】【折叠保护】收集当前渲染周期需要冻结折叠与固定展示的步骤下标。
    ///
    /// 参数: 无
    ///
    /// 返回:
    /// - `(是否冻结近期自动折叠, 不可隐藏或折叠的单元集合)`
    pub(super) fn inspection_protection(&self) -> (bool, HashSet<usize>) {
        let hovered = self.hovered_inspect_anchor();
        let pause_active = !self.follow
            || !self.activity.all_following()
            || self.reasoning.any_paused()
            || hovered.is_some()
            || !self.pinned_cells.is_empty();
        let mut pinned = self.pinned_cells.clone();
        // 1. 【终端】【悬停保护】悬停所在步骤与思考块在刷新期间保持展开并留在可见区
        if let Some(anchor) = hovered {
            match anchor.target {
                InspectTarget::Step(cell) | InspectTarget::Reasoning(cell) => {
                    pinned.insert(cell);
                }
                InspectTarget::Activity(_) => {}
            }
        }
        // 2. 【终端】【手动展开】已写入全文展开键的步骤在任何刷新下都不会被收起
        for key in &self.expanded {
            if key.part == ParagraphPart::Whole {
                pinned.insert(key.cell);
            }
        }
        // 3. 【终端】【阅读保护】暂停跟随或悬停查看时，保住屏幕上已展开的近期详情
        if pause_active {
            for span in &self.document.paragraphs {
                if span.expanded
                    && !matches!(
                        span.key.part,
                        ParagraphPart::ActivityGroup | ParagraphPart::OlderActivity
                    )
                {
                    pinned.insert(span.key.cell);
                }
            }
        }
        (pause_active, pinned)
    }

    /// 【终端】【刷新保护】在同步新文档前冻结悬停中的单元与内部执行链跟随。
    ///
    /// 参数: 无
    ///
    /// 返回:
    /// - 刷新前捕获的悬停锚点
    pub(super) fn prepare_inspection_refresh(&mut self) -> Option<InspectAnchor> {
        let anchor = self.hovered_inspect_anchor();
        if let Some(InspectAnchor {
            target: InspectTarget::Step(cell) | InspectTarget::Reasoning(cell),
            ..
        }) = anchor
        {
            self.pinned_cells.insert(cell);
        }
        if !self.follow || !self.activity.all_following() || anchor.is_some() {
            for span in &self.document.paragraphs {
                if span.expanded
                    && !matches!(
                        span.key.part,
                        ParagraphPart::ActivityGroup | ParagraphPart::OlderActivity
                    )
                {
                    self.pinned_cells.insert(span.key.cell);
                }
            }
        }
        anchor
    }

    /// 【终端】【悬停定位】计算新文档中对应锚点的起始行，保持正在查看的元素留在原屏行。
    ///
    /// 参数:
    /// - `document`: 新渲染的投影文档
    /// - `anchor`: 刷新前捕获的悬停锚点
    ///
    /// 返回:
    /// - 新文档中的起始行；元素已消失时为 None
    pub(super) fn locate_inspect_anchor(
        document: &FullscreenDocument,
        anchor: InspectAnchor,
    ) -> Option<usize> {
        match anchor.target {
            InspectTarget::Step(cell) => document
                .paragraphs
                .iter()
                .find(|span| {
                    span.key.cell == cell
                        && !matches!(
                            span.key.part,
                            ParagraphPart::ActivityGroup | ParagraphPart::OlderActivity
                        )
                })
                .map(|span| span.start),
            InspectTarget::Reasoning(cell) => document
                .reasonings
                .iter()
                .find(|region| region.key.cell == cell)
                .map(|region| region.start),
            InspectTarget::Activity(key) => document
                .activities
                .iter()
                .find(|region| region.key == key)
                .map(|region| region.start),
        }
    }

    /// 【终端】【恢复跟随】清理查看保护状态并恢复内外层实时跟随，通常由回到底部触发。
    ///
    /// 参数: 无
    ///
    /// 返回:
    /// - 是否清理了此前的查看锁定并需要重绘文档以恢复默认折叠状态
    pub(super) fn release_inspection_locks(&mut self) -> bool {
        let had_locks = !self.pinned_cells.is_empty()
            || !self.activity.all_following()
            || self.reasoning.any_paused();
        self.pinned_cells.clear();
        self.activity.resume_tail_follow();
        self.reasoning.resume_live_follow();
        had_locks
    }
}

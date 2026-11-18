//! 工具详情查看保护：仅主动展开或手动滚动时保持阅读位置。

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

/// 【终端】【阅读锚点】记录主动展开元素的文档行，用于重排后保持屏幕位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InspectAnchor {
    pub(super) target: InspectTarget,
    pub(super) row: usize,
}

impl FullscreenState {
    /// 【终端】【主动阅读】寻找当前可见且由用户主动展开的工具或思考节点。
    /// 参数: 无；返回: 阅读锚点，普通悬停不产生锚点。
    pub(super) fn inspection_anchor(&self) -> Option<InspectAnchor> {
        let end = self.scroll
            + self.layout.map_or(usize::MAX - self.scroll, |layout| {
                usize::from(layout.body_height)
            });
        // 1. 【终端】【展开识别】仅使用手动展开键或固定集合，不根据指针位置推断阅读意图
        self.document
            .paragraphs
            .iter()
            .find_map(|span| {
                let manual = self.pinned_cells.contains(&span.key.cell)
                    || self.expanded.contains(&span.key)
                    || self.expanded.contains(&ParagraphKey::whole(span.key.cell));
                if !manual
                    || !span.expanded
                    || span.end <= self.scroll
                    || span.start >= end
                    || matches!(
                        span.key.part,
                        ParagraphPart::ActivityGroup | ParagraphPart::OlderActivity
                    )
                {
                    return None;
                }
                let target = if self
                    .document
                    .reasonings
                    .iter()
                    .any(|region| region.key.cell == span.key.cell)
                {
                    InspectTarget::Reasoning(span.key.cell)
                } else {
                    InspectTarget::Step(span.key.cell)
                };
                Some(InspectAnchor {
                    target,
                    row: span.start,
                })
            })
            .or_else(|| {
                // 2. 【终端】【链内阅读】手动滚动执行链时保留区域锚点，正常跟随不锁定
                if self.activity.all_following() {
                    return None;
                }
                self.document
                    .activities
                    .iter()
                    .find(|region| {
                        Some(region.key) == self.activity.selected
                            && region.end > self.scroll
                            && region.start < end
                    })
                    .map(|region| InspectAnchor {
                        target: InspectTarget::Activity(region.key),
                        row: region.start,
                    })
            })
    }

    /// 【终端】【折叠保护】收集当前渲染周期需要冻结折叠与固定展示的步骤下标。
    ///
    /// 参数: 无
    ///
    /// 返回:
    /// - `(是否冻结近期自动折叠, 不可隐藏或折叠的单元集合)`
    pub(super) fn inspection_protection(&self) -> (bool, HashSet<usize>) {
        let pause_active = !self.follow
            || !self.activity.all_following()
            || self.reasoning.any_paused()
            || !self.pinned_cells.is_empty();
        let mut pinned = self.pinned_cells.clone();
        // 1. 【终端】【手动展开】已写入全文展开键的步骤在任何刷新下都不会被收起
        for key in &self.expanded {
            if key.part == ParagraphPart::Whole {
                pinned.insert(key.cell);
            }
        }
        // 2. 【终端】【阅读保护】手动暂停跟随时保住屏幕上已展开的近期详情
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

    /// 【终端】【刷新保护】在同步新文档前捕获阅读锚点，仅在已暂停跟随时锁定当前展开步骤。
    ///
    /// 参数: 无
    ///
    /// 返回:
    /// - 刷新前捕获的阅读锚点
    pub(super) fn prepare_inspection_refresh(&mut self) -> Option<InspectAnchor> {
        let anchor = self.inspection_anchor();
        // 1. 【终端】【阅读锁定】仅在用户暂停跟随时持久化已展开步骤
        if !self.follow || !self.activity.all_following() {
            self.lock_visible_inspection(anchor);
        }
        anchor
    }

    /// 【终端】【阅读锁定】将当前展开步骤与画面内已展开节点写入持久固定集合，直到恢复底部。
    ///
    /// 参数:
    /// - `anchor`: 当前阅读锚点
    ///
    /// 返回:
    /// - 无
    pub(super) fn lock_visible_inspection(&mut self, anchor: Option<InspectAnchor>) {
        if let Some(InspectAnchor {
            target: InspectTarget::Step(cell) | InspectTarget::Reasoning(cell),
            ..
        }) = anchor
        {
            self.pinned_cells.insert(cell);
        }
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

    /// 【终端】【阅读定位】计算新文档中对应锚点的起始行，保持正在查看的元素留在原屏行。
    ///
    /// 参数:
    /// - `document`: 新渲染的投影文档
    /// - `anchor`: 刷新前捕获的阅读锚点
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

    /// 【终端】【恢复跟随】清理查看保护状态与执行链临时展开键，通常由回到底部触发。
    ///
    /// 参数: 无
    ///
    /// 返回:
    /// - 是否清理了此前的查看锁定并需要重绘文档以恢复默认折叠状态
    pub(super) fn release_inspection_locks(&mut self) -> bool {
        let before_expanded = self.expanded.len();
        // 1. 【终端】【恢复自动收缩】回到实时底部时仅解除执行链步骤与较早步骤的临时查看展开，保留消息主体折叠状态
        let retained = &self.document.retained_keys;
        self.expanded.retain(|key| match key.part {
            ParagraphPart::OlderActivity => false,
            ParagraphPart::Whole => !retained.contains(&ParagraphKey {
                cell: key.cell,
                part: ParagraphPart::Active,
            }),
            ParagraphPart::Active | ParagraphPart::ActivityGroup => true,
        });
        let had_locks = !self.pinned_cells.is_empty()
            || !self.activity.all_following()
            || self.reasoning.any_paused()
            || self.expanded.len() != before_expanded;
        self.pinned_cells.clear();
        self.activity.resume_tail_follow();
        self.reasoning.resume_live_follow();
        had_locks
    }
}

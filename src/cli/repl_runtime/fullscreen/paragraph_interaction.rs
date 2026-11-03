//! 全屏段落展开、正文收起与操作步骤的屏幕位置锚定。

use super::state::FullscreenState;
use crate::render::transcript::{ParagraphKey, ParagraphPart, ParagraphSpan};

impl FullscreenState {
    /// 【终端】【段落切换】折叠提示支持展开，展开区域支持收起，执行链锚定当前步骤。
    ///
    /// 拖选由输入层优先处理，此处只处理已经确认的单击。
    ///
    /// 参数:
    /// - `row`: 被点击的文档行
    ///
    /// 返回:
    /// - 被切换的段落键与点击时标题行在屏幕上的偏移；未命中为 None
    pub(super) fn toggle_at(&mut self, row: usize) -> Option<(ParagraphKey, isize)> {
        // 1. 【终端】【区域命中】折叠状态只响应控件，展开状态响应整个节点区域
        let span = self
            .document
            .paragraph_at(row)
            .filter(|span| span.expanded || span.control == row)?
            .clone();
        // 2. 【终端】【步骤位置】记录当前步骤的屏幕偏移和链内位置
        let anchor = span.start;
        let screen_offset = anchor as isize - self.scroll as isize;
        let region = self
            .document
            .activities
            .iter()
            .find(|region| span.start >= region.start && span.start < region.end);
        let body_offset = region
            .map(|region| anchor.saturating_sub(region.start + 1))
            .unwrap_or(0);
        self.toggle_expansion(&span, region.is_some());
        self.activity.focus_at(span.key, body_offset);
        // 3. 【终端】【操作锚定】停止主会话跟随，避免新输出移动所操作步骤
        self.follow = false;
        Some((span.key, screen_offset))
    }

    /// 【终端】【展开状态】协调近期节点和全文键，收起时同时退出完整输出模式。
    /// 参数: `span` 为命中的段落，`in_activity` 为是否属于执行链；返回: 无。
    fn toggle_expansion(&mut self, span: &ParagraphSpan, in_activity: bool) {
        if in_activity && matches!(span.key.part, ParagraphPart::Whole | ParagraphPart::Active) {
            let full = ParagraphKey::whole(span.key.cell);
            let active = ParagraphKey {
                cell: span.key.cell,
                part: ParagraphPart::Active,
            };
            if span.expanded {
                self.expanded.remove(&full);
                self.expanded.insert(active);
            } else if span.key.part == ParagraphPart::Whole || span.control > span.start {
                self.expanded.remove(&active);
                self.expanded.insert(full);
            } else {
                self.expanded.remove(&active);
                self.expanded.remove(&full);
            }
        } else if !self.expanded.remove(&span.key) {
            self.expanded.insert(span.key);
        }
    }

    /// 【终端】【位置恢复】重排后让操作步骤或普通段落标题保持原来的屏幕位置。
    ///
    /// 普通段落的末尾收起控件可能位于标题下方；标题已不可见时恢复到正文顶部。
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
        key: ParagraphKey,
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

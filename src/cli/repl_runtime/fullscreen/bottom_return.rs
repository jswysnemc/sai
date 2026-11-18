//! 点击“回到底部 / 有新输出”时的详情清理策略：保留底部可视区域内的展开详情，收起离屏已查看详情并恢复自动收起。

use super::activity_window::body_height_budget;
use super::state::FullscreenState;
use crate::render::transcript::ParagraphPart;
use std::collections::HashSet;

/// 与执行链近期详情自动折叠窗口一致的行数阈值。
const RECENT_DETAIL_WINDOW_ROWS: usize = 10;

impl FullscreenState {
    /// 【全屏视图】【回到底部清理】跳至底部，仅保留最下方可视区域内的展开工具详情，将其余已展开工具恢复折叠。
    ///
    /// 参数:
    /// - `body_height`: 正文区可见行数
    /// - `terminal_rows`: 终端总行数
    ///
    /// 返回:
    /// - 需要写入 `TranscriptStore::fold_offscreen_cells` 回归自动收起的离屏步骤下标列表
    pub(super) fn return_to_bottom_closing_offscreen(
        &mut self,
        body_height: usize,
        terminal_rows: usize,
    ) -> Vec<usize> {
        let total_lines = self.document.lines.len();
        let bottom_top = total_lines.saturating_sub(body_height);
        let activity_budget = body_height_budget(body_height, terminal_rows);

        // 1. 【全屏视图】【底屏可视判定】识别落在最下方可见区域且未被执行链子视口推离尾部的步骤单元
        let mut visible_bottom_cells = HashSet::new();
        let mut offscreen_open_cells = Vec::new();
        let activity_cells: HashSet<usize> = self
            .document
            .retained_keys
            .iter()
            .filter(|key| key.part == ParagraphPart::Active)
            .map(|key| key.cell)
            .collect();

        for span in &self.document.paragraphs {
            if matches!(
                span.key.part,
                ParagraphPart::ActivityGroup | ParagraphPart::OlderActivity
            ) {
                continue;
            }
            if !activity_cells.contains(&span.key.cell) {
                continue;
            }
            let in_outer_bottom = span.end > bottom_top;
            let region = self
                .document
                .activities
                .iter()
                .find(|region| span.start >= region.start && span.start < region.end);
            let in_activity_bottom = match region {
                Some(region) => {
                    let has_older_fold = self.document.paragraphs.iter().any(|item| {
                        item.key.part == ParagraphPart::OlderActivity
                            && item.start >= region.start
                            && item.start < region.end
                    });
                    let rows_after = region.end.saturating_sub(span.end);
                    let limit = if has_older_fold {
                        RECENT_DETAIL_WINDOW_ROWS
                    } else {
                        activity_budget.max(RECENT_DETAIL_WINDOW_ROWS)
                    };
                    rows_after < limit
                }
                None => true,
            };

            if in_outer_bottom && in_activity_bottom {
                visible_bottom_cells.insert(span.key.cell);
            } else if span.expanded {
                offscreen_open_cells.push(span.key.cell);
            }
        }

        // 2. 【全屏视图】【展开键收敛】移除底部可视区外的工具全文展开键、历史步骤展开键与手动折叠覆盖键
        self.expanded.retain(|key| match key.part {
            ParagraphPart::OlderActivity => self
                .document
                .paragraphs
                .iter()
                .find(|span| span.key == *key)
                .is_some_and(|span| span.end > bottom_top),
            ParagraphPart::Whole => {
                if activity_cells.contains(&key.cell) {
                    visible_bottom_cells.contains(&key.cell)
                } else {
                    true
                }
            }
            ParagraphPart::Active => visible_bottom_cells.contains(&key.cell),
            ParagraphPart::ActivityGroup => true,
        });

        // 3. 【全屏视图】【恢复实时跟随】清空查看固定锁，恢复执行链与思考块尾随并强制重排正文
        self.pinned_cells.clear();
        self.activity.resume_tail_follow();
        self.reasoning.resume_live_follow();
        self.scroll = self.max_scroll(body_height);
        self.follow = true;
        self.unseen = false;
        self.resuming_bottom = true;
        self.pending_refresh = true;
        offscreen_open_cells
    }
}

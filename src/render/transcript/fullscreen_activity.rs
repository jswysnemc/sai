//! 全屏视图的临时状态只进入固定面板，不进入可滚动正文。

use super::store::{TranscriptStore, TranscriptView};
use super::AnsiLine;
use crate::render::content_indent::align_to_guide_column_with_width;
use crate::render::work_status::WorkStatus;

impl TranscriptStore {
    /// 【全屏视图】【固定状态】组装固定在输入框上方的工作状态，限制在一行以内。
    /// @param width 为固定面板宽度
    /// @returns 不随正文滚动的状态行；没有活动时为空
    pub(crate) fn fullscreen_activity_lines(&self, width: usize) -> Vec<String> {
        let frame = self.live_animation_frame();
        let elapsed = self
            .work_status_started
            .map(|started| started.elapsed())
            .unwrap_or_default();
        // 1. 【全屏视图】【状态来源】子会话只显示自己的运行状态，不冒用主会话状态
        if matches!(self.view, TranscriptView::Subagent { .. }) {
            return if self.viewing_running_subagent() {
                status_row(&WorkStatus::Working.render_line(frame, elapsed), width)
            } else {
                Vec::new()
            };
        }
        // 2. 【全屏视图】【工作状态】思考与工具进度留在执行链内，固定区只显示工作状态
        self.work_status
            .map(|status| status_row(&status.render_line(frame, elapsed), width))
            .unwrap_or_default()
    }
}

/// 【全屏视图】【状态限宽】取摘要首个非空视觉行，避免长参数撑高固定区。
/// @param text 为状态或工具摘要；width 为可用列数
/// @returns 至多一行、带统一引导缩进的文本
fn status_row(text: &str, width: usize) -> Vec<String> {
    let padding = crate::render::content_indent::CONTENT_LEFT_INDENT.min(width.saturating_sub(1));
    AnsiLine::wrap_block(text, width.saturating_sub(padding).max(1))
        .into_iter()
        .find(|line| !super::spacing::is_visual_blank(line))
        .map(|line| vec![align_to_guide_column_with_width(line.as_str(), padding)])
        .unwrap_or_default()
}

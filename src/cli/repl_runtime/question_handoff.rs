use super::ReplRuntime;
use crate::question_tui::OverlayPlacement;
use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    queue,
    style::Print,
    terminal::{Clear, ClearType},
};

impl ReplRuntime {
    /// 【终端提问】【返回主屏】无参数，返回交互结束后的主屏同步结果。
    pub(in crate::cli) fn finish_question_prompt(&mut self) -> Result<()> {
        self.question_overlay_height = None;
        self.last_composer_signature = None;
        self.sync_transcript(true)
    }

    /// 【终端提问】【底部覆盖】无参数，返回当前屏幕应绘制提问卡片的固定区域。
    pub(in crate::cli) fn question_overlay_placement(&self) -> OverlayPlacement {
        let size = self.viewport.size();
        if self.fullscreen.is_some() {
            return OverlayPlacement::slot(
                size.cols,
                size.rows,
                self.viewport.composer_top(),
                self.viewport.composer_height(),
            );
        }
        OverlayPlacement::inline(size.cols, size.rows, self.viewport.composer_top())
    }

    /// 【终端提问】【界面交接】暂停输出并清底部覆盖区旧底色，保留草稿供提问结束后恢复。
    /// @returns 绘制结果；无参数
    pub(in crate::cli) fn prepare_question_prompt(&mut self) -> Result<()> {
        if self.fullscreen.is_some() {
            // 1. 【终端提问】【底部覆盖】全屏把提问高度当作底部输入槽，正文仍画在上方
            let size = self.viewport.size();
            self.question_overlay_height =
                Some(OverlayPlacement::bottom(size.cols, size.rows).panel_lines);
        }
        self.pause_for_permission_prompt()?;
        let placement = self.question_overlay_placement();
        queue!(
            self.frame,
            Print("\x1b[0m"),
            MoveTo(0, placement.anchor_y),
            Clear(ClearType::FromCursorDown)
        )?;
        self.last_composer_signature = None;
        self.commit_frame()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentMode;
    use crate::cli::repl_chrome::ReplChrome;
    use crate::render::transcript::TranscriptRenderOptions;

    /// 【终端提问】【残留回归】交接撤下输入区绘制基线而不丢失草稿；无参数或返回值。
    #[test]
    fn question_handoff_invalidates_old_composer_paint() {
        let mut runtime = ReplRuntime::new(
            1000,
            TranscriptRenderOptions {
                reasoning_mode: crate::render::ReasoningDisplayMode::Summary,
                tool_call_mode: crate::render::ToolCallDisplayMode::Summary,
            },
        );
        let chrome = ReplChrome {
            mode: AgentMode::Yolo,
            context_ratio: 0.0,
            context_window_tokens: 1000,
            model: "fixture".into(),
            thinking: "auto".into(),
            directory: "/tmp".into(),
            branch: None,
            cache_hit_ratio: None,
            status_plugin: None,
        };
        runtime
            .update_composer(&chrome, "saved draft", 4, false, Vec::new(), 0)
            .unwrap();
        runtime.draw_composer().unwrap();
        assert!(runtime.last_composer_signature.is_some());
        runtime.prepare_question_prompt().unwrap();
        assert!(runtime.last_composer_signature.is_none());
        assert!(runtime.composer.is_some());
        assert!(runtime.fullscreen.is_none());
        let placement = runtime.question_overlay_placement();
        assert!(placement.anchor_y <= runtime.viewport.composer_top());
        assert!(placement.anchor_y + placement.panel_lines <= 24);
    }

    /// 【终端提问】【底部覆盖】全屏提问不退出会话，覆盖区贴在底部输入槽。
    #[test]
    fn question_handoff_keeps_fullscreen_and_pins_overlay() {
        let mut runtime = ReplRuntime::new(
            1000,
            TranscriptRenderOptions {
                reasoning_mode: crate::render::ReasoningDisplayMode::Summary,
                tool_call_mode: crate::render::ToolCallDisplayMode::Summary,
            },
        );
        let chrome = ReplChrome {
            mode: AgentMode::Yolo,
            context_ratio: 0.0,
            context_window_tokens: 1000,
            model: "fixture".into(),
            thinking: "auto".into(),
            directory: "/tmp".into(),
            branch: None,
            cache_hit_ratio: None,
            status_plugin: None,
        };
        runtime
            .update_composer(&chrome, "saved draft", 4, false, Vec::new(), 0)
            .unwrap();
        runtime.enter_fullscreen().unwrap();
        runtime.prepare_question_prompt().unwrap();
        assert!(runtime.fullscreen.is_some());
        assert_eq!(runtime.question_overlay_height, Some(16));
        let placement = runtime.question_overlay_placement();
        assert_eq!(placement.anchor_y, runtime.viewport.composer_top());
        assert_eq!(placement.panel_lines, runtime.viewport.composer_height());
        assert_eq!(placement.anchor_y + placement.panel_lines, 24);
        runtime.finish_question_prompt().unwrap();
        assert!(runtime.fullscreen.is_some());
        assert!(runtime.question_overlay_height.is_none());
    }
}

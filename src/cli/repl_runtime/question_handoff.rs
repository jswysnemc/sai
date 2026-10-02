use super::ReplRuntime;
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
        self.last_composer_signature = None;
        self.sync_transcript(true)
    }

    /// 【终端提问】【界面交接】暂停输出并从输入框顶部撤下旧底色，保留草稿供提问结束后恢复。
    /// @returns 绘制结果；无参数
    pub(in crate::cli) fn prepare_question_prompt(&mut self) -> Result<()> {
        self.pause_for_permission_prompt()?;
        if self.composer.is_some() {
            // 1. 光标在输入正文中间，必须从整个输入区顶部清理，不能只清光标以下
            queue!(
                self.frame,
                Print("\x1b[0m"),
                MoveTo(0, self.viewport.composer_top()),
                Clear(ClearType::FromCursorDown)
            )?;
            self.last_composer_signature = None;
            self.commit_frame()?;
        }
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
    }
}

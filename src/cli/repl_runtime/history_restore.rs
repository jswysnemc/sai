use super::*;
use crate::render::transcript::DisplayWindow;

impl ReplRuntime {
    /// 【会话载入】【完整重放】将已载入历史依次写入终端并同步输入区位置。
    /// 参数: 无
    /// 返回: 终端绘制结果
    pub(super) fn restore_history(&mut self) -> Result<()> {
        self.commit_frame()?;
        let size = TerminalSize::current();
        let composer_height = self.composer_height_for(size);
        let (window, painted) = write_history(
            &mut self.frame,
            &mut self.transcript,
            &self.options,
            &mut self.viewport,
            size,
            composer_height,
        )?;
        // 1. 【会话载入】【滚动记账】完整输出后只将可见尾部计为屏幕内内容
        self.transcript.clear_dirty();
        self.stream.reset(&window, painted);
        self.last_composer_signature = None;
        self.queue_composer()?;
        self.commit_frame()?;
        self.reflow.clear_pending();
        self.reflow.mark_reflowed(size, false);
        Ok(())
    }
}

/// 【会话载入】【历史输出】按当前显示配置输出所有历史，不受窗口行数限制。
/// 参数: output 为绘制目标，transcript 为历史源，options 为展示配置，viewport 为布局状态，size 为终端尺寸，composer_height 为输入区高度
/// 返回: 完整窗口和实际留在屏幕上的历史行数
fn write_history<W: Write>(
    output: &mut W,
    transcript: &mut TranscriptStore,
    options: &TranscriptRenderOptions,
    viewport: &mut InlineViewport,
    size: TerminalSize,
    composer_height: u16,
) -> Result<(DisplayWindow, usize)> {
    let window = layout::display_window(
        transcript,
        usize::from(size.cols),
        options,
        usize::MAX,
        0,
        usize::MAX,
    );
    viewport.restart_at(size, 0);
    viewport.update(size, composer_height, window.total);
    let painted = reflow::replay_full(output, viewport, &window.lines)?;
    viewport.update(size, composer_height, painted);
    Ok((window, painted))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【会话载入】【回滚验证】首尾历史全部写入终端，输入框出现后无需重复输出；无参数，无返回值
    #[test]
    fn restored_history_writes_every_row_beyond_viewport_and_row_cap() {
        let options = TranscriptRenderOptions {
            reasoning_mode: crate::render::ReasoningDisplayMode::Summary,
            tool_call_mode: crate::render::ToolCallDisplayMode::Summary,
        };
        for composer_height in [0, 4] {
            let mut runtime = ReplRuntime::new(5, options);
            for index in 0..100 {
                runtime.transcript.push_meta(format!("history-{index:03}"));
            }
            let mut bytes = Vec::new();
            let size = TerminalSize::current();
            let (window, painted) = write_history(
                &mut bytes,
                &mut runtime.transcript,
                &options,
                &mut runtime.viewport,
                size,
                composer_height,
            )
            .unwrap();
            let output = String::from_utf8(bytes).unwrap();
            for index in 0..100 {
                assert_eq!(output.matches(&format!("history-{index:03}")).count(), 1);
            }
            assert!(!output.contains("\x1b[3J"));
            assert_eq!(window.start, 0);
            assert_eq!(painted, 24 - usize::from(composer_height.max(1)));
            runtime.transcript.clear_dirty();
            runtime.stream.reset(&window, painted);
            let next = layout::display_window(
                &mut runtime.transcript,
                80,
                &options,
                64,
                runtime.stream.offscreen(),
                12,
            );
            assert!(matches!(runtime.stream.sync(&next), SyncPlan::Unchanged));
        }
    }
}

use super::{reflow, InlineViewport, ReplRuntime, TerminalSize};
use anyhow::Result;

impl ReplRuntime {
    /// 【终端】【尺寸预览】即时调整可见区域，延后源码重排与滚动记账
    /// 参数: size 为新尺寸；返回绘制结果，不修改稳定历史快照
    pub(super) fn preview_resize(&mut self, size: TerminalSize) -> Result<()> {
        let height = self.composer_height_for(size);
        let lines = self
            .stream
            .preview_lines(usize::from(size.cols), usize::from(size.rows));
        let mut preview = self.viewport;
        let origin = preview.origin_row().min(
            size.rows
                .saturating_sub(height)
                .saturating_sub(lines.len().min(usize::from(size.rows)) as u16),
        );
        preview.restart_at(size, origin);
        preview.update(size, height, lines.len());
        reflow::replay(&mut self.frame, &preview, &lines)?;
        self.resize_preview = Some(preview);
        self.last_composer_signature = None;
        self.queue_composer()?;
        self.commit_frame()
    }

    /// 返回实际绘制输入框的视口，无参数；预览期间使用临时尺寸
    pub(super) fn drawing_viewport(&self) -> InlineViewport {
        self.resize_preview.unwrap_or(self.viewport)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::transcript::{AnsiLine, DisplayWindow};

    /// 尺寸预览只换行少量已渲染文本，不改变原生滚动进度
    #[test]
    fn preview_keeps_history_accounting_and_bounds_work() {
        let mut stream = super::super::StreamState::default();
        let lines = (0..1000)
            .map(|i| AnsiLine::new(format!("ROW-{i:04} value")))
            .collect();
        stream.reset(
            &DisplayWindow {
                total: 1000,
                start: 0,
                lines,
                dirty_from: 0,
            },
            20,
        );
        let offscreen = stream.offscreen();
        let preview = stream.preview_lines(10, 24);
        assert!(preview.len() <= 48);
        assert!(preview
            .iter()
            .any(|line| line.as_str().contains("ROW-0999")));
        assert_eq!(stream.offscreen(), offscreen);
        let size = TerminalSize { cols: 10, rows: 24 };
        let mut viewport = InlineViewport::new();
        viewport.restart_at(size, 0);
        viewport.update(size, 4, preview.len());
        let mut output = Vec::new();
        reflow::replay(&mut output, &viewport, &preview).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(!output.contains('\n'));
        assert!(!output.contains("\x1b[3J"));
    }
}

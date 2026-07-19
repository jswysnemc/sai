use super::command_result_block::render_live_command_output_for_cli;
use super::streaming_replace::{clear_rendered_rows, rendered_visual_rows};
use crate::render::tool_view::command_output_buffer::CommandOutputBuffer;
use crate::tools::command::{CommandOutputChunk, CommandOutputStream};
use anyhow::Result;
use std::io::{self, Write};

/// 保存普通 CLI 当前前台命令的有限实时输出。
pub(crate) struct CliCommandPreview {
    stdout: CommandOutputBuffer,
    stderr: CommandOutputBuffer,
    rendered_rows: usize,
}

impl CliCommandPreview {
    /// 创建空的 CLI 命令输出预览。
    ///
    /// 返回:
    /// - 新的命令输出预览
    pub(crate) fn new() -> Self {
        Self {
            stdout: CommandOutputBuffer::default(),
            stderr: CommandOutputBuffer::default(),
            rendered_rows: 0,
        }
    }

    /// 开始新的前台命令输出预览。
    pub(crate) fn begin(&mut self) {
        self.stdout = CommandOutputBuffer::default();
        self.stderr = CommandOutputBuffer::default();
        self.rendered_rows = 0;
    }

    /// 追加命令输出分块并重绘五行摘要。
    ///
    /// 参数:
    /// - `chunk`: 命令输出分块
    ///
    /// 返回:
    /// - 重绘是否发生
    pub(crate) fn append(&mut self, chunk: &CommandOutputChunk) -> Result<bool> {
        let target = match chunk.stream {
            CommandOutputStream::Stdout => &mut self.stdout,
            CommandOutputStream::Stderr => &mut self.stderr,
        };
        target.append(&chunk.bytes, chunk.omitted_bytes);
        self.redraw()
    }

    /// 清除当前实时摘要并释放终端行。
    ///
    /// 返回:
    /// - 清除是否成功
    pub(crate) fn clear(&mut self) -> Result<()> {
        if self.rendered_rows == 0 {
            return Ok(());
        }
        let mut stdout = io::stdout();
        write!(stdout, "{}", clear_rendered_rows(self.rendered_rows))?;
        stdout.flush()?;
        self.rendered_rows = 0;
        Ok(())
    }

    /// 生成并写入当前摘要。
    fn redraw(&mut self) -> Result<bool> {
        let rendered = render_live_command_output_for_cli(
            &self.stdout.display_text(),
            &self.stderr.display_text(),
        );
        if rendered.trim().is_empty() {
            return Ok(false);
        }
        let mut stdout = io::stdout();
        if self.rendered_rows > 0 {
            write!(stdout, "{}", clear_rendered_rows(self.rendered_rows))?;
        }
        let block = format!("{rendered}\n");
        write!(stdout, "{block}")?;
        stdout.flush()?;
        self.rendered_rows = rendered_visual_rows(&block);
        Ok(true)
    }

    #[cfg(test)]
    /// 返回当前 stdout 与 stderr 的可显示文本。
    ///
    /// 返回:
    /// - 两个命令输出缓冲的显示文本
    pub(super) fn display_texts(&self) -> (String, String) {
        (
            self.stdout.display_text().into_owned(),
            self.stderr.display_text().into_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证连续前台命令不会复用上一条命令输出。
    #[test]
    fn begin_resets_previous_command_buffers() {
        let mut preview = CliCommandPreview::new();
        preview.stdout.append(b"first", 0);
        preview.stderr.append(b"error", 0);

        preview.begin();

        assert_eq!(preview.display_texts(), (String::new(), String::new()));
    }
}

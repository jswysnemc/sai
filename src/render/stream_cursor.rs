use crate::render::stream::StreamRenderer;
use anyhow::Result;
use crossterm::cursor::{Hide, Show};
use crossterm::execute;
use std::io;

impl StreamRenderer {
    /// 隐藏终端光标。
    ///
    /// 返回:
    /// - 操作是否成功
    pub(crate) fn hide_cursor(&mut self) -> Result<()> {
        if !self.cursor_hidden {
            execute!(io::stdout(), Hide)?;
            self.cursor_hidden = true;
        }
        Ok(())
    }

    /// 显示终端光标。
    ///
    /// 返回:
    /// - 操作是否成功
    pub(crate) fn show_cursor(&mut self) -> Result<()> {
        if self.cursor_hidden {
            execute!(io::stdout(), Show)?;
            self.cursor_hidden = false;
        }
        Ok(())
    }
}

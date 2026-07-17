use crate::render::tool_call_preview::tool_call_status_text;
use anyhow::Result;
use crossterm::execute;
use crossterm::terminal::{Clear, ClearType};
use std::io::{self, Write};

pub(crate) struct LiveToolStatus {
    active: bool,
}

impl LiveToolStatus {
    /// 创建单行工具状态管理器。
    ///
    /// 返回:
    /// - 新的单行工具状态管理器
    pub(crate) fn new() -> Self {
        Self { active: false }
    }

    /// 判断当前是否存在活动状态行。
    ///
    /// 返回:
    /// - 是否存在活动状态行
    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    /// 写入或覆盖当前工具状态行。
    ///
    /// 参数:
    /// - `name`: 工具展示标签
    /// - `status`: 工具状态，取值为 arg、run、ok 或 err
    /// - `final_line`: 是否结束当前状态行
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn write(&mut self, name: &str, status: &str, final_line: bool) -> Result<()> {
        let text = tool_call_status_text(name, status);
        let mut stdout = io::stdout();
        execute!(stdout, Clear(ClearType::CurrentLine))?;
        write!(stdout, "\r\x1b[2m{text}\x1b[0m")?;
        if final_line {
            writeln!(stdout)?;
        }
        stdout.flush()?;
        self.active = !final_line;
        Ok(())
    }

    /// 结束当前工具状态行。
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn finish(&mut self) -> Result<()> {
        if self.active {
            let mut stdout = io::stdout();
            writeln!(stdout)?;
            stdout.flush()?;
            self.active = false;
        }
        Ok(())
    }

    /// 清除当前工具状态行。
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn clear(&mut self) -> Result<()> {
        if self.active {
            let mut stdout = io::stdout();
            execute!(stdout, Clear(ClearType::CurrentLine))?;
            write!(stdout, "\r")?;
            stdout.flush()?;
            self.active = false;
        }
        Ok(())
    }
}

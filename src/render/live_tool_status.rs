use crate::render::terminal_paint::paint_lock;
use crate::render::tool_event_line::{tool_call_status_text, tool_event_text};
use anyhow::Result;
use crossterm::queue;
use crossterm::terminal::{Clear, ClearType};
use std::io::{self, Write};

pub(crate) struct LiveToolStatus {
    active: bool,
    /// 是否已经把静态状态写进终端。只挂了底行扫光时不算。
    painted: bool,
}

impl LiveToolStatus {
    /// 创建单行工具状态管理器。
    ///
    /// 返回:
    /// - 新的单行工具状态管理器
    pub(crate) fn new() -> Self {
        Self {
            active: false,
            painted: false,
        }
    }

    /// 标记有一条进行中的工具状态，先不写终端。
    ///
    /// 终端里的流光由等待动画画出；这里只占住底行所有权。
    ///
    /// 返回:
    /// - 无
    pub(crate) fn arm(&mut self) {
        self.active = true;
        self.painted = false;
    }

    /// 结束底行占用，不额外换行。
    ///
    /// 返回:
    /// - 无
    pub(crate) fn disarm(&mut self) {
        self.active = false;
        self.painted = false;
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
    /// live 阶段整行弱化；定稿行改用与 transcript 一致的统一排版
    /// （状态色圆点 + 粗体动词），避免历史中残留一条弱化行。
    ///
    /// 参数:
    /// - `name`: 工具展示标签
    /// - `status`: 工具状态，取值为 arg、run、ok 或 err
    /// - `final_line`: 是否结束当前状态行
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn write(&mut self, name: &str, status: &str, final_line: bool) -> Result<()> {
        let _paint = paint_lock();
        let mut stdout = io::stdout();
        queue!(stdout, Clear(ClearType::CurrentLine))?;
        if final_line {
            writeln!(stdout, "\r{}", tool_event_text(name, status))?;
        } else {
            let text = tool_call_status_text(name, status);
            write!(stdout, "\r\x1b[2m{text}\x1b[0m")?;
        }
        stdout.flush()?;
        self.active = !final_line;
        self.painted = !final_line;
        Ok(())
    }

    /// 结束当前工具状态行。
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn finish(&mut self) -> Result<()> {
        if self.active && self.painted {
            let _paint = paint_lock();
            let mut stdout = io::stdout();
            writeln!(stdout)?;
            stdout.flush()?;
        }
        self.active = false;
        self.painted = false;
        Ok(())
    }

    /// 清除当前工具状态行。
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn clear(&mut self) -> Result<()> {
        if self.active {
            let _paint = paint_lock();
            let mut stdout = io::stdout();
            queue!(stdout, Clear(ClearType::CurrentLine))?;
            write!(stdout, "\r")?;
            stdout.flush()?;
            self.active = false;
        }
        self.painted = false;
        Ok(())
    }
}

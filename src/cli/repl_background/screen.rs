//! 任务面板终端守卫和增量帧输出。

use anyhow::Result;
use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{self, Clear, ClearType},
};
use std::io::{self, Write};

pub(super) struct TaskScreen {
    _mouse: crate::cli::mouse_capture::MouseCapture,
    was_raw: bool,
    entered: bool,
    stdout: io::Stdout,
    previous: Vec<String>,
    size: Option<(u16, u16)>,
}

impl TaskScreen {
    /// 【后台任务】【终端保护】进入管理屏幕，初始化失败时也恢复终端。
    /// 参数: 无；返回: 屏幕守卫。
    pub fn start() -> Result<Self> {
        let mut screen = Self {
            _mouse: crate::cli::mouse_capture::MouseCapture::acquire()?,
            was_raw: terminal::is_raw_mode_enabled()?,
            entered: false,
            stdout: io::stdout(),
            previous: Vec::new(),
            size: None,
        };
        terminal::enable_raw_mode()?;
        crate::cli::alternate_screen::enter_alternate_screen(&mut screen.stdout)?;
        screen.entered = true;
        execute!(screen.stdout, Hide)?;
        write!(
            screen.stdout,
            "{}",
            crate::render::terminal_image::kitty_delete_placements()
        )?;
        Ok(screen)
    }
    /// 【后台任务】【增量绘制】同步绘制发生变化的行，仅首次或尺寸变化时清屏。
    /// 参数: lines 为帧文本，size 为终端尺寸；返回: 输出结果。
    pub fn draw(&mut self, lines: &[String], size: (u16, u16)) -> Result<()> {
        if self.size == Some(size) && self.previous == lines {
            return Ok(());
        }
        crate::config_tui::ui::begin_synced_frame(&mut self.stdout)?;
        if self.size != Some(size) {
            execute!(self.stdout, Clear(ClearType::All))?;
            self.previous.clear();
        }
        crate::render::terminal_rows::paint_changed_rows(
            &mut self.stdout,
            0,
            usize::from(size.0),
            lines,
            Some(&self.previous),
        )?;
        crate::config_tui::ui::end_synced_frame(&mut self.stdout)?;
        self.previous = lines.to_vec();
        self.size = Some(size);
        Ok(())
    }
}
impl Drop for TaskScreen {
    /// 【后台任务】【终端恢复】恢复光标、鼠标和调用前的输入模式。
    /// 参数: 无；返回: 无。
    fn drop(&mut self) {
        let _ = execute!(self.stdout, Show);
        if self.entered {
            let _ = crate::cli::alternate_screen::leave_alternate_screen(&mut self.stdout);
        }
        if !self.was_raw {
            let _ = terminal::disable_raw_mode();
        }
    }
}

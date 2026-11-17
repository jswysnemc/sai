mod catalog;
mod model;
mod mouse;
mod rows;
#[cfg(test)]
mod tests;
mod view;
mod workspace_prompt;

use crate::{paths::SaiPaths, state::ResumeTarget};
use anyhow::Result;
use crossterm::{
    cursor::{Hide, Show},
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind,
        KeyModifiers,
    },
    execute, terminal,
};
use std::{io, path::Path, time::Duration};

/// 【会话选择】【终端入口】显示当前或全部工作区会话，返回完整目标；取消时不修改任何状态。
///
/// 参数:
/// - `paths`: 应用路径
/// - `directory`: 初始目录
/// - `all`: 初始范围是否为全部工作区
///
/// 返回:
/// - 选中的恢复目标或取消
pub(super) fn select(
    paths: &SaiPaths,
    directory: &Path,
    all: bool,
) -> Result<Option<ResumeTarget>> {
    let (mut catalog, mut picker) = catalog::Catalog::new(paths, directory, all)?;
    if all {
        catalog.load_all(&mut picker)?;
    }
    let _terminal = TerminalGuard::start()?;
    let mut stdout = io::stdout();
    let mut dirty = true;
    let mut mouse = mouse::Mouse::default();
    loop {
        dirty |= catalog.poll(&mut picker);
        if dirty {
            mouse.state = view::draw(&mut stdout, &mut picker, terminal::size()?)?;
        }
        dirty = false;
        if !event::poll(Duration::from_millis(150))? {
            continue;
        }
        match mouse.event(event::read()?, &mut picker) {
            Event::Mouse(_) => dirty = true,
            Event::Resize(_, _) => dirty = true,
            Event::Paste(text) => {
                // 1. 【会话选择】【粘贴过滤】过滤换行与控制字符写入搜索框，防止粘贴换行误触发恢复
                if picker.paste_query(&text) {
                    dirty = true;
                }
            }
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                dirty = true;
                match key.code {
                    KeyCode::Esc => return Ok(None),
                    KeyCode::Char('c' | 'd') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(None)
                    }
                    KeyCode::Enter => {
                        if let Some(target) = picker.target() {
                            if let Some(target) = workspace_prompt::locate(paths, target.clone())? {
                                return Ok(Some(target));
                            }
                        }
                    }
                    KeyCode::Tab | KeyCode::BackTab => {
                        picker.toggle();
                        if picker.all {
                            catalog.load_all(&mut picker)?;
                        }
                    }
                    KeyCode::Up => picker.selected = picker.selected.saturating_sub(1),
                    KeyCode::Down => {
                        picker.selected =
                            (picker.selected + 1).min(picker.visible.len().saturating_sub(1))
                    }
                    KeyCode::PageUp => picker.selected = picker.selected.saturating_sub(10),
                    KeyCode::PageDown => {
                        picker.selected =
                            (picker.selected + 10).min(picker.visible.len().saturating_sub(1))
                    }
                    KeyCode::Home => picker.selected = 0,
                    KeyCode::End => picker.selected = picker.visible.len().saturating_sub(1),
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        picker.query.clear();
                        picker.selected = 0;
                        picker.scroll = 0;
                        picker.filter();
                    }
                    KeyCode::Backspace => {
                        picker.query.pop();
                        picker.selected = 0;
                        picker.scroll = 0;
                        picker.filter();
                    }
                    KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        picker.query.push(ch);
                        picker.selected = 0;
                        picker.scroll = 0;
                        picker.filter();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

struct TerminalGuard {
    _mouse: crate::cli::mouse_capture::MouseCapture,
    was_raw: bool,
}

impl TerminalGuard {
    /// 【会话选择】【终端保护】启用独立备用屏、鼠标捕获与括号粘贴，失败时安全恢复终端。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 自动恢复守卫
    fn start() -> Result<Self> {
        let mouse = crate::cli::mouse_capture::MouseCapture::acquire()?;
        let was_raw = terminal::is_raw_mode_enabled()?;
        terminal::enable_raw_mode()?;
        let guard = Self {
            was_raw,
            _mouse: mouse,
        };
        crate::cli::alternate_screen::enter_alternate_screen(&mut io::stdout())?;
        execute!(io::stdout(), EnableBracketedPaste, Hide)?;
        Ok(guard)
    }
}

impl Drop for TerminalGuard {
    /// 【会话选择】【终端恢复】退出或错误时恢复括号粘贴、光标与备用屏嵌套状态。
    ///
    /// 参数:
    /// - `self`: 当前守卫
    ///
    /// 返回:
    /// - 无
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableBracketedPaste, Show);
        let _ = crate::cli::alternate_screen::leave_alternate_screen(&mut io::stdout());
        if !self.was_raw {
            let _ = terminal::disable_raw_mode();
        }
    }
}

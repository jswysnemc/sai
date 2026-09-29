mod model;
#[cfg(test)]
mod tests;
mod view;
mod workspace_prompt;

use crate::{paths::SaiPaths, state::ResumeTarget};
use anyhow::Result;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{io, path::Path, time::Duration};

/// 【会话选择】【终端入口】显示当前/全部工作区，返回完整目标；取消不修改任何状态。
/// 参数: paths 为应用路径，directory 为初始目录，all 为初始范围；返回: 目标或取消
pub(super) fn select(
    paths: &SaiPaths,
    directory: &Path,
    all: bool,
) -> Result<Option<ResumeTarget>> {
    let targets = crate::state::resume_catalog(paths, directory, true)?;
    let id = crate::state::workspace_id_for_path(&crate::platform::windows_path::canonicalize(
        directory,
    )?);
    let mut picker = model::Picker::new(targets, id, all);
    let _terminal = TerminalGuard::start()?;
    let mut stdout = io::stdout();
    let mut dirty = true;
    loop {
        if dirty {
            view::draw(&mut stdout, &picker, terminal::size()?)?;
        }
        dirty = false;
        if !event::poll(Duration::from_millis(150))? {
            continue;
        }
        match event::read()? {
            Event::Resize(_, _) => dirty = true,
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
                    KeyCode::Tab | KeyCode::BackTab => picker.toggle(),
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
                    KeyCode::Backspace => {
                        picker.query.pop();
                        picker.selected = 0;
                        picker.filter();
                    }
                    KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        picker.query.push(ch);
                        picker.selected = 0;
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
    was_raw: bool,
}
impl TerminalGuard {
    /// 【会话选择】【终端保护】启用独立屏幕，失败时恢复原终端状态。
    /// 参数: 无；返回: 自动恢复守卫
    fn start() -> Result<Self> {
        let was_raw = terminal::is_raw_mode_enabled()?;
        terminal::enable_raw_mode()?;
        let guard = Self { was_raw };
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(guard)
    }
}
impl Drop for TerminalGuard {
    /// 【会话选择】【终端保护】退出或错误时恢复屏幕与光标。
    /// 参数: 无；返回: 无
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
        if !self.was_raw {
            let _ = terminal::disable_raw_mode();
        }
    }
}

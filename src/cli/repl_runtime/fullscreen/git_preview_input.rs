//! 固定审查区的局部滚动与焦点分流。

use super::git_preview::{DiffMode, ReviewFocus};
use super::git_preview_view::PreviewLayout;
use super::input::Effect;
use crate::cli::repl_runtime::ReplRuntime;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};

impl ReplRuntime {
    /// 【终端审查】【区域输入】聊天、文件清单和差异各自接管所属区域的滚动，不全局捕获鼠标。
    /// 参数: event 为终端事件；返回: 处理效果，聊天事件为空。
    pub(super) fn git_preview_event(&mut self, event: &Event) -> Option<Effect> {
        let session = self.fullscreen.as_mut()?;
        let layout = session.state.layout?;
        let count = session.sidebar.git.summary.files.len();
        let geometry = PreviewLayout::compute(&layout, count);
        let preview = &mut session.sidebar.preview;
        preview.path.as_ref()?;
        match event {
            Event::Mouse(mouse) => {
                // 1. 【终端审查】【聊天捕获】从聊天开始的拖选跨入右栏仍归聊天，不切换选区来源
                if session.state.press.is_some()
                    && matches!(
                        mouse.kind,
                        MouseEventKind::Drag(MouseButton::Left)
                            | MouseEventKind::Up(MouseButton::Left)
                    )
                {
                    return None;
                }
                if !geometry.contains(mouse.column, mouse.row) {
                    preview.hovered = None;
                    if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                        preview.focus = None;
                        session.sidebar.focused = false;
                    }
                    return None;
                }
                if mouse.kind == MouseEventKind::Moved {
                    let target =
                        geometry.hover_at(mouse.column, mouse.row, preview.file_scroll, count);
                    let changed = preview.hovered != target;
                    preview.hovered = target;
                    return Some(if changed {
                        Effect::Repaint
                    } else {
                        Effect::Nothing
                    });
                }
                let cleared = preview.hovered.take().is_some();
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if let Some(mode) = geometry.mode_at(mouse.column, mouse.row) {
                            preview.focus = Some(ReviewFocus::Diff);
                            preview.set_mode(mode);
                            return Some(Effect::Repaint);
                        }
                        if geometry.in_files(mouse.row) {
                            let index =
                                preview.file_scroll + usize::from(mouse.row) - geometry.files_top;
                            self.open_review_file(index, ReviewFocus::Files);
                            return Some(Effect::Repaint);
                        }
                        if geometry.in_diff(mouse.row) {
                            preview.focus = Some(ReviewFocus::Diff);
                        }
                    }
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                        let delta = if mouse.kind == MouseEventKind::ScrollUp {
                            -3
                        } else {
                            3
                        };
                        let changed = if geometry.in_files(mouse.row) {
                            preview.scroll_files(delta, geometry.files_visible, count)
                        } else if geometry.in_diff(mouse.row) {
                            preview.scroll_by(delta, geometry.visible)
                        } else {
                            false
                        };
                        return Some(if changed || cleared {
                            Effect::Repaint
                        } else {
                            Effect::Nothing
                        });
                    }
                    _ => {}
                }
                Some(Effect::Repaint)
            }
            Event::Key(key) if key.kind == KeyEventKind::Release => {
                preview.focus.map(|_| Effect::Nothing)
            }
            Event::Key(key) => {
                preview.hovered = None;
                if key.code == KeyCode::F(6) {
                    preview.focus = match preview.focus {
                        None => Some(ReviewFocus::Files),
                        Some(ReviewFocus::Files) => Some(ReviewFocus::Diff),
                        Some(ReviewFocus::Diff) => None,
                    };
                    session.sidebar.focused = false;
                    return Some(Effect::Repaint);
                }
                let focus = preview.focus?;
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c' | 'o'))
                {
                    return Some(Effect::Pass);
                }
                if key.code == KeyCode::Esc {
                    preview.close();
                    return Some(Effect::Refresh);
                }
                if key.code == KeyCode::Tab {
                    preview.set_mode(preview.mode.next());
                    return Some(Effect::Repaint);
                }
                if let KeyCode::Char(value @ ('1' | '2' | '3')) = key.code {
                    preview.set_mode(match value {
                        '1' => DiffMode::Working,
                        '2' => DiffMode::Staged,
                        _ => DiffMode::Unstaged,
                    });
                    return Some(Effect::Repaint);
                }
                if focus == ReviewFocus::Files {
                    let current = session
                        .sidebar
                        .git
                        .summary
                        .files
                        .iter()
                        .position(|(_, path)| Some(path.as_str()) == preview.path.as_deref())
                        .unwrap_or(0);
                    let index = match key.code {
                        KeyCode::Up => current.saturating_sub(1),
                        KeyCode::Down => (current + 1).min(count.saturating_sub(1)),
                        KeyCode::Home => 0,
                        KeyCode::End => count.saturating_sub(1),
                        KeyCode::Enter => {
                            preview.focus = Some(ReviewFocus::Diff);
                            return Some(Effect::Repaint);
                        }
                        _ => return Some(Effect::Nothing),
                    };
                    self.open_review_file(index, ReviewFocus::Files);
                    return Some(Effect::Repaint);
                }
                let page = geometry.visible.saturating_sub(1).max(1) as isize;
                let changed = match key.code {
                    KeyCode::Up => preview.scroll_by(-1, geometry.visible),
                    KeyCode::Down => preview.scroll_by(1, geometry.visible),
                    KeyCode::PageUp => preview.scroll_by(-page, geometry.visible),
                    KeyCode::PageDown => preview.scroll_by(page, geometry.visible),
                    KeyCode::Home => preview.scroll_by(isize::MIN, geometry.visible),
                    KeyCode::End => preview.scroll_by(isize::MAX, geometry.visible),
                    KeyCode::Left => {
                        let before = preview.horizontal;
                        preview.horizontal = before.saturating_sub(4);
                        before != preview.horizontal
                    }
                    KeyCode::Right => {
                        let max = preview
                            .content
                            .as_ref()
                            .map_or(0, |content| {
                                content
                                    .lines
                                    .iter()
                                    .map(|line| crate::cli::repl_text::visible_width(&line.text))
                                    .max()
                                    .unwrap_or(0)
                            })
                            .saturating_sub(geometry.width.saturating_sub(10));
                        let before = preview.horizontal;
                        preview.horizontal = before.saturating_add(4).min(max);
                        before != preview.horizontal
                    }
                    KeyCode::Char('r') => {
                        let current = session
                            .sidebar
                            .git
                            .summary
                            .files
                            .iter()
                            .position(|(_, path)| Some(path.as_str()) == preview.path.as_deref());
                        if let Some(index) = current {
                            self.open_review_file(index, ReviewFocus::Diff);
                        }
                        return Some(Effect::Repaint);
                    }
                    _ => false,
                };
                Some(if changed {
                    Effect::Repaint
                } else {
                    Effect::Nothing
                })
            }
            Event::Paste(_) if preview.focus.is_some() => Some(Effect::Nothing),
            _ => None,
        }
    }

    /// 【终端审查】【切换文件】保持比较模式和文件列表位置，只重新读取目标文件差异。
    /// 参数: index 为文件序号，focus 为切换后焦点；返回: 无。
    fn open_review_file(&mut self, index: usize, focus: ReviewFocus) {
        let file = self
            .fullscreen
            .as_ref()
            .and_then(|session| session.sidebar.git.summary.files.get(index))
            .map(|(_, path)| path.clone());
        let Some(path) = file else {
            return;
        };
        let directory = self.sidebar_directory();
        let session = self.fullscreen.as_mut().unwrap();
        let count = session.sidebar.git.summary.files.len();
        let geometry = PreviewLayout::compute(&session.state.layout.unwrap(), count);
        let preview = &mut session.sidebar.preview;
        let mode = preview.mode;
        preview.open(directory, path);
        preview.set_mode(mode);
        preview.focus = Some(focus);
        if index < preview.file_scroll {
            preview.file_scroll = index;
        }
        if index >= preview.file_scroll + geometry.files_visible {
            preview.file_scroll = index + 1 - geometry.files_visible.max(1);
        }
        preview.scroll_files(0, geometry.files_visible, count);
    }
}

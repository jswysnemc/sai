use super::super::input::{read_event, ConfigEvent};
use super::super::layout::content_frame;
use super::super::mouse::{ClickTracker, HitRect, ListMouseState, MenuInput};
use super::super::theme::help_line;
use super::*;
use crate::i18n::text as t;
use crossterm::cursor::MoveTo;
use crossterm::event::{KeyCode, MouseEventKind};
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType};
use std::io;

/// 提示弹窗最大宽度：短提示不需要横跨整屏。
const MESSAGE_MAX_WIDTH: u16 = 88;

/// 未保存更改时的退出选择。
pub(crate) enum UnsavedExitChoice {
    Save,
    Discard,
    Cancel,
}

/// 破坏性删除操作的确认弹窗。
///
/// 默认停在「取消」：删除键往往与目标键相邻（如 s/d、a/d），
/// 误触后无法撤销，因此默认选项必须是安全项。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `title`: 顶栏标题
/// - `target`: 待删除对象名称
/// - `warning`: 副标题中的后果说明（可为空）
///
/// 返回:
/// - 是否确认删除
pub(crate) fn confirm_delete(
    stdout: &mut io::Stdout,
    title: &str,
    target: &str,
    warning: &str,
) -> Result<bool> {
    let mut selected = 1usize;
    let mut mouse = ListMouseState::default();
    loop {
        let options = [
            format!("{} {target}", t("Delete", "删除")),
            t("Cancel", "取消").to_string(),
        ];
        let hitbox = draw_menu_interactive(
            stdout,
            title,
            &options,
            &[],
            selected,
            &mouse,
            &help_line(&[
                ("y", t("confirm delete", "确认删除")),
                ("n/Esc", t("cancel", "取消")),
                ("↑↓", t("move", "移动")),
                ("Enter", t("select", "确定")),
            ]),
            warning,
        )?;
        let code = match hitbox.read(&mut mouse, selected, true)? {
            MenuInput::Redraw => continue,
            MenuInput::Select(index) => {
                selected = index;
                continue;
            }
            MenuInput::Activate(index) => return Ok(index == 0),
            MenuInput::RightClick(index) => {
                selected = index;
                continue;
            }
            MenuInput::Key(key) => key.code,
            MenuInput::Action(code) => code,
        };
        match code {
            KeyCode::Esc | KeyCode::Char('q' | 'n' | 'N') => return Ok(false),
            KeyCode::Char('y' | 'Y') => return Ok(true),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Enter => return Ok(selected == 0),
            _ => {}
        }
    }
}

/// 用配置界面的菜单询问如何处理未保存更改，支持单键直达与鼠标点击减少确认步数。
///
/// 参数:
/// - `stdout`: 终端输出
///
/// 返回:
/// - 保存、放弃或取消
pub(crate) fn confirm_unsaved_exit(stdout: &mut io::Stdout) -> Result<UnsavedExitChoice> {
    let mut selected = 0usize;
    let mut mouse = ListMouseState::default();
    loop {
        let options = [
            t("Save and exit  [y/s]", "保存并退出  [y/s]"),
            t("Discard changes  [d/n]", "放弃更改  [d/n]"),
            t("Cancel  [Esc]", "取消  [Esc]"),
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        let hitbox = draw_menu_interactive(
            stdout,
            t(" Unsaved changes ", " 未保存的更改 "),
            &options,
            &[],
            selected,
            &mouse,
            &help_line(&[
                ("y/s/Enter", t("save", "保存")),
                ("d/n", t("discard", "放弃")),
                ("Esc", t("stay", "返回")),
            ]),
            "",
        )?;
        let choice_for = |index: usize| match index {
            0 => UnsavedExitChoice::Save,
            1 => UnsavedExitChoice::Discard,
            _ => UnsavedExitChoice::Cancel,
        };
        let code = match hitbox.read(&mut mouse, selected, true)? {
            MenuInput::Redraw => continue,
            MenuInput::Select(index) => {
                selected = index;
                continue;
            }
            MenuInput::Activate(index) => return Ok(choice_for(index)),
            MenuInput::RightClick(index) => {
                selected = index;
                continue;
            }
            MenuInput::Key(key) => key.code,
            MenuInput::Action(code) => code,
        };
        match code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(UnsavedExitChoice::Cancel),
            KeyCode::Char('y' | 'Y' | 's' | 'S') => return Ok(UnsavedExitChoice::Save),
            KeyCode::Char('d' | 'D' | 'n' | 'N') => return Ok(UnsavedExitChoice::Discard),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Enter => return Ok(choice_for(selected)),
            _ => {}
        }
    }
}

pub(crate) fn message(stdout: &mut io::Stdout, text: &str) -> Result<()> {
    let mut offset = 0usize;
    let mut clicks = ClickTracker::default();
    loop {
        let (cols, rows) = terminal::size()?;
        // 1. 按当前终端尺寸折行并限制偏移，保证尺寸调整后不越界
        let probe = content_frame(cols, rows, 1, MESSAGE_MAX_WIDTH);
        let lines = wrap_text(text, probe.width.saturating_sub(4) as usize);
        let content_rows = lines.len().saturating_add(2).min(usize::from(u16::MAX)) as u16;
        let frame = content_frame(cols, rows, content_rows.max(3), MESSAGE_MAX_WIDTH);
        let page = frame.height.saturating_sub(4) as usize;
        let max_offset = lines.len().saturating_sub(page);
        offset = offset.min(max_offset);

        begin_synced_frame(stdout)?;
        queue!(stdout, Clear(ClearType::All))?;
        draw_box(
            stdout,
            frame.x,
            frame.y,
            frame.width,
            frame.height,
            t(" Notice ", " 提示 "),
        )?;
        for (row, line) in lines.iter().skip(offset).take(page).enumerate() {
            queue!(
                stdout,
                MoveTo(
                    frame.x.saturating_add(2),
                    frame.y.saturating_add(2).saturating_add(row as u16)
                ),
                Print(crate::render::readable_terminal_text(
                    &(line.clone()).to_string()
                ))
            )?;
        }
        let status = if max_offset == 0 {
            help_line(&[(t("click/key", "点击/任意键"), t("continue", "继续"))])
        } else {
            // 滚动时把位置一并显示，否则用户不知道下面还有多少
            format!(
                "{}  {}/{}",
                help_line(&[
                    ("↑↓", t("scroll", "滚动")),
                    (t("click/key", "点击/任意键"), t("continue", "继续")),
                ]),
                offset.saturating_add(page).min(lines.len()),
                lines.len()
            )
        };
        draw_status_bar(stdout, &frame, &status)?;
        end_synced_frame(stdout)?;
        let hit_frame = HitRect::from(frame);
        match read_event()? {
            ConfigEvent::Resize => {}
            ConfigEvent::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => offset = offset.saturating_sub(1),
                MouseEventKind::ScrollDown => offset = (offset + 1).min(max_offset),
                _ => {
                    let target = hit_frame.contains(mouse.column, mouse.row).then_some(true);
                    if clicks.update(mouse, target).is_some() {
                        return Ok(());
                    }
                }
            },
            ConfigEvent::Key(key) => {
                let next = match key.code {
                    KeyCode::Up | KeyCode::Char('k') => offset.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => (offset + 1).min(max_offset),
                    KeyCode::PageUp => offset.saturating_sub(page),
                    KeyCode::PageDown | KeyCode::Char(' ') => (offset + page).min(max_offset),
                    KeyCode::Home => 0,
                    KeyCode::End => max_offset,
                    _ => return Ok(()),
                };
                // 2. 已经到头/到尾时按方向键视为「任意键」直接关闭，避免空按一下没反应
                if next == offset && max_offset > 0 {
                    return Ok(());
                }
                offset = next;
            }
        }
    }
}

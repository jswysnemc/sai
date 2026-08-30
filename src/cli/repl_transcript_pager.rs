use super::repl_runtime::ReplRuntime;
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use std::io::{self, Write};
use std::time::Duration;

/// 在备用屏中浏览完整 transcript，滚动进度由程序自管理。
///
/// 与主屏 scrollback 不同：resize 重排后按「距底部行数」锚定恢复位置，
/// 回看进度不会因重放而丢失。
///
/// 参数:
/// - `runtime`: 当前 REPL 运行期（用于按宽度渲染 transcript）
///
/// 返回:
/// - 浏览是否成功结束
pub(super) fn open_transcript_pager(runtime: &mut ReplRuntime) -> Result<()> {
    let was_raw = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
    if !was_raw {
        crossterm::terminal::enable_raw_mode()?;
    }
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        Hide,
        event::EnableMouseCapture
    )?;
    let result = run_pager_loop(runtime);
    execute!(
        io::stdout(),
        event::DisableMouseCapture,
        LeaveAlternateScreen,
        Show
    )?;
    if !was_raw {
        crossterm::terminal::disable_raw_mode()?;
    }
    result
}

/// 浏览主循环：渲染、滚动与 resize 锚定。
///
/// 参数:
/// - `runtime`: 当前 REPL 运行期
///
/// 返回:
/// - 循环是否成功结束
fn run_pager_loop(runtime: &mut ReplRuntime) -> Result<()> {
    let (mut cols, mut rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let mut lines = runtime.transcript_pager_lines(usize::from(cols));
    // 进度锚点：视口底部距内容末尾的行数；初始停在底部（最新内容）
    let mut offset_from_bottom = 0usize;
    // 上一次实际绘制的画面指纹：poll 只是唤醒手段，没有变化时不该重绘。
    // 原先每次 250ms 超时都整屏 Clear + 逐行重打，空闲时表现为持续微闪，
    // 在 SSH / 远程会话上还要白白吃掉带宽
    let mut drawn: Option<(u16, u16, usize, usize)> = None;
    loop {
        let view_height = usize::from(rows.max(2)) - 1;
        let max_offset = lines.len().saturating_sub(view_height);
        offset_from_bottom = offset_from_bottom.min(max_offset);
        let frame = (cols, rows, offset_from_bottom, lines.len());
        if drawn != Some(frame) {
            draw_view(&lines, view_height, cols, offset_from_bottom)?;
            drawn = Some(frame);
        }
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => return Ok(()),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(())
                }
                KeyCode::Down if is_jump_to_output_bottom_key(key.code, key.modifiers) => {
                    offset_from_bottom = 0;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    offset_from_bottom = offset_from_bottom.saturating_add(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    offset_from_bottom = offset_from_bottom.saturating_sub(1);
                }
                KeyCode::PageUp => {
                    offset_from_bottom = offset_from_bottom.saturating_add(view_height);
                }
                KeyCode::PageDown => {
                    offset_from_bottom = offset_from_bottom.saturating_sub(view_height);
                }
                KeyCode::Home => offset_from_bottom = usize::MAX,
                KeyCode::End => offset_from_bottom = 0,
                _ => {}
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => {
                    offset_from_bottom = offset_from_bottom.saturating_add(3);
                }
                MouseEventKind::ScrollDown => {
                    offset_from_bottom = offset_from_bottom.saturating_sub(3);
                }
                _ => {}
            },
            Event::Resize(next_cols, next_rows) => {
                // resize 锚定：按新宽度重新渲染，保持距底部行数不变
                cols = next_cols.max(1);
                rows = next_rows.max(2);
                lines = runtime.transcript_pager_lines(usize::from(cols));
            }
            _ => {}
        }
    }
}

/// 判断按键是否为回到输出底部快捷键。
///
/// 参数:
/// - `code`: 终端键码
/// - `modifiers`: 修饰键集合
///
/// 返回:
/// - `Ctrl+↓` 时返回 `true`
pub(super) fn is_jump_to_output_bottom_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    code == KeyCode::Down && modifiers.contains(KeyModifiers::CONTROL)
}

/// 绘制一帧视口内容与底部状态行。
///
/// 参数:
/// - `lines`: 已按当前宽度预折行的 transcript 行
/// - `view_height`: 视口行数（不含状态行）
/// - `cols`: 终端列数
/// - `offset_from_bottom`: 距底部行数
///
/// 返回:
/// - 绘制是否成功
fn draw_view(
    lines: &[String],
    view_height: usize,
    cols: u16,
    offset_from_bottom: usize,
) -> Result<()> {
    let mut stdout = io::stdout();
    let end = lines.len().saturating_sub(offset_from_bottom);
    let start = end.saturating_sub(view_height);
    for row in 0..view_height {
        queue!(
            stdout,
            MoveTo(0, row.min(usize::from(u16::MAX)) as u16),
            Clear(ClearType::CurrentLine)
        )?;
        if let Some(line) = lines.get(start + row).filter(|_| start + row < end) {
            queue!(stdout, Print(line))?;
        }
    }
    // 底部状态行：位置 + 键位提示
    let position = if lines.len() <= view_height {
        t("all", "全部").to_string()
    } else {
        format!("{}%", (end.min(lines.len()) * 100) / lines.len().max(1))
    };
    let status = format!(
        "\x1b[7m {} {} · ↑↓/PgUp/PgDn {} · Ctrl+↓/End {} · Esc {} \x1b[0m",
        t("transcript", "会话记录"),
        position,
        t("scroll", "滚动"),
        t("bottom", "回到底部"),
        t("close", "关闭")
    );
    queue!(
        stdout,
        MoveTo(0, view_height.min(usize::from(u16::MAX)) as u16),
        Clear(ClearType::CurrentLine),
        Print(clip_to_width(&status, usize::from(cols)))
    )?;
    stdout.flush()?;
    Ok(())
}

/// 将单行 ANSI 文本截断到终端宽度。
///
/// 参数:
/// - `line`: 原始行
/// - `cols`: 终端列数
///
/// 返回:
/// - 不超宽的行
fn clip_to_width(line: &str, cols: usize) -> String {
    let mut out = String::new();
    let mut width = 0usize;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            out.push(ch);
            if chars.peek() == Some(&'[') {
                for next in chars.by_ref() {
                    out.push(next);
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > cols {
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out.push_str("\x1b[0m");
    out
}

#[cfg(test)]
mod tests {
    use super::{clip_to_width, is_jump_to_output_bottom_key};
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn clip_preserves_ansi_and_limits_width() {
        let clipped = clip_to_width("\x1b[7m 会话记录 100% \x1b[0m", 8);
        assert!(clipped.starts_with("\x1b[7m"));
        assert!(clipped.ends_with("\x1b[0m"));
        // 去掉 ANSI 后显示宽度不超过 8
        let mut width = 0usize;
        let mut chars = clipped.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\u{1b}' {
                if chars.peek() == Some(&'[') {
                    for next in chars.by_ref() {
                        if next.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                continue;
            }
            width += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        }
        assert!(width <= 8, "width={width}");
    }

    /// 只有 Ctrl+下方向键触发回到底部，普通下方向键仍用于逐行滚动。
    #[test]
    fn recognizes_jump_to_bottom_shortcut() {
        assert!(is_jump_to_output_bottom_key(
            KeyCode::Down,
            KeyModifiers::CONTROL
        ));
        assert!(!is_jump_to_output_bottom_key(
            KeyCode::Down,
            KeyModifiers::NONE
        ));
        assert!(!is_jump_to_output_bottom_key(
            KeyCode::Up,
            KeyModifiers::CONTROL
        ));
    }
}

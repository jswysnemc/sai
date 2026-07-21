use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use std::io::{self, Write};

/// 在备用屏幕中分页展示完整文本（学习 codex StaticOverlay 的交互模型）。
///
/// 参数:
/// - `title`: 顶部标题
/// - `body`: 完整正文（可含换行）
///
/// 返回:
/// - 是否成功
pub(super) fn open_text_pager(title: &str, body: &str) -> Result<()> {
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, Hide)?;
    let result = (|| -> Result<()> {
        let lines: Vec<&str> = if body.is_empty() {
            vec![""]
        } else {
            body.lines().collect()
        };
        let mut scroll: usize = 0;
        loop {
            let (cols, rows) = terminal::size().unwrap_or((80, 24));
            let rows = rows.max(3) as usize;
            let cols = cols.max(20) as usize;
            let header_rows = 1usize;
            let footer_rows = 1usize;
            let view_h = rows.saturating_sub(header_rows + footer_rows).max(1);
            let max_scroll = lines.len().saturating_sub(view_h);
            if scroll > max_scroll {
                scroll = max_scroll;
            }
            queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
            // 1. 标题
            let header = truncate_visible(title, cols);
            write!(stdout, "\x1b[1m{header}\x1b[0m\r\n")?;
            // 2. 正文窗口
            for row in 0..view_h {
                let idx = scroll + row;
                let line = lines.get(idx).copied().unwrap_or("");
                let clipped = truncate_visible(line, cols);
                write!(stdout, "{clipped}\r\n")?;
            }
            // 3. 底栏：进度 + 快捷键
            let end = (scroll + view_h).min(lines.len()).max(scroll);
            let pct = if lines.is_empty() {
                100
            } else {
                ((end as f64 / lines.len() as f64) * 100.0).round() as u16
            };
            let footer = format!(
                "{}  {}  {}%",
                t("Esc/q close · ↑↓/PgUp/PgDn scroll", "Esc/q 关闭 · ↑↓/PgUp/PgDn 滚动"),
                t("lines", "行"),
                pct
            );
            write!(stdout, "\x1b[2m{}\x1b[0m", truncate_visible(&footer, cols))?;
            stdout.flush()?;

            match event::read()? {
                Event::Key(KeyEvent {
                    code, modifiers, ..
                }) => match code {
                    KeyCode::Esc | KeyCode::Char('q') => break,
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Up | KeyCode::Char('k') => scroll = scroll.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => {
                        scroll = (scroll + 1).min(max_scroll);
                    }
                    KeyCode::PageUp => scroll = scroll.saturating_sub(view_h),
                    KeyCode::PageDown | KeyCode::Char(' ') => {
                        scroll = (scroll + view_h).min(max_scroll);
                    }
                    KeyCode::Home => scroll = 0,
                    KeyCode::End => scroll = max_scroll,
                    _ => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
        Ok(())
    })();
    let _ = execute!(stdout, Show, LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
    result
}

/// 按可见宽度截断文本。
///
/// 参数:
/// - `value`: 原文
/// - `width`: 最大列宽
///
/// 返回:
/// - 截断文本
fn truncate_visible(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0usize;
    for ch in value.chars() {
        if ch == '\r' {
            continue;
        }
        let w = if (ch as u32) >= 0x2e80 { 2 } else { 1 };
        if used + w > width {
            break;
        }
        out.push(ch);
        used += w;
    }
    out
}

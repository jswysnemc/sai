use crate::i18n::text as t;
use crate::render::fold_text::wrap_display_lines;
use crate::render::transcript::ExpandableBlock;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use std::io::{self, Write};
use unicode_width::UnicodeWidthChar;

/// 在备用屏幕中分页展示可展开块列表，支持左右切换。
///
/// 参数:
/// - `blocks`: 全部可展开块（时间序）
/// - `start_index`: 初始展示的块下标
///
/// 返回:
/// - 是否成功
pub(super) fn open_blocks_pager(blocks: &[ExpandableBlock], start_index: usize) -> Result<()> {
    if blocks.is_empty() {
        return Ok(());
    }
    let mut stdout = io::stdout();
    // 1. 记录进入前的 raw 状态；pager 结束后恢复，避免打断 TUI 输入
    let was_raw = terminal::is_raw_mode_enabled().unwrap_or(false);
    if !was_raw {
        terminal::enable_raw_mode()?;
    }
    execute!(
        stdout,
        EnterAlternateScreen,
        Hide,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES),
        EnableBracketedPaste
    )?;
    let result = (|| -> Result<()> {
        let mut index = start_index.min(blocks.len() - 1);
        let mut scroll: usize = 0;
        loop {
            let (cols, rows) = terminal::size().unwrap_or((80, 24));
            let rows = rows.max(3) as usize;
            let cols = cols.max(20) as usize;
            let header_rows = 1usize;
            let footer_rows = 1usize;
            let view_h = rows.saturating_sub(header_rows + footer_rows).max(1);
            // 2. 按当前终端宽度折行，保证渲染与折叠口径一致
            let block = &blocks[index];
            let lines = wrap_display_lines(&block.body, cols);
            let max_scroll = lines.len().saturating_sub(view_h);
            if scroll > max_scroll {
                scroll = max_scroll;
            }

            queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
            // 3. 标题：块序号 + 标题
            let header = format!(
                "[{}/{}] {}",
                index + 1,
                blocks.len(),
                block.title
            );
            write!(stdout, "\x1b[1m{}\x1b[0m\r\n", truncate_visible(&header, cols))?;
            // 4. 正文窗口
            for row in 0..view_h {
                let idx = scroll + row;
                let line = lines.get(idx).map(String::as_str).unwrap_or("");
                write!(stdout, "{}\r\n", truncate_visible(line, cols))?;
            }
            // 5. 底栏
            let end = (scroll + view_h).min(lines.len()).max(scroll);
            let pct = if lines.is_empty() {
                100
            } else {
                ((end as f64 / lines.len() as f64) * 100.0).round() as u16
            };
            let footer = if blocks.len() > 1 {
                format!(
                    "{}  {}%  {}/{}",
                    t(
                        "←→ blocks · ↑↓/PgUp/PgDn scroll · Esc close",
                        "←→ 切换块 · ↑↓/PgUp/PgDn 滚动 · Esc 关闭",
                    ),
                    pct,
                    index + 1,
                    blocks.len()
                )
            } else {
                format!(
                    "{}  {}%",
                    t("↑↓/PgUp/PgDn scroll · Esc close", "↑↓/PgUp/PgDn 滚动 · Esc 关闭"),
                    pct
                )
            };
            write!(stdout, "\x1b[2m{}\x1b[0m", truncate_visible(&footer, cols))?;
            stdout.flush()?;

            match event::read()? {
                Event::Key(KeyEvent {
                    code,
                    modifiers,
                    kind,
                    ..
                }) if kind != KeyEventKind::Release => match code {
                    KeyCode::Esc | KeyCode::Char('q') => break,
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Left | KeyCode::Char('h') if blocks.len() > 1 => {
                        index = if index == 0 {
                            blocks.len() - 1
                        } else {
                            index - 1
                        };
                        scroll = 0;
                    }
                    KeyCode::Right | KeyCode::Char('l') if blocks.len() > 1 => {
                        index = (index + 1) % blocks.len();
                        scroll = 0;
                    }
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
    // 6. 离开备用屏并恢复进入前的终端输入状态
    let _ = execute!(
        stdout,
        DisableBracketedPaste,
        PopKeyboardEnhancementFlags,
        Show,
        LeaveAlternateScreen
    );
    if was_raw {
        let _ = terminal::enable_raw_mode();
        let _ = execute!(
            stdout,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES),
            EnableBracketedPaste
        );
    } else {
        let _ = terminal::disable_raw_mode();
    }
    let _ = stdout.flush();
    result
}

/// 兼容单文本打开（单块）。
///
/// 参数:
/// - `title`: 标题
/// - `body`: 正文
///
/// 返回:
/// - 是否成功
#[allow(dead_code)]
pub(super) fn open_text_pager(title: &str, body: &str) -> Result<()> {
    let block = ExpandableBlock {
        title: title.to_string(),
        body: body.to_string(),
    };
    open_blocks_pager(&[block], 0)
}

/// 按可见显示宽度截断文本。
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
        let w = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
        if used + w > width {
            break;
        }
        out.push(ch);
        used += w;
    }
    out
}

use crate::cli::keyboard_enhancement::KeyboardEnhancementState;
use crate::i18n::text as t;
use crate::render::render_expandable_body;
use crate::render::transcript::{AnsiLine, ExpandableBlock, ExpandableBlockKind};
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use std::io::{self, Write};
use unicode_width::UnicodeWidthChar;

/// 在备用屏幕中分页展示可展开块列表，支持左右切换与可拖动进度条。
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
    if was_raw {
        execute!(stdout, EnterAlternateScreen, Hide, EnableMouseCapture)?;
    } else {
        execute!(
            stdout,
            EnterAlternateScreen,
            Hide,
            EnableBracketedPaste,
            EnableMouseCapture
        )?;
    }
    let mut keyboard_enhancement = if was_raw {
        KeyboardEnhancementState::default()
    } else {
        KeyboardEnhancementState::enable(&mut stdout)
    };
    let result = (|| -> Result<()> {
        let mut index = start_index.min(blocks.len() - 1);
        let mut scroll: usize = 0;
        // 拖动目标：横向底栏进度条 / 右侧竖向滚动条
        let mut drag_target = ScrollDragTarget::None;
        loop {
            let (cols, rows) = terminal::size().unwrap_or((80, 24));
            let rows = rows.max(4) as usize;
            let cols = cols.max(20) as usize;
            // 2. 固定顶栏仅序号；命令标题进入可滚动区，不钉在顶部
            let block = &blocks[index];
            let rendered_body = render_expandable_body(block.kind, &block.body);
            let header_prefix = format!("[{}/{}]", index + 1, blocks.len());
            let header_rows = 1usize; // 仅块序号
            let footer_rows = 2usize; // 进度条 + 快捷键
            let view_h = rows
                .saturating_sub(header_rows + footer_rows)
                .max(1);
            // 3. 滚动窗口内容 = 完整命令标题 + 空行 + 正文
            let body_width = cols.saturating_sub(1).max(1);
            let mut content_lines = AnsiLine::wrap_block(&block.title, body_width);
            if !block.title.trim().is_empty() && !rendered_body.trim().is_empty() {
                content_lines.push(AnsiLine::new(String::new()));
            }
            content_lines.extend(AnsiLine::wrap_block(&rendered_body, body_width));
            let max_scroll = content_lines.len().saturating_sub(view_h);
            if scroll > max_scroll {
                scroll = max_scroll;
            }

            queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
            // 4. 固定顶栏：仅块序号
            write!(
                stdout,
                "\x1b[1m{}\x1b[0m\r\n",
                truncate_visible(&header_prefix, cols)
            )?;
            let scrollbar = scrollbar_glyphs(view_h, content_lines.len(), scroll);
            for row in 0..view_h {
                let idx = scroll + row;
                let line = content_lines.get(idx).map(AnsiLine::as_str).unwrap_or("");
                let bar = scrollbar.get(row).copied().unwrap_or(' ');
                write!(stdout, "{}\x1b[2m{}\x1b[0m\r\n", pad_line(line, body_width), bar)?;
            }
            // 7. 可拖动进度条（第二底栏上方的横向轨道）
            let end = (scroll + view_h).min(content_lines.len()).max(scroll);
            let pct = if content_lines.is_empty() {
                100
            } else {
                ((end as f64 / content_lines.len() as f64) * 100.0).round() as u16
            };
            let track = horizontal_progress_track(cols, content_lines.len(), view_h, scroll);
            write!(stdout, "{track}\r\n")?;
            // 8. 底栏快捷键
            let footer = if blocks.len() > 1 {
                format!(
                    "{}  {}%  {}/{}",
                    t(
                        "←→ blocks · ↑↓/PgUp/PgDn/mouse scroll · drag bar · Esc close",
                        "←→ 切换块 · ↑↓/PgUp/PgDn/鼠标滚动 · 拖动进度条 · Esc 关闭",
                    ),
                    pct,
                    index + 1,
                    blocks.len()
                )
            } else {
                format!(
                    "{}  {}%",
                    t(
                        "↑↓/PgUp/PgDn/mouse scroll · drag bar · Esc close",
                        "↑↓/PgUp/PgDn/鼠标滚动 · 拖动进度条 · Esc 关闭"
                    ),
                    pct
                )
            };
            write!(stdout, "\x1b[2m{}\x1b[0m", truncate_visible(&footer, cols))?;
            stdout.flush()?;

            let progress_row = (header_rows + view_h) as u16;
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
                        drag_target = ScrollDragTarget::None;
                    }
                    KeyCode::Right | KeyCode::Char('l') if blocks.len() > 1 => {
                        index = (index + 1) % blocks.len();
                        scroll = 0;
                        drag_target = ScrollDragTarget::None;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        scroll = scroll.saturating_sub(1);
                        drag_target = ScrollDragTarget::None;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        scroll = (scroll + 1).min(max_scroll);
                        drag_target = ScrollDragTarget::None;
                    }
                    KeyCode::PageUp => {
                        scroll = scroll.saturating_sub(view_h);
                        drag_target = ScrollDragTarget::None;
                    }
                    KeyCode::PageDown | KeyCode::Char(' ') => {
                        scroll = (scroll + view_h).min(max_scroll);
                        drag_target = ScrollDragTarget::None;
                    }
                    KeyCode::Home => {
                        scroll = 0;
                        drag_target = ScrollDragTarget::None;
                    }
                    KeyCode::End => {
                        scroll = max_scroll;
                        drag_target = ScrollDragTarget::None;
                    }
                    _ => {}
                },
                Event::Mouse(mouse) => {
                    apply_mouse(
                        mouse,
                        cols,
                        view_h,
                        content_lines.len(),
                        max_scroll,
                        progress_row,
                        header_rows as u16,
                        &mut scroll,
                        &mut drag_target,
                    );
                }
                Event::Resize(_, _) => {
                    drag_target = ScrollDragTarget::None;
                }
                _ => {}
            }
        }
        Ok(())
    })();
    // 9. 离开备用屏并恢复进入前的终端输入状态
    let _ = execute!(stdout, DisableMouseCapture);
    if was_raw {
        let _ = execute!(stdout, Show, LeaveAlternateScreen);
    } else {
        let _ = execute!(stdout, DisableBracketedPaste);
        keyboard_enhancement.disable(&mut stdout);
        let _ = execute!(stdout, Show, LeaveAlternateScreen);
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
        kind: ExpandableBlockKind::Plain,
    };
    open_blocks_pager(&[block], 0)
}

/// 处理鼠标滚轮与进度条拖动。
///
/// 参数:
/// - `mouse`: 鼠标事件
/// - `cols`: 终端列数
/// - `view_h`: 可视行数
/// - `total_lines`: 正文总行数
/// - `max_scroll`: 最大滚动偏移
/// - `progress_row`: 横向进度条所在行
/// - `body_top_row`: 正文首行所在行
/// - `scroll`: 当前滚动偏移（可写）
/// 滚动条拖动目标。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScrollDragTarget {
    None,
    /// 底部横向进度条
    Horizontal,
    /// 右侧竖向滚动条
    Vertical,
}

/// 处理鼠标事件（滚轮、点击/拖动进度条）。
///
/// 参数:
/// - `mouse`: 鼠标事件
/// - `cols`: 终端列数
/// - `view_h`: 可视行数
/// - `total_lines`: 正文总行数
/// - `max_scroll`: 最大滚动偏移
/// - `progress_row`: 横向进度条所在行
/// - `body_top_row`: 正文首行所在行
/// - `scroll`: 当前滚动偏移（可写）
/// - `drag_target`: 当前拖动目标（可写）
fn apply_mouse(
    mouse: MouseEvent,
    cols: usize,
    view_h: usize,
    total_lines: usize,
    max_scroll: usize,
    progress_row: u16,
    body_top_row: u16,
    scroll: &mut usize,
    drag_target: &mut ScrollDragTarget,
) {
    let body_bottom = body_top_row.saturating_add(view_h as u16);
    let on_horizontal = mouse.row == progress_row
        || mouse.row.saturating_add(1) == progress_row
        || progress_row.saturating_add(1) == mouse.row;
    let on_vertical = mouse.row >= body_top_row
        && mouse.row < body_bottom
        && cols > 0
        && mouse.column as usize + 1 >= cols;
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            *scroll = scroll.saturating_sub(3);
            *drag_target = ScrollDragTarget::None;
        }
        MouseEventKind::ScrollDown => {
            *scroll = (*scroll + 3).min(max_scroll);
            *drag_target = ScrollDragTarget::None;
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if on_horizontal {
                *drag_target = ScrollDragTarget::Horizontal;
                *scroll = scroll_from_track_x(mouse.column as usize, cols, total_lines, view_h);
            } else if on_vertical {
                *drag_target = ScrollDragTarget::Vertical;
                let local = (mouse.row.saturating_sub(body_top_row)) as usize;
                *scroll = scroll_from_vertical_thumb(local, view_h, total_lines, max_scroll);
            } else {
                *drag_target = ScrollDragTarget::None;
            }
        }
        // 部分终端把按住拖动报告为 Moved 而非 Drag
        MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Moved => {
            if *drag_target == ScrollDragTarget::None {
                return;
            }
            // 拖动中不依赖当前行是否仍在轨道上，避免松脱
            match *drag_target {
                ScrollDragTarget::Horizontal => {
                    *scroll = scroll_from_track_x(mouse.column as usize, cols, total_lines, view_h);
                }
                ScrollDragTarget::Vertical => {
                    let local = if mouse.row < body_top_row {
                        0
                    } else {
                        (mouse.row.saturating_sub(body_top_row) as usize)
                            .min(view_h.saturating_sub(1))
                    };
                    *scroll = scroll_from_vertical_thumb(local, view_h, total_lines, max_scroll);
                }
                ScrollDragTarget::None => {}
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            *drag_target = ScrollDragTarget::None;
        }
        _ => {}
    }
}

/// 由横向进度条点击位置换算 scroll。
///
/// 参数:
/// - `x`: 列位置
/// - `cols`: 总列数
/// - `total_lines`: 总行数
/// - `view_h`: 可视高度
///
/// 返回:
/// - 滚动偏移
fn scroll_from_track_x(x: usize, cols: usize, total_lines: usize, view_h: usize) -> usize {
    let max_scroll = total_lines.saturating_sub(view_h);
    if max_scroll == 0 || cols == 0 {
        return 0;
    }
    let thumb_w = ((cols * view_h) / total_lines.max(1)).clamp(1, cols);
    let travel = cols.saturating_sub(thumb_w).max(1);
    // 点击位置映射到滑块中心所在行程
    let x = x.min(cols.saturating_sub(1));
    let center = x.saturating_sub(thumb_w / 2).min(travel);
    ((center as f64 / travel as f64) * max_scroll as f64).round() as usize
}

/// 由竖向滚动条位置换算 scroll。
///
/// 参数:
/// - `local_row`: 正文区内的相对行
/// - `view_h`: 可视高度
/// - `total_lines`: 总行数
/// - `max_scroll`: 最大滚动
///
/// 返回:
/// - 滚动偏移
fn scroll_from_vertical_thumb(
    local_row: usize,
    view_h: usize,
    total_lines: usize,
    max_scroll: usize,
) -> usize {
    if max_scroll == 0 || view_h == 0 {
        return 0;
    }
    let thumb_h = vertical_thumb_height(view_h, total_lines);
    let travel = view_h.saturating_sub(thumb_h).max(1);
    let center = local_row.saturating_sub(thumb_h / 2).min(travel);
    ((center as f64 / travel as f64) * max_scroll as f64).round() as usize
}

/// 计算竖向滑块高度。
///
/// 参数:
/// - `view_h`: 可视高度
/// - `total_lines`: 总行数
///
/// 返回:
/// - 滑块占用行数
fn vertical_thumb_height(view_h: usize, total_lines: usize) -> usize {
    if total_lines == 0 || total_lines <= view_h {
        return view_h.max(1);
    }
    ((view_h * view_h) / total_lines).clamp(1, view_h)
}

/// 生成右侧竖向滚动条字符。
///
/// 参数:
/// - `view_h`: 可视高度
/// - `total_lines`: 总行数
/// - `scroll`: 当前偏移
///
/// 返回:
/// - 每行一个字符
fn scrollbar_glyphs(view_h: usize, total_lines: usize, scroll: usize) -> Vec<char> {
    let mut glyphs = vec!['│'; view_h];
    if view_h == 0 {
        return glyphs;
    }
    if total_lines <= view_h {
        glyphs.fill('█');
        return glyphs;
    }
    let thumb_h = vertical_thumb_height(view_h, total_lines);
    let max_scroll = total_lines.saturating_sub(view_h);
    let travel = view_h.saturating_sub(thumb_h);
    let thumb_top = if max_scroll == 0 {
        0
    } else {
        (scroll * travel) / max_scroll
    };
    for row in thumb_top..thumb_top.saturating_add(thumb_h).min(view_h) {
        glyphs[row] = '█';
    }
    glyphs
}

/// 渲染横向可拖动进度条。
///
/// 参数:
/// - `cols`: 列数
/// - `total_lines`: 总行数
/// - `view_h`: 可视高度
/// - `scroll`: 当前偏移
///
/// 返回:
/// - ANSI 进度条文本
fn horizontal_progress_track(
    cols: usize,
    total_lines: usize,
    view_h: usize,
    scroll: usize,
) -> String {
    if cols == 0 {
        return String::new();
    }
    let max_scroll = total_lines.saturating_sub(view_h);
    let mut track = vec!['─'; cols];
    if max_scroll == 0 {
        track.fill('━');
    } else {
        let thumb_w = ((cols * view_h) / total_lines.max(1)).clamp(1, cols);
        let travel = cols.saturating_sub(thumb_w);
        let thumb_x = (scroll * travel) / max_scroll;
        for col in thumb_x..thumb_x.saturating_add(thumb_w).min(cols) {
            track[col] = '━';
        }
    }
    format!("\x1b[36m{}\x1b[0m", track.into_iter().collect::<String>())
}

/// 将单行正文填充到固定可见宽度，避免滚动条错位。
///
/// 参数:
/// - `line`: 可能含 ANSI 的正文行
/// - `width`: 目标可见宽度
///
/// 返回:
/// - 右侧补空格后的文本
fn pad_line(line: &str, width: usize) -> String {
    let visible = visible_width_ansi(line);
    if visible >= width {
        return line.to_string();
    }
    format!("{line}{}", " ".repeat(width - visible))
}

/// 计算含 ANSI 文本的可见宽度。
///
/// 参数:
/// - `value`: 原文
///
/// 返回:
/// - 可见列数
fn visible_width_ansi(value: &str) -> usize {
    let mut width = 0usize;
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        if ch == '\r' {
            continue;
        }
        width += UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
    }
    width
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_x_maps_edges() {
        assert_eq!(scroll_from_track_x(0, 100, 1000, 10), 0);
        let end = scroll_from_track_x(99, 100, 1000, 10);
        assert!(end >= 900, "end scroll should be near max, got {end}");
    }

    #[test]
    fn vertical_thumb_maps_edges() {
        assert_eq!(scroll_from_vertical_thumb(0, 20, 200, 180), 0);
        let end = scroll_from_vertical_thumb(19, 20, 200, 180);
        assert!(end >= 150, "end scroll should be near max, got {end}");
    }
}
